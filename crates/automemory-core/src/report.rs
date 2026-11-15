//! "Is it working?" in numbers, built only from rows AutoMemory already keeps (audit log, leases, memories, batons).
//! Nothing is measured in addition, so every figure can be traced to a row.
use crate::db::Database;
use chrono::{Duration, Utc};
use rusqlite::params;
use serde_json::{json, Value};

fn count(db: &Database, sql: &str, since: &str) -> i64 {
    db.conn().query_row(sql, params![since], |r| r.get(0)).unwrap_or(0)
}

/// Counts for the last `days` days (plus a few totals that are not time-bound).
pub fn value_report(db: &Database, days: i64) -> Value {
    let since = (Utc::now() - Duration::days(days)).to_rfc3339();
    // A lock denial or question says "Collision prevented"; anything else that blocked was a guardrail rule.
    let blocked = |action: &str, like: &str| {
        let sql = format!("SELECT COUNT(*) FROM audit_log WHERE action = '{action}' AND timestamp > ?1 AND details {like} '%Collision prevented%'");
        count(db, &sql, &since)
    };
    let (deny_all, ask_all) = (
        count(db, "SELECT COUNT(*) FROM audit_log WHERE action = 'hook.deny' AND timestamp > ?1", &since),
        count(db, "SELECT COUNT(*) FROM audit_log WHERE action = 'hook.ask' AND timestamp > ?1", &since),
    );
    let collisions = blocked("hook.deny", "LIKE") + blocked("hook.ask", "LIKE");
    json!({
        "days": days,
        "collisions_prevented": collisions,
        "risky_commands_stopped": (deny_all + ask_all - collisions).max(0),
        "notes_recalled": count(db, "SELECT COUNT(*) FROM memories WHERE last_recalled_at > ?1 AND (project_id IS NULL OR project_id != 'demo')", &since),
        "notes_added_by_you": count(db, "SELECT COUNT(*) FROM memories WHERE source_kind = 'user' AND created_at > ?1 AND (project_id IS NULL OR project_id != 'demo')", &since),
        "notes_added_by_agents": count(db, "SELECT COUNT(*) FROM memories WHERE source_kind != 'user' AND status = 'active' AND created_at > ?1 AND (project_id IS NULL OR project_id != 'demo')", &since),
        "handoffs_written": count(db, "SELECT COUNT(*) FROM batons WHERE created_at > ?1", &since),
        "handoffs_picked_up": count(db, "SELECT COUNT(*) FROM batons WHERE to_session IS NOT NULL AND created_at > ?1", &since),
        "dashboard_actions": count(db, "SELECT COUNT(*) FROM audit_log WHERE action = 'ui.action' AND timestamp > ?1", &since),
        "active_notes": count(db, "SELECT COUNT(*) FROM memories WHERE status IN ('active','pinned') AND ?1 IS NOT NULL", &since),
        "waiting_for_review": count(db, "SELECT COUNT(*) FROM review_proposals WHERE status = 'pending' AND ?1 IS NOT NULL", &since),
    })
}

/// One short line per non-zero figure, in plain words.
pub fn summary(v: &Value) -> String {
    let n = |k: &str| v[k].as_i64().unwrap_or(0);
    let rows = [
        (n("collisions_prevented"), "edit collisions prevented"),
        (n("risky_commands_stopped"), "risky commands stopped or questioned"),
        (n("notes_recalled"), "notes given to agents"),
        (n("notes_added_by_you"), "notes you added"),
        (n("notes_added_by_agents"), "notes agents added"),
        (n("handoffs_written"), "hand-offs written"),
        (n("handoffs_picked_up"), "hand-offs picked up"),
    ];
    let body: Vec<String> = rows.iter().filter(|(c, _)| *c > 0).map(|(c, l)| format!("  {c:>4}  {l}")).collect();
    let mut out = format!("Last {} days\n", v["days"]);
    out += &if body.is_empty() { "  Nothing yet. Connect an agent and use it; this fills in as it works.\n".to_string() } else { body.join("\n") + "\n" };
    out + &format!("Now: {} active notes, {} waiting for your review\n", n("active_notes"), n("waiting_for_review"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_come_from_rows_and_collisions_are_told_from_other_blocks() {
        let db = Database::open_in_memory().unwrap();
        assert!(summary(&value_report(&db, 7)).contains("Nothing yet"));
        let log = |a: &str, reason: &str| db.log_audit(a, "tool", "Edit", &json!({ "reason": reason })).unwrap();
        log("hook.deny", "Collision prevented: 'a.rs' is locked by agent 'x'");
        log("hook.ask", "Collision prevented: 'b.rs' is locked by agent 'y'");
        log("hook.deny", "Blocked: git push --force");
        log("ui.action", "pin a memory");
        let v = value_report(&db, 7);
        assert_eq!(v["collisions_prevented"], 2);
        assert_eq!(v["risky_commands_stopped"], 1);
        assert_eq!(v["dashboard_actions"], 1);
        let text = summary(&v);
        assert!(text.contains("2  edit collisions prevented") && text.contains("1  risky commands"), "{text}");
        assert_eq!(value_report(&db, 7)["collisions_prevented"], 2, "idempotent");
    }
}
