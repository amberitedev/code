use super::model::{
    InstallJobKind, InstallJobSnapshot, InstallJobState, InstallJobStatus,
    InstallProgressState, InstallRequest,
};
use crate::ErrorKind;
use crate::state::Installs;
use anyhow::{Context, bail};
use chrono::{DateTime, TimeZone, Utc};
use contracts::Side;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, MutexGuard, Weak};
use tokio::sync::Notify;
use uuid::Uuid;

pub(super) fn completion_notification(
    id: Uuid,
    installs: &Installs,
) -> Arc<Notify> {
    let mut notifications = installs
        .completions
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    notifications.retain(|_, notification| notification.strong_count() > 0);
    if let Some(notification) = notifications.get(&id).and_then(Weak::upgrade) {
        return notification;
    }
    let notification = Arc::new(Notify::new());
    notifications.insert(id, Arc::downgrade(&notification));
    notification
}

fn notify_completion(id: Uuid, installs: &Installs) {
    let notifications = installs
        .completions
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(notification) = notifications.get(&id).and_then(Weak::upgrade) {
        notification.notify_waiters();
    }
}

#[derive(Clone, Debug)]
pub struct InstallJobRecord {
    pub id: Uuid,
    pub instance_id: Option<String>,
    pub kind: InstallJobKind,
    pub status: InstallJobStatus,
    pub state: InstallJobState,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub finished: Option<DateTime<Utc>>,
    pub dismissed: bool,
}

#[derive(Debug)]
struct InstallJobRow {
    pub id: String,
    pub instance_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub state: String,
    pub created: i64,
    pub modified: i64,
    pub finished: Option<i64>,
    pub dismissed: i64,
}

impl InstallJobRecord {
    pub(crate) fn needs_recovery(&self) -> bool {
        self.instance_id.is_some()
            && matches!(
                self.status,
                InstallJobStatus::Failed | InstallJobStatus::Interrupted
            )
            && self.state.rollback_error.is_some()
    }

    /// Active jobs report their live progress, which can be newer than the stored one.
    pub(crate) fn snapshot(&self, installs: &Installs) -> InstallJobSnapshot {
        let active = is_active(self.status);
        let mut snapshot = InstallJobSnapshot {
            content_count: self.state.content_count,
            job_id: self.id.to_string(),
            instance_id: self.instance_id.clone(),
            kind: self.kind,
            status: self.status,
            paused: false,
            canceling: false,
            can_pause: false,
            can_cancel: false,
            target: self.state.target.clone(),
            phase: self.state.progress.phase,
            progress: self.state.progress.progress.clone(),
            details: self.state.progress.details.clone(),
            display: self.state.display.clone(),
            error: self.state.error.clone(),
            rollback_error: self.state.rollback_error.clone(),
            created: self.created.to_rfc3339(),
            modified: self.modified.to_rfc3339(),
            finished: self.finished.map(|finished| finished.to_rfc3339()),
        };
        if installs.side == Side::Server {
            without_host_paths(&mut snapshot);
        }
        set_controls(&mut snapshot, self.id, active, installs);
        if active && let Some(job) = live_jobs(installs).get_mut(&self.id) {
            job.apply_progress(&mut snapshot);
            job.snapshot = Some(snapshot.clone());
        }
        snapshot
    }
}

/// Drops what names a path on the host: an instance's icon file, and where a failed step read
/// or wrote. An icon from Modrinth is a URL and stays.
fn without_host_paths(snapshot: &mut InstallJobSnapshot) {
    if let Some(display) = &mut snapshot.display {
        display.icon.take_if(|icon| {
            !icon.starts_with("https://") && !icon.starts_with("http://")
        });
    }
    let errors = [&mut snapshot.error, &mut snapshot.rollback_error];
    for context in errors
        .into_iter()
        .flatten()
        .filter_map(|error| error.context.as_mut())
    {
        context.source_path = None;
        context.target_path = None;
    }
}

/// Progress of a job running in this engine. It changes far more often than the
/// rest of a job, so it is kept in memory instead of rewriting the stored job on
/// every update.
#[derive(Debug)]
pub(crate) struct LiveJob {
    progress: InstallProgressState,
    snapshot: Option<InstallJobSnapshot>,
}

impl LiveJob {
    fn apply_progress(&self, snapshot: &mut InstallJobSnapshot) {
        snapshot.phase = self.progress.phase;
        snapshot.progress = self.progress.progress.clone();
        snapshot.details = self.progress.details.clone();
    }
}

