//! One place for long-running work started from the dashboard (indexing, docs rescan, eval, ...).
//! A job reports progress over the WebSocket, can be stopped, and keeps its last message for the UI.

use crate::commands::serve::state::AppState;
use axum::{extract::Path, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Serialize)]
pub struct JobInfo {
    pub id: String,
    pub kind: String,
    pub title: String,
    /// running, done, failed or cancelled
    pub state: String,
    pub done: usize,
    pub total: usize,
    /// Progress text while running; the result (or the error) when finished.
    pub message: Option<String>,
    pub started_at: i64,
}

struct Job {
    info: JobInfo,
    cancel: Arc<AtomicBool>,
}

static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());
const KEEP: usize = 20;

fn jobs() -> std::sync::MutexGuard<'static, Vec<Job>> {
    JOBS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Handle given to the running work.
pub struct JobCtl {
    id: String,
    cancel: Arc<AtomicBool>,
    tx: tokio::sync::broadcast::Sender<String>,
}

impl JobCtl {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn flag(&self) -> &AtomicBool {
        &self.cancel
    }

    pub fn progress(&self, done: usize, total: usize, message: Option<String>) {
        let info = {
            let mut all = jobs();
            let Some(j) = all.iter_mut().find(|j| j.info.id == self.id) else { return };
            j.info.done = done;
            j.info.total = total;
            if message.is_some() {
                j.info.message = message;
            }
            j.info.clone()
        };
        let _ = self.tx.send(json!({ "type": "job_progress", "job": info }).to_string());
    }
}

/// Starts `work` on its own thread. One job of a kind runs at a time.
pub fn start(
    state: &AppState,
    kind: &str,
    title: &str,
    work: impl FnOnce(&JobCtl) -> anyhow::Result<String> + Send + 'static,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut all = jobs();
        if all.iter().any(|j| j.info.kind == kind && j.info.state == "running") {
            return Err(format!("{title} is already running"));
        }
        all.push(Job {
            info: JobInfo { id: id.clone(), kind: kind.into(), title: title.into(), state: "running".into(), done: 0, total: 0, message: None, started_at: chrono::Utc::now().timestamp() },
            cancel: Arc::clone(&cancel),
        });
        let extra = all.len().saturating_sub(KEEP);
        if extra > 0 {
            let mut dropped = 0;
            all.retain(|j| {
                let drop_it = dropped < extra && j.info.state != "running";
                if drop_it {
                    dropped += 1;
                }
                !drop_it
            });
        }
    }
    let ctl = JobCtl { id: id.clone(), cancel, tx: state.broadcast_tx.clone() };
    std::thread::spawn(move || {
        let result = work(&ctl);
        let info = {
            let mut all = jobs();
            let Some(j) = all.iter_mut().find(|j| j.info.id == ctl.id) else { return };
            match result {
                Ok(msg) => {
                    j.info.state = if ctl.cancelled() { "cancelled" } else { "done" }.into();
                    j.info.message = Some(msg);
                }
                Err(e) => {
                    j.info.state = "failed".into();
                    j.info.message = Some(e.to_string());
                }
            }
            j.info.clone()
        };
        let _ = ctl.tx.send(json!({ "type": "job_progress", "job": info, "finished": true }).to_string());
    });
    Ok(id)
}

pub fn list() -> Vec<JobInfo> {
    let mut v: Vec<JobInfo> = jobs().iter().map(|j| j.info.clone()).collect();
    v.reverse();
    v
}

pub fn latest(kind: &str) -> Option<JobInfo> {
    jobs().iter().rev().find(|j| j.info.kind == kind).map(|j| j.info.clone())
}

/// Stops a running job by id; true when one was running.
pub fn cancel(id: &str) -> bool {
    jobs().iter().find(|j| j.info.id == id && j.info.state == "running").map(|j| j.cancel.store(true, Ordering::Relaxed)).is_some()
}

pub fn cancel_kind(kind: &str) -> bool {
    jobs().iter().find(|j| j.info.kind == kind && j.info.state == "running").map(|j| j.cancel.store(true, Ordering::Relaxed)).is_some()
}

/// GET /api/jobs
pub async fn api_list() -> impl IntoResponse {
    Json(list())
}

/// POST /api/jobs/:id/cancel (Safe: stopping work is harmless)
pub async fn api_cancel(Path(id): Path<String>) -> impl IntoResponse {
    Json(json!({ "cancelled": cancel(&id) }))
}
