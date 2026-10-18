//! Watches project folders for files that change while their lease holder is quiet.
//!
//! Hooks cover agents that run them. A file that changes under a lease whose holder has
//! done nothing for a while was most likely written by something else: an agent without
//! hooks (Cursor, Cline, Aider, a desktop app) or a person. The watcher cannot tell which,
//! and cannot block; it records an alert for the dashboard.

use crate::config::LockSettings;
use crate::db::queries::leases::scope_covers;
use crate::db::Database;
use anyhow::Result;
use chrono::{DateTime, Utc};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::params;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A holder with no tool event for this long is "idle" (its own edits arrive within seconds).
pub const IDLE_SECONDS: i64 = 20;
const ALERT_REPEAT: Duration = Duration::from_secs(60);

impl Database {
    /// Seconds since the session's last recorded tool event (None: never seen).
    pub fn session_idle_seconds(&self, session_id: &str) -> Option<i64> {
        let ts: String = self
            .conn()
            .query_row("SELECT MAX(timestamp) FROM tool_events WHERE session_id = ?1", params![session_id], |r| r.get(0))
            .ok()?;
        let last = DateTime::parse_from_rfc3339(&ts).ok()?.with_timezone(&Utc);
        Some((Utc::now() - last).num_seconds())
    }

    pub fn session_command_in_flight(&self, session_id: &str) -> bool {
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM workspace_snapshots WHERE session_id = ?1 AND in_flight = 1",
                params![session_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false)
    }
}

/// The alert for a change to `rel_path`, if its lease holder is idle. Pure decision, no I/O
/// beyond the database, so it can be tested without a file watcher.
pub fn alert_for_change(
    db: &Database,
    settings: &LockSettings,
    project_id: Option<&str>,
    rel_path: &str,
) -> Option<(String, String, String)> {
    if settings.is_ignored(rel_path) {
        return None;
    }
    let holder = db
        .list_active_leases(project_id)
        .ok()?
        .into_iter()
        .find(|l| scope_covers(&l.scope, &l.scope_type, rel_path))?;
    let idle = db.session_idle_seconds(&holder.session_id)?;
    if idle < IDLE_SECONDS || db.session_command_in_flight(&holder.session_id) {
        return None; // the holder is working: this is probably its own write
    }
    let msg = format!(
        "'{rel_path}' changed while its lease holder '{}' (goal: '{}') had been idle for {idle}s: \
         probably another agent without hooks, or you.",
        holder.agent, holder.goal
    );
    Some((holder.agent, holder.session_id, msg))
}

/// Starts the watcher thread. `on_alert` receives each new alert message (e.g. to broadcast it).
pub fn start(
    db: Arc<Database>,
    settings: LockSettings,
    roots: Vec<(Option<String>, PathBuf)>,
    on_alert: impl Fn(String) + Send + 'static,
) -> Result<()> {
    let (tx, rx) = channel::<notify::Result<notify::Event>>();
    let mut watcher = RecommendedWatcher::new(move |res| { let _ = tx.send(res); }, Config::default())?;
    for (_, root) in &roots {
        let _ = watcher.watch(root, RecursiveMode::Recursive);
    }
    std::thread::spawn(move || {
        let _keep = watcher;
        let mut last: HashMap<String, Instant> = HashMap::new();
        while let Ok(Ok(event)) = rx.recv() {
            if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)) {
                continue;
            }
            for path in event.paths {
                let Some((project, rel)) = relative_to_root(&roots, &path) else { continue };
                if last.get(&rel).is_some_and(|t| t.elapsed() < ALERT_REPEAT) {
                    continue;
                }
                if let Some((agent, session, msg)) = alert_for_change(&db, &settings, project.as_deref(), &rel) {
                    last.insert(rel.clone(), Instant::now());
                    db.record_alert(project.as_deref(), &rel, &agent, &session, &msg);
                    on_alert(msg);
                }
            }
            if last.len() > 2000 {
                last.retain(|_, t| t.elapsed() < ALERT_REPEAT);
            }
        }
    });
    Ok(())
}

fn relative_to_root(roots: &[(Option<String>, PathBuf)], path: &Path) -> Option<(Option<String>, String)> {
    roots.iter().find_map(|(project, root)| {
        path.strip_prefix(root).ok().map(|r| (project.clone(), r.to_string_lossy().to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LeaseAcquireArgs, LeaseScopeType};

    fn hold(db: &Database, session: &str, scope: &str) {
        db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("p"), agent: "claude", session_id: session, scope,
            scope_type: LeaseScopeType::File, goal: "refactor", ttl_seconds: 600,
        })
        .unwrap();
    }

    fn event_ago(db: &Database, session: &str, secs: i64) {
        db.conn()
            .execute(
                "INSERT INTO tool_events (event_id, event_type, session_id, agent_adapter, status, sensitivity, timestamp)
                 VALUES (?1, 'post-tool', ?2, 'claude', 'success', 'normal', ?3)",
                params![uuid::Uuid::new_v4().to_string(), session, (Utc::now() - chrono::Duration::seconds(secs)).to_rfc3339()],
            )
            .unwrap();
    }

    #[test]
    fn a_change_under_an_idle_holders_lease_raises_an_alert() {
        let db = Database::open_in_memory().unwrap();
        let s = LockSettings::default();
        hold(&db, "A", "src/a.rs");

        // Holder never seen: cannot judge.
        assert!(alert_for_change(&db, &s, Some("p"), "src/a.rs").is_none());
        // Holder active a moment ago: its own write.
        event_ago(&db, "A", 2);
        assert!(alert_for_change(&db, &s, Some("p"), "src/a.rs").is_none());
        // Holder idle for 5 minutes: someone else wrote it.
        event_ago(&db, "B", 300);
        hold(&db, "B", "src/b.rs");
        let (agent, session, msg) = alert_for_change(&db, &s, Some("p"), "src/b.rs").unwrap();
        assert_eq!((agent.as_str(), session.as_str()), ("claude", "B"));
        assert!(msg.contains("src/b.rs") && msg.contains("idle"));
        // Unleased and ignored files never alert.
        assert!(alert_for_change(&db, &s, Some("p"), "src/free.rs").is_none());
        assert!(alert_for_change(&db, &s, Some("p"), "target/debug/x").is_none());
        // A running command by the idle holder counts as activity.
        db.begin_snapshot("B", Some("p"), &Default::default()).unwrap();
        assert!(alert_for_change(&db, &s, Some("p"), "src/b.rs").is_none());
    }

    #[test]
    fn the_watcher_sees_a_real_file_change() {
        let dir = std::env::temp_dir().join(format!("am-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let canon = dir.canonicalize().unwrap();
        let db = Arc::new(Database::open_in_memory().unwrap());
        hold(&db, "B", "src/b.rs");
        event_ago(&db, "B", 300);
        let (tx, rx) = std::sync::mpsc::channel();
        start(db.clone(), LockSettings::default(), vec![(Some("p".into()), canon.clone())], move |m| { let _ = tx.send(m); }).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(canon.join("src/b.rs"), "changed by something else").unwrap();
        let msg = rx.recv_timeout(Duration::from_secs(10)).expect("an alert for the changed file");
        assert!(msg.contains("src/b.rs"));
        assert_eq!(db.recent_alerts(5).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