fn live_jobs(installs: &Installs) -> MutexGuard<'_, HashMap<Uuid, LiveJob>> {
    installs
        .live_jobs
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn is_active(status: InstallJobStatus) -> bool {
    matches!(status, InstallJobStatus::Queued | InstallJobStatus::Running)
}

fn set_controls(
    snapshot: &mut InstallJobSnapshot,
    id: Uuid,
    active: bool,
    installs: &Installs,
) {
    let (paused, canceling, controllable) =
        super::control::snapshot(id, installs);
    snapshot.paused = active && paused;
    snapshot.canceling = active && canceling;
    snapshot.can_pause = active && controllable;
    snapshot.can_cancel = active && controllable;
}

/// Keeps the live progress of a job in step with what was just stored for it.
fn track_live_job(record: &InstallJobRecord, installs: &Installs) {
    let mut jobs = live_jobs(installs);
    if !is_active(record.status) {
        jobs.remove(&record.id);
        return;
    }
    let progress = record.state.progress.clone();
    jobs.entry(record.id)
        .and_modify(|job| job.progress = progress.clone())
        .or_insert(LiveJob {
            progress,
            snapshot: None,
        });
}

/// Updates the progress of a running job without storing it. Returns `None`
/// when the job has no snapshot to update yet, so the caller should store it.
pub(super) fn update_live_progress(
    id: Uuid,
    state: &InstallJobState,
    installs: &Installs,
) -> Option<InstallJobSnapshot> {
    let mut jobs = live_jobs(installs);
    let job = jobs.get_mut(&id)?;
    let mut snapshot = job.snapshot.clone()?;
    job.progress = state.progress.clone();
    job.apply_progress(&mut snapshot);
    set_controls(&mut snapshot, id, true, installs);
    job.snapshot = Some(snapshot.clone());
    Some(snapshot)
}

/// Reads back a job that was just written.
async fn get_written(
    id: Uuid,
    pool: &SqlitePool,
    installs: &Installs,
) -> crate::Result<InstallJobRecord> {
    let record = get_required(id, pool).await?;
    track_live_job(&record, installs);
    Ok(record)
}

