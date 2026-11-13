//! Long operations, which start and return: a route registers the caller's op id, runs the work in
//! the background and answers 202. The op reports `AppEvent::Progress` and ends with
//! `AppEvent::OpEnded`; `DELETE /app/ops/{id}` cancels it. No request stays open for its length.

use std::{future::Future, sync::Arc};

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError, Empty, ErrorCode,
    server::{Req, route},
};
use dashmap::{DashMap, mapref::entry::Entry};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::{
    AppState,
    endpoints::ops::{CancelOp, OpResponse},
    events::AppEvent,
};

pub struct Ops {
    running: DashMap<String, CancellationToken>,
    events: broadcast::Sender<AppEvent>,
}

impl Ops {
    pub fn new(events: broadcast::Sender<AppEvent>) -> Self {
        Self {
            running: DashMap::new(),
            events,
        }
    }

    /// Runs `work` in the background as `op` and answers 202 once it is registered. The op ends
    /// with what `work` returns. `conflict` when an op with that id is running.
    pub fn start<F>(
        self: &Arc<Self>,
        op: String,
        work: impl FnOnce(Op) -> F,
    ) -> Result<StatusCode, ApiError>
    where
        F: Future<Output = Result<Option<OpResponse>, ApiError>>
            + Send
            + 'static,
    {
        let cancel = CancellationToken::new();
        match self.running.entry(op.clone()) {
            Entry::Occupied(_) => {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    "An operation with this id is already running",
                ));
            }
            Entry::Vacant(entry) => {
                entry.insert(cancel.clone());
            }
        }
        let work = work(Op {
            id: op.clone(),
            cancel,
            events: self.events.clone(),
        });
        let ops = Arc::clone(self);
        tokio::spawn(async move {
            let (error, response) = match work.await {
                Ok(response) => (None, response),
                Err(error) => (Some(error), None),
            };
            ops.running.remove(&op);
            let _ = ops.events.send(AppEvent::OpEnded {
                op,
                error,
                response,
            });
        });
        Ok(StatusCode::ACCEPTED)
    }

    fn cancel(&self, op: &str) -> Result<Empty, ApiError> {
        let cancel = self.running.get(op).ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                "No operation with this id is running",
            )
        })?;
        cancel.cancel();
        Ok(Empty)
    }
}

/// A running op, as its work sees it.
#[derive(Clone)]
pub struct Op {
    id: String,
    cancel: CancellationToken,
    events: broadcast::Sender<AppEvent>,
}

impl Op {
    pub fn progress(&self, stage: Option<&str>, done: u64, total: Option<u64>) {
        let _ = self.events.send(AppEvent::Progress {
            op: self.id.clone(),
            stage: stage.map(str::to_owned),
            done,
            total,
        });
    }

    /// Cancelled when the op is: for work that stops itself, such as blocking work.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// `work`, or `cancelled` once the op is cancelled, dropping `work`.
    pub async fn until_cancelled<T>(
        &self,
        work: impl Future<Output = Result<T, ApiError>>,
    ) -> Result<T, ApiError> {
        self.cancel
            .run_until_cancelled(work)
            .await
            .unwrap_or_else(|| Err(cancelled()))
    }
}

pub fn cancelled() -> ApiError {
    ApiError::new(ErrorCode::Cancelled, "Cancelled")
}

pub fn routes() -> Router<AppState> {
    let (path, cancel) = route::<CancelOp, _, _, _>(cancel_op);
    Router::new().route(path, cancel)
}

async fn cancel_op(
    State(ops): State<Arc<Ops>>,
    Req { params, .. }: Req<CancelOp>,
) -> Result<Empty, ApiError> {
    ops.cancel(&params.id)
}

#[cfg(test)]
mod tests {
    use tokio::sync::oneshot;

    use super::*;

    fn ops() -> (Arc<Ops>, broadcast::Receiver<AppEvent>) {
        let events = broadcast::Sender::new(16);
        let receiver = events.subscribe();
        (Arc::new(Ops::new(events)), receiver)
    }

    #[tokio::test]
    async fn a_start_answers_before_its_op_ends_with_the_work_result() {
        let (ops, mut events) = ops();
        let (finish, finished) = oneshot::channel::<()>();

        let started = ops.start("export".into(), |op| async move {
            op.progress(Some("exporting"), 1, Some(2));
            let _ = finished.await;
            Ok(Some(OpResponse {
                status: 201,
                body: "done".into(),
            }))
        });

        assert_eq!(started.unwrap(), StatusCode::ACCEPTED);
        assert!(matches!(
            events.recv().await.unwrap(),
            AppEvent::Progress { op, stage: Some(stage), done: 1, total: Some(2) }
                if op == "export" && stage == "exporting"
        ));
        assert!(events.try_recv().is_err(), "the op ended before its work");
        assert!(
            ops.start("export".into(), |_| async { Ok(None) }).is_err(),
            "a running op id was taken twice"
        );
        finish.send(()).unwrap();
        assert!(matches!(
            events.recv().await.unwrap(),
            AppEvent::OpEnded { op, error: None, response: Some(OpResponse { status: 201, body }) }
                if op == "export" && body == "done"
        ));
        assert_eq!(
            ops.cancel("export").unwrap_err().error,
            ErrorCode::NotFound
        );
    }

    #[tokio::test]
    async fn cancelling_ends_the_op_with_cancelled() {
        let (ops, mut events) = ops();
        ops.start("upload".into(), |op| async move {
            op.until_cancelled(std::future::pending::<Result<_, ApiError>>())
                .await
        })
        .unwrap();

        ops.cancel("upload").unwrap();

        assert!(matches!(
            events.recv().await.unwrap(),
            AppEvent::OpEnded { op, error: Some(error), response: None }
                if op == "upload" && error.error == ErrorCode::Cancelled
        ));
    }
}
