use super::model::{
    InstallErrorContext, InstallJobEventKind, InstallJobSnapshot,
    InstallJobState, InstallPhaseDetails, InstallPhaseId, InstallProgress,
};
use super::store;
use crate::event::{Event, EventSink};
use crate::state::{Installs, State};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(750);
const CONTENT_PROGRESS_EMIT_STEPS: u64 = 25;

#[derive(Clone, Debug)]
pub struct InstallProgressReporter {
    job_id: Uuid,
    pool: SqlitePool,
    installs: Arc<Installs>,
    events: EventSink,
    state: Arc<Mutex<InstallProgressReporterState>>,
}

#[derive(Debug)]
struct InstallProgressReporterState {
    job: InstallJobState,
    last_emitted_at: Instant,
    last_emitted_progress: Option<(InstallPhaseId, u64)>,
}

impl InstallProgressReporter {
    pub fn new(job_id: Uuid, state: InstallJobState, engine: &State) -> Self {
        Self {
            job_id,
            pool: engine.pool.clone(),
            installs: Arc::clone(&engine.installs),
            events: engine.events.clone(),
            state: Arc::new(Mutex::new(InstallProgressReporterState {
                job: state,
                last_emitted_at: Instant::now(),
                last_emitted_progress: None,
            })),
        }
    }

    pub async fn update(
        &self,
        phase: InstallPhaseId,
        progress: Option<InstallProgress>,
        details: InstallPhaseDetails,
    ) -> crate::Result<()> {
        self.update_with_events(phase, progress, details, Vec::new())
            .await
    }

    pub async fn set_context(
        &self,
        context: InstallErrorContext,
    ) -> crate::Result<()> {
        super::control::checkpoint(self.job_id, &self.installs).await?;
        self.update_context(Some(context), true).await
    }

    pub async fn set_transient_context(
        &self,
        context: InstallErrorContext,
    ) -> crate::Result<()> {
        super::control::checkpoint(self.job_id, &self.installs).await?;
        self.update_context(Some(context), false).await
    }

    pub async fn clear_context(&self) -> crate::Result<()> {
        self.update_context(None, true).await
    }

    async fn update_context(
        &self,
        context: Option<InstallErrorContext>,
        persist: bool,
    ) -> crate::Result<()> {
        let mut state = self.state.lock().await;
        state.job.set_context(context);

        if !persist {
            return Ok(());
        }

        let record = store::update_state(
            self.job_id,
            &state.job,
            &self.pool,
            &self.installs,
        )
        .await?;
        state.mark_emitted();
        emit_install_job(&self.events, &record.snapshot(&self.installs)).await
    }

    pub async fn persist(&self) -> crate::Result<InstallJobSnapshot> {
        let mut state = self.state.lock().await;

        let record = store::update_state(
            self.job_id,
            &state.job,
            &self.pool,
            &self.installs,
        )
        .await?;
        state.mark_emitted();
        let snapshot = record.snapshot(&self.installs);
        emit_install_job(&self.events, &snapshot).await?;
        Ok(snapshot)
    }

    pub async fn persist_failure_context(&self, context: InstallErrorContext) {
        if let Err(error) = self.update_context(Some(context), true).await {
            tracing::warn!(
                "Failed to persist install context for failed operation: {error}"
            );
        }
    }

    pub async fn preserve_failure_context<T>(
        &self,
        context: InstallErrorContext,
        result: crate::Result<T>,
    ) -> crate::Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.persist_failure_context(context).await;
                Err(error)
            }
        }
    }

    pub async fn update_with_events(
        &self,
        phase: InstallPhaseId,
        progress: Option<InstallProgress>,
        details: InstallPhaseDetails,
        events: Vec<InstallJobEventKind>,
    ) -> crate::Result<()> {
        super::control::checkpoint(self.job_id, &self.installs).await?;
        let mut state = self.state.lock().await;
        let phase_started = state.job.progress.phase != phase
            || matches!(
                &state.job.progress.details,
                InstallPhaseDetails::Empty
            ) && !matches!(&details, InstallPhaseDetails::Empty);

        let persist = phase_started || !events.is_empty();

        state.job.set_progress(phase, progress, details);
        for event in events {
            state.job.record_event(event);
        }

        if !persist {
            if !state.should_emit() {
                return Ok(());
            }
            if let Some(snapshot) = store::update_live_progress(
                self.job_id,
                &state.job,
                &self.installs,
            ) {
                state.mark_emitted();
                return emit_install_job(&self.events, &snapshot).await;
            }
        }

        let record = store::update_state(
            self.job_id,
            &state.job,
            &self.pool,
            &self.installs,
        )
        .await?;
        state.mark_emitted();
        emit_install_job(&self.events, &record.snapshot(&self.installs)).await
    }
}

impl InstallProgressReporterState {
    fn should_emit(&self) -> bool {
        let Some(progress) = &self.job.progress.progress else {
            return true;
        };

        if progress.current >= progress.total {
            return true;
        }

        let progressed_enough =
            if self.job.progress.phase == InstallPhaseId::DownloadingContent {
                self.last_emitted_progress
                    .map(|(phase, current)| {
                        phase != self.job.progress.phase
                            || progress.current.saturating_sub(current)
                                >= CONTENT_PROGRESS_EMIT_STEPS
                    })
                    .unwrap_or(true)
            } else {
                false
            };

        progressed_enough
            || self.last_emitted_at.elapsed() >= PROGRESS_EMIT_INTERVAL
    }

    fn mark_emitted(&mut self) {
        self.last_emitted_at = Instant::now();
        self.last_emitted_progress = self
            .job
            .progress
            .progress
            .as_ref()
            .map(|progress| (self.job.progress.phase, progress.current));
    }
}

pub async fn emit_install_job(
    events: &EventSink,
    snapshot: &InstallJobSnapshot,
) -> crate::Result<()> {
    events.send(Event::InstallJob(Arc::new(snapshot.clone())));
    Ok(())
}