pub async fn insert(
    id: Uuid,
    request: &InstallRequest,
    state: &InstallJobState,
    status: InstallJobStatus,
    pool: &SqlitePool,
) -> crate::Result<InstallJobRecord> {
    let now = Utc::now();
    let kind = request.kind();
    let json = serde_json::to_string(state)?;
    let request_json = serde_json::to_string(request)?;
    let status_value = status.as_str();
    let kind_value = kind.as_str();
    let instance_id = instance_id(state);
    let id_value = id.to_string();
    let created = now.timestamp();
    let modified = created;

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "
		INSERT INTO install_jobs (
			id, instance_id, kind, status, state, created, modified, finished, dismissed
		)
		VALUES (?, ?, ?, ?, ?, ?, ?, NULL, 0)
		",
        id_value,
        instance_id,
        kind_value,
        status_value,
        json,
        created,
        modified,
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query!(
        "
		INSERT INTO install_job_requests (job_id, request)
		VALUES (?, ?)
		",
        id_value,
        request_json,
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    get(id, pool)
        .await?
        .context(format!("install job {id} was not inserted"))
}

/// The request a job was started with. Jobs that succeeded or were canceled
/// no longer have one, since they cannot be retried.
pub async fn get_request(
    id: Uuid,
    pool: &SqlitePool,
) -> crate::Result<Option<InstallRequest>> {
    let id = id.to_string();
    let request = sqlx::query_scalar!(
        "
		SELECT json(request) AS \"request!: String\"
		FROM install_job_requests
		WHERE job_id = ?
		",
        id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(request
        .map(|request| serde_json::from_str(&request))
        .transpose()?)
}

pub async fn get_required_request(
    id: Uuid,
    pool: &SqlitePool,
) -> crate::Result<InstallRequest> {
    get_request(id, pool).await?.ok_or(
        ErrorKind::InputError(format!(
            "install job {id} no longer has its request"
        ))
        .into(),
    )
}

/// Jobs that succeeded or were canceled cannot be retried, so their request is
/// no longer needed.
async fn release_request(
    id: Uuid,
    status: InstallJobStatus,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if !matches!(
        status,
        InstallJobStatus::Succeeded | InstallJobStatus::Canceled
    ) {
        return Ok(());
    }
    let id = id.to_string();
    sqlx::query!(
        "
		DELETE FROM install_job_requests
		WHERE job_id = ?
		",
        id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(
    id: Uuid,
    pool: &SqlitePool,
) -> crate::Result<Option<InstallJobRecord>> {
    let id = id.to_string();
    let row = sqlx::query_as!(
        InstallJobRow,
        "
		SELECT
			id AS \"id!: String\",
			instance_id,
			kind AS \"kind!: String\",
			status AS \"status!: String\",
			json(state) AS \"state!: String\",
			created AS \"created!: i64\",
			modified AS \"modified!: i64\",
			finished,
			dismissed AS \"dismissed!: i64\"
		FROM install_jobs
		WHERE id = ?
		",
        id,
    )
    .fetch_optional(pool)
    .await?;

    row.map(row_to_record).transpose()
}

pub async fn list(
    include_finished: bool,
    pool: &SqlitePool,
) -> crate::Result<Vec<InstallJobRecord>> {
    Ok(deserialize_rows(list_rows(pool).await?)
        .into_iter()
        .filter(|job| {
            job.needs_recovery()
                || !job.status.is_finished()
                || (!job.dismissed
                    && (include_finished
                        || matches!(
                            job.status,
                            InstallJobStatus::Failed
                                | InstallJobStatus::Interrupted
                        )))
        })
        .collect())
}

pub(crate) async fn list_all(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstallJobRecord>> {
    list_rows(pool)
        .await?
        .into_iter()
        .map(row_to_record)
        .collect()
}

async fn list_rows(pool: &SqlitePool) -> crate::Result<Vec<InstallJobRow>> {
    let rows = sqlx::query_as!(
        InstallJobRow,
        "
		SELECT
			id AS \"id!: String\",
			instance_id,
			kind AS \"kind!: String\",
			status AS \"status!: String\",
			json(state) AS \"state!: String\",
			created AS \"created!: i64\",
			modified AS \"modified!: i64\",
			finished,
			dismissed AS \"dismissed!: i64\"
		FROM install_jobs
		ORDER BY created ASC
		"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub(crate) async fn ensure_no_pending_recovery(
    instance_id: &str,
    except_job: Option<Uuid>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if list_all(pool).await?.iter().any(|job| {
        job.instance_id.as_deref() == Some(instance_id)
            && Some(job.id) != except_job
            && job.needs_recovery()
    }) {
        bail!(ErrorKind::InputError(
            "retry the failed installation to recover this instance before changing its content"
                .to_string(),
        ));
    }
    Ok(())
}

pub async fn list_interrupted_candidates(
    pool: &SqlitePool,
) -> crate::Result<Vec<InstallJobRecord>> {
    let rows = sqlx::query_as!(
        InstallJobRow,
        "
		SELECT
			id AS \"id!: String\",
			instance_id,
			kind AS \"kind!: String\",
			status AS \"status!: String\",
			json(state) AS \"state!: String\",
			created AS \"created!: i64\",
			modified AS \"modified!: i64\",
			finished,
			dismissed AS \"dismissed!: i64\"
		FROM install_jobs
		WHERE status IN ('queued', 'running')
		ORDER BY created ASC
		",
    )
    .fetch_all(pool)
    .await?;

    Ok(deserialize_rows(rows))
}

pub async fn list_active_for_instance(
    instance_id: &str,
    pool: &SqlitePool,
) -> crate::Result<Vec<InstallJobRecord>> {
    Ok(list_interrupted_candidates(pool)
        .await?
        .into_iter()
        .filter(|job| job.instance_id.as_deref() == Some(instance_id))
        .collect())
}

pub(crate) async fn update_state(
    id: Uuid,
    state: &InstallJobState,
    pool: &SqlitePool,
    installs: &Installs,
) -> crate::Result<InstallJobRecord> {
    let now = Utc::now();
    let json = serde_json::to_string(state)?;
    let instance_id = instance_id(state);
    let id_value = id.to_string();
    let modified = now.timestamp();

    let result = sqlx::query(
        "
		UPDATE install_jobs
		SET
			instance_id = (SELECT id FROM instances WHERE id = ?),
			state = ?,
			modified = ?
		WHERE id = ? AND status IN ('queued', 'running')
		",
    )
    .bind(instance_id)
    .bind(json)
    .bind(modified)
    .bind(id_value)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        bail!(ErrorKind::InputError(format!(
            "install job {id} is no longer active"
        )));
    }

    get_written(id, pool, installs).await
}

pub(crate) async fn update_status(
    id: Uuid,
    status: InstallJobStatus,
    state: &InstallJobState,
    pool: &SqlitePool,
    installs: &Installs,
) -> crate::Result<InstallJobRecord> {
    let now = Utc::now();
    let finished = status.is_finished().then_some(now.timestamp());
    let json = serde_json::to_string(state)?;
    let status_value = status.as_str();
    let instance_id = instance_id(state);
    let id_value = id.to_string();
    let modified = now.timestamp();

    sqlx::query!(
        "
		UPDATE install_jobs
		SET instance_id = ?, status = ?, state = ?, modified = ?, finished = ?
		WHERE id = ?
		",
        instance_id,
        status_value,
        json,
        modified,
        finished,
        id_value,
    )
    .execute(pool)
    .await?;

    release_request(id, status, pool).await?;
    if status.is_finished() {
        notify_completion(id, installs);
    }

    get_written(id, pool, installs).await
}

pub(crate) async fn update_status_if(
    id: Uuid,
    expected_status: InstallJobStatus,
    status: InstallJobStatus,
    state: &InstallJobState,
    pool: &SqlitePool,
    installs: &Installs,
) -> crate::Result<Option<InstallJobRecord>> {
    let now = Utc::now();
    let finished = status.is_finished().then_some(now.timestamp());
    let json = serde_json::to_string(state)?;
    let status_value = status.as_str();
    let expected_status_value = expected_status.as_str();
    let instance_id = instance_id(state);
    let id_value = id.to_string();
    let modified = now.timestamp();

    let result = sqlx::query(
        "
		UPDATE install_jobs
		SET instance_id = ?, status = ?, state = ?, modified = ?, finished = ?
		WHERE id = ? AND status = ?
		",
    )
    .bind(instance_id)
    .bind(status_value)
    .bind(json)
    .bind(modified)
    .bind(finished)
    .bind(id_value)
    .bind(expected_status_value)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    release_request(id, status, pool).await?;
    if status.is_finished() {
        notify_completion(id, installs);
    }

    get_written(id, pool, installs).await.map(Some)
}

pub(crate) async fn finish_active(
    id: Uuid,
    status: InstallJobStatus,
    state: &InstallJobState,
    pool: &SqlitePool,
    installs: &Installs,
) -> crate::Result<Option<InstallJobRecord>> {
    let now = Utc::now();
    let finished = now.timestamp();
    let json = serde_json::to_string(state)?;
    let status_value = status.as_str();
    let instance_id = instance_id(state);
    let id_value = id.to_string();
    let modified = finished;

    let result = sqlx::query(
        "
		UPDATE install_jobs
		SET
			instance_id = (SELECT id FROM instances WHERE id = ?),
			status = ?,
			state = ?,
			modified = ?,
			finished = ?
		WHERE id = ? AND status IN ('queued', 'running')
		",
    )
    .bind(instance_id)
    .bind(status_value)
    .bind(json)
    .bind(modified)
    .bind(finished)
    .bind(id_value)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    release_request(id, status, pool).await?;
    if status.is_finished() {
        notify_completion(id, installs);
    }

    get_written(id, pool, installs).await.map(Some)
}

pub(crate) async fn complete_success(
    id: Uuid,
    state: &InstallJobState,
    pool: &SqlitePool,
    installs: &Installs,
) -> crate::Result<Option<InstallJobRecord>> {
    let Some(instance_id) = instance_id(state) else {
        bail!(ErrorKind::InputError(
            "install job is missing its instance id".to_string(),
        ));
    };
    let now = Utc::now().timestamp();
    let json = serde_json::to_string(state)?;
    let id_value = id.to_string();
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;

    let job_result = sqlx::query(
        "
		UPDATE install_jobs
		SET
			instance_id = (SELECT id FROM instances WHERE id = ?),
			status = 'succeeded',
			state = ?,
			modified = ?,
			finished = ?
		WHERE id = ? AND status = 'running'
		",
    )
    .bind(&instance_id)
    .bind(json)
    .bind(now)
    .bind(now)
    .bind(id_value)
    .execute(&mut *transaction)
    .await?;

    if job_result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(None);
    }

    let instance_result = sqlx::query(
        "
		UPDATE instances
		SET install_stage = 'installed', modified = ?
		WHERE id = ?
		",
    )
    .bind(now)
    .bind(&instance_id)
    .execute(&mut *transaction)
    .await?;

    if instance_result.rows_affected() == 0 {
        transaction.rollback().await?;
        bail!(ErrorKind::InputError(format!(
            "unknown instance {instance_id}"
        )));
    }

    transaction.commit().await?;
    release_request(id, InstallJobStatus::Succeeded, pool).await?;
    notify_completion(id, installs);
    get_written(id, pool, installs).await.map(Some)
}

pub async fn dismiss(id: Uuid, pool: &SqlitePool) -> crate::Result<()> {
    let job = get_required(id, pool).await?;
    if job.instance_id.is_some() && job.needs_recovery() {
        bail!(ErrorKind::InputError(
            "recover or delete this instance before dismissing its failed installation"
                .to_string(),
        ));
    }
    let id = id.to_string();
    let modified = Utc::now().timestamp();
    sqlx::query!(
        "
		UPDATE install_jobs
		SET dismissed = 1, modified = ?
		WHERE id = ?
		",
        modified,
        id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_required(
    id: Uuid,
    pool: &SqlitePool,
) -> crate::Result<InstallJobRecord> {
    get(id, pool).await?.ok_or(
        ErrorKind::InputError(format!("unknown install job {id}")).into(),
    )
}

fn row_to_record(row: InstallJobRow) -> crate::Result<InstallJobRecord> {
    Ok(InstallJobRecord {
        id: Uuid::parse_str(&row.id).context(ErrorKind::InputError(
            format!("invalid install job id {}", row.id),
        ))?,
        instance_id: row.instance_id,
        kind: InstallJobKind::from_stored_str(&row.kind),
        status: InstallJobStatus::from_stored_str(&row.status),
        state: serde_json::from_str(&row.state)?,
        created: timestamp(row.created),
        modified: timestamp(row.modified),
        finished: row.finished.and_then(optional_timestamp),
        dismissed: row.dismissed != 0,
    })
}

fn deserialize_rows(rows: Vec<InstallJobRow>) -> Vec<InstallJobRecord> {
    rows.into_iter()
        .filter_map(|row| {
            let id = row.id.clone();
            match row_to_record(row) {
                Ok(record) => Some(record),
                Err(error) => {
                    tracing::error!(
                        "Failed to deserialize install job {id}: {error}"
                    );
                    None
                }
            }
        })
        .collect()
}

fn instance_id(state: &InstallJobState) -> Option<String> {
    match &state.target {
        super::model::InstallTarget::NewInstance { instance_id } => {
            instance_id.clone()
        }
        super::model::InstallTarget::ExistingInstance { instance_id } => {
            Some(instance_id.clone())
        }
    }
}

fn timestamp(value: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(value, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

fn optional_timestamp(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(value, 0).single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::model::{
        InstallErrorContext, InstallErrorView, InstallJobDisplay,
        InstallPhaseId,
    };
    use crate::state::{EngineConfig, tests};

    #[tokio::test(flavor = "multi_thread")]
    async fn a_servers_jobs_name_no_path_on_its_host() {
        crate::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let engine = crate::Engine::new(EngineConfig {
            side: Side::Server,
            ..tests::config(dir.path().to_path_buf())
        })
        .await
        .unwrap();
        tests::add_instance(&engine, "s", "srv").await;
        let request = InstallRequest::InstallExistingInstance {
            instance_id: "s".to_owned(),
            force: false,
        };
        let mut state = InstallJobState::new(&request);
        state.display = Some(InstallJobDisplay {
            title: "srv".to_owned(),
            icon: Some("/srv/copal/icon.png".to_owned()),
        });
        let failed = InstallErrorView {
            context: Some(
                InstallErrorContext::new("extract modpack override")
                    .source_path("/srv/copal/pack.mrpack")
                    .target_path("/srv/copal/profiles/srv/config/a.toml")
                    .entry_path("overrides/config/a.toml")
                    .build(),
            ),
            ..InstallErrorView::from_message(
                "path_error",
                InstallPhaseId::ExtractingOverrides,
                "failed",
            )
        };
        state.error = Some(failed.clone());
        state.rollback_error = Some(failed);
        insert(
            Uuid::new_v4(),
            &request,
            &state,
            InstallJobStatus::Failed,
            &engine.pool,
        )
        .await
        .unwrap();

        let jobs = crate::install::list_jobs(true, &engine).await.unwrap();
        let json = serde_json::to_string(&jobs).unwrap();
        assert!(!json.contains("/srv/copal"), "{json}");
        assert!(json.contains("overrides/config/a.toml"), "{json}");
    }
}
