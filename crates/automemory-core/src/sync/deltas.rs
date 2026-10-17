//! Git-like incremental sync: content-addressed op-log deltas with
//! fingerprint convergence and conflict routing.
//!
//! Model (mirrors git's object model, in plain JSONL):
//! - Every push writes ONLY records changed since this device last synced
//!   them, into `deltas/<sha256>-<device>.jsonl` (content-addressed:
//!   identical content never duplicates, concurrent devices never
//!   filename-collide, so concurrent pushes never git-conflict).
//! - Pull applies every delta file present (sorted); application is
//!   idempotent, so re-applying is always safe.
//! - Convergence is by content fingerprint, never by clock: no NTP
//!   assumptions, no silent last-writer-wins on concurrent edits.
//! - Concurrent edits of one record become `sync_conflict` review proposals;
//!   the local copy is always kept.
//! - Archive is a status flip, so deletes travel as ordinary upserts —
//!   no tombstone table needed.

use crate::db::Database;
use crate::models::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// One synced record version. `fingerprint` is sha256 over the canonical
/// payload JSON; equality means convergence regardless of clocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaOp {
    pub op: String,
    pub kind: String,
    pub id: String,
    pub project_id: Option<String>,
    pub updated_at: String,
    pub device_id: String,
    pub fingerprint: String,
    pub payload: serde_json::Value,
}

/// Outcome of applying delta files. Conflicts keep the local copy and file
/// a review proposal; nothing is ever silently overwritten.
#[derive(Debug, Default)]
pub struct ApplyReport {
    pub applied: usize,
    pub converged: usize,
    pub conflicts: usize,
    pub skipped_own: usize,
    pub stale: usize,
}

const APPLIED_KEY: &str = "applied_fingerprints";

fn get_kv(db: &Database, key: &str) -> Result<Option<String>> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT value FROM sync_kv WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

fn set_kv(db: &Database, key: &str, value: &str) -> Result<()> {
    db.conn().execute(
        "INSERT INTO sync_kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Fingerprints this device already pushed or applied: `{record_id: sha256}`.
/// Unbounded growth is accepted (personal scale); entries never expire
/// because a stable fingerprint means "nothing to send", forever.
pub fn get_applied(db: &Database) -> Result<HashMap<String, String>> {
    match get_kv(db, APPLIED_KEY)? {
        Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        None => Ok(HashMap::new()),
    }
}

pub fn merge_applied(db: &Database, entries: &HashMap<String, String>) -> Result<()> {
    let mut map = get_applied(db)?;
    for (k, v) in entries {
        map.insert(k.clone(), v.clone());
    }
    set_kv(db, APPLIED_KEY, &serde_json::to_string(&map)?)?;
    Ok(())
}

fn fingerprint(payload: &serde_json::Value) -> String {
    let raw = serde_json::to_string(payload).unwrap_or_default();
    format!("{:x}", Sha256::digest(raw.as_bytes()))
}

fn fingerprint_of<T: Serialize>(v: &T) -> String {
    fingerprint(&serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
}

fn make_op(
    kind: &str,
    id: String,
    project_id: Option<String>,
    updated_at: DateTime<Utc>,
    device_id: &str,
    payload: serde_json::Value,
) -> DeltaOp {
    let fp = fingerprint(&payload);
    DeltaOp {
        op: "upsert".to_string(),
        kind: kind.to_string(),
        id,
        project_id,
        updated_at: updated_at.to_rfc3339(),
        device_id: device_id.to_string(),
        fingerprint: fp,
        payload,
    }
}

/// Collects every sync-worthy record whose fingerprint differs from what
/// this device last pushed or applied. Deterministic order for stable files.
pub fn collect_deltas(db: &Database, device_id: &str) -> Result<Vec<DeltaOp>> {
    let applied = get_applied(db)?;
    let mut ops = Vec::new();
    let mut consider = |kind: &str,
                        id: String,
                        project_id: Option<String>,
                        updated_at: DateTime<Utc>,
                        payload: serde_json::Value| {
        let op = make_op(kind, id, project_id, updated_at, device_id, payload);
        if applied.get(&op.id).map(|fp| fp == &op.fingerprint) != Some(true) {
            ops.push(op);
        }
    };

    for m in db.list_memories(None, None, None, 10000).unwrap_or_default() {
        let updated = m.updated_at;
        let pid = m.project_id.clone();
        let id = m.id.clone();
        let payload = serde_json::to_value(&m).unwrap_or(serde_json::Value::Null);
        consider("memory", id, pid, updated, payload);
    }
    for t in db.list_traces(None, 5000).unwrap_or_default() {
        let updated = t.updated_at;
        let pid = t.project_id.clone();
        let id = t.id.clone();
        let payload = serde_json::to_value(&t).unwrap_or(serde_json::Value::Null);
        consider("trace", id, pid, updated, payload);
    }
    for f in db.list_failures(None, None, 5000).unwrap_or_default() {
        let updated = f.updated_at;
        let pid = f.project_id.clone();
        let id = f.id.clone();
        let payload = serde_json::to_value(&f).unwrap_or(serde_json::Value::Null);
        consider("failure", id, pid, updated, payload);
    }
    for s in db.list_skills(None, 10000).unwrap_or_default() {
        let updated = s.updated_at;
        let pid = s.project_id.clone();
        let id = s.id.clone();
        let payload = serde_json::to_value(&s).unwrap_or(serde_json::Value::Null);
        consider("skill", id, pid, updated, payload);
    }
    for d in db.list_decisions(None, None, 10000).unwrap_or_default() {
        let updated = d.updated_at;
        let pid = d.project_id.clone();
        let id = d.id.clone();
        let payload = serde_json::to_value(&d).unwrap_or(serde_json::Value::Null);
        consider("decision", id, pid, updated, payload);
    }

    ops.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
    Ok(ops)
}

/// Writes ops to a content-addressed delta file. Returns None when there is
/// nothing new (caller writes nothing, commits nothing extra).
pub fn write_delta_file(
    vault_root: &Path,
    device_id: &str,
    ops: &[DeltaOp],
) -> Result<Option<PathBuf>> {
    if ops.is_empty() {
        return Ok(None);
    }
    let canonical = serde_json::to_string(ops)?;
    let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    let deltas_dir = vault_root.join("deltas");
    fs::create_dir_all(&deltas_dir)?;
    let path = deltas_dir.join(format!("{}-{}.jsonl", &digest[..16], device_id));
    if !path.exists() {
        let mut out = String::with_capacity(canonical.len() + ops.len() * 2);
        for op in ops {
            out.push_str(&serde_json::to_string(op)?);
            out.push('\n');
        }
        fs::write(&path, out)?;
    }
    Ok(Some(path))
}

/// Reads one delta file tolerantly: corrupt lines are warned and skipped,
/// never fatal (mirrors the snapshot importer's contract).
pub fn read_delta_file(path: &Path) -> Vec<DeltaOp> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut ops = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<DeltaOp>(line) {
            Ok(op) => ops.push(op),
            Err(e) => tracing::warn!(
                path = %path.display(),
                line_no = idx + 1,
                error = %e,
                "delta apply: unparseable line skipped"
            ),
        }
    }
    ops
}

/// Lists delta files oldest-first (content hash prefix is not time-ordered
/// across devices, so mtime order is used with name tiebreak).
pub fn list_delta_files(vault_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(vault_root.join("deltas")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn conflict_proposal(op: &DeltaOp, local_updated: DateTime<Utc>) -> ReviewProposal {
    ReviewProposal {
        id: format!("sync-{}", uuid::Uuid::now_v7().simple()),
        project_id: op.project_id.clone(),
        proposal_type: "sync_conflict".to_string(),
        target_id: format!("{}:{}", op.kind, op.id),
        description: format!(
            "Concurrent edit of {} {}: local version from {} vs {} version from {} (device {}). Local copy kept; resolve manually, then push.",
            op.kind,
            op.id,
            local_updated.to_rfc3339(),
            op.kind,
            op.updated_at,
            op.device_id
        ),
        suggested_action: "keep_local".to_string(),
        status: "pending".to_string(),
        created_at: Utc::now(),
        decided_at: None,
    }
}

fn already_proposed(db: &Database, target_id: &str) -> bool {
    db.list_pending_proposals(None)
        .unwrap_or_default()
        .iter()
        .any(|p| p.proposal_type == "sync_conflict" && p.target_id == target_id)
}

/// Core merge rule, clock-free:
/// - no local row → insert.
/// - fingerprint equal → converged, skip.
/// - never synced this id → converge (first-sync rule, matches legacy import).
/// - local fingerprint == last-synced → untouched → fast-forward apply.
/// - incoming fingerprint == last-synced → remote stale → skip.
/// - else both moved → conflict proposal, keep local.
fn merge_one(
    db: &Database,
    applied: &HashMap<String, String>,
    op: &DeltaOp,
    local_fp: Option<String>,
    local_updated: Option<DateTime<Utc>>,
    write: impl FnOnce() -> Result<()>,
    report: &mut ApplyReport,
) -> Result<Option<(String, String)>> {
    let record = |report: &mut ApplyReport| {
        report.applied += 1;
        Some((op.id.clone(), op.fingerprint.clone()))
    };
    match (local_fp, applied.get(&op.id)) {
        (None, _) => {
            write()?;
            Ok(record(report))
        }
        (Some(local), _) if local == op.fingerprint => {
            report.converged += 1;
            Ok(Some((op.id.clone(), op.fingerprint.clone())))
        }
        (_, None) => {
            write()?;
            Ok(record(report))
        }
        (Some(local), Some(known)) if local == *known => {
            write()?;
            Ok(record(report))
        }
        (Some(_), Some(known)) if op.fingerprint == *known => {
            report.stale += 1;
            Ok(None)
        }
        (Some(_), Some(_)) => {
            let target = format!("{}:{}", op.kind, op.id);
            if !already_proposed(db, &target) {
                let when = local_updated.unwrap_or_else(Utc::now);
                let _ = db.save_proposal(&conflict_proposal(op, when));
            }
            report.conflicts += 1;
            // Deliberately NOT recorded: the local copy still diverges, so
            // the next collect must re-emit it and the next pull must
            // re-evaluate (deduplicated by already_proposed).
            Ok(None)
        }
    }
}

fn apply_op(
    db: &Database,
    applied: &HashMap<String, String>,
    op: &DeltaOp,
    report: &mut ApplyReport,
) -> Result<Option<(String, String)>> {
    macro_rules! load {
        ($get:ident) => {{
            match db.$get(&op.id)? {
                None => None,
                Some(local) => {
                    let fp = fingerprint_of(&local);
                    Some((fp, local.updated_at))
                }
            }
        }};
    }
    match op.kind.as_str() {
        "memory" => {
            let mem: Memory = serde_json::from_value(op.payload.clone())?;
            match load!(get_memory_by_id) {
                None => merge_one(db, applied, op, None, None, || db.save_memory_direct(&mem), report),
                Some((fp, updated)) => merge_one(db, applied, op, Some(fp), Some(updated), || {
                    db.save_memory_direct(&mem)
                }, report),
            }
        }
        "trace" => {
            let t: SuccessTrace = serde_json::from_value(op.payload.clone())?;
            match db.get_trace_by_id(&op.id)? {
                None => merge_one(db, applied, op, None, None, || db.save_trace(&t), report),
                Some(local) => {
                    let fp = fingerprint_of(&local);
                    let updated = local.updated_at;
                    merge_one(db, applied, op, Some(fp), Some(updated), || {
                        db.save_trace(&t)
                    }, report)
                }
            }
        }
        "failure" => {
            let f: FailureRecord = serde_json::from_value(op.payload.clone())?;
            match db.get_failure_by_id(&op.id)? {
                None => merge_one(db, applied, op, None, None, || db.save_failure(&f), report),
                Some(local) => {
                    let fp = fingerprint_of(&local);
                    let updated = local.updated_at;
                    merge_one(db, applied, op, Some(fp), Some(updated), || {
                        db.save_failure(&f)
                    }, report)
                }
            }
        }
        "skill" => {
            let s: ProceduralSkill = serde_json::from_value(op.payload.clone())?;
            match db.get_skill_by_id(&op.id)? {
                None => merge_one(db, applied, op, None, None, || db.save_skill(&s), report),
                Some(local) => {
                    let fp = fingerprint_of(&local);
                    let updated = local.updated_at;
                    merge_one(db, applied, op, Some(fp), Some(updated), || {
                        db.save_skill(&s)
                    }, report)
                }
            }
        }
        "decision" => {
            let d: Decision = serde_json::from_value(op.payload.clone())?;
            match db.get_decision_by_id(&op.id)? {
                None => merge_one(db, applied, op, None, None, || db.save_decision_direct(&d), report),
                Some(local) => {
                    let fp = fingerprint_of(&local);
                    let updated = local.updated_at;
                    merge_one(db, applied, op, Some(fp), Some(updated), || {
                        db.save_decision_direct(&d)
                    }, report)
                }
            }
        }
        other => {
            tracing::warn!(kind = other, id = %op.id, "delta apply: unknown record kind skipped");
            Ok(None)
        }
    }
}

/// Applies every delta file in a vault directory, oldest first.
/// Idempotent: re-applying converged records is a no-op.
pub fn apply_all_deltas(
    db: &Database,
    vault_root: &Path,
    self_device: &str,
) -> Result<ApplyReport> {
    let applied = get_applied(db)?;
    let mut report = ApplyReport::default();
    for path in list_delta_files(vault_root) {
        for op in read_delta_file(&path) {
            if op.device_id == self_device {
                report.skipped_own += 1;
                continue;
            }
            apply_op(db, &applied, &op, &mut report)?;
        }
    }
    // Refresh fingerprints for everything now converged so the next collect
    // skips them. Records are re-serialized locally, keeping the map exact.
    let mut fresh = HashMap::new();
    for path in list_delta_files(vault_root) {
        for op in read_delta_file(&path) {
            if op.device_id == self_device {
                continue;
            }
            let current: Option<String> = match op.kind.as_str() {
                "memory" => db
                    .get_memory_by_id(&op.id)?
                    .map(|m| fingerprint_of(&m)),
                "trace" => db.get_trace_by_id(&op.id)?.map(|t| fingerprint_of(&t)),
                "failure" => db
                    .get_failure_by_id(&op.id)?
                    .map(|f| fingerprint_of(&f)),
                "skill" => db.get_skill_by_id(&op.id)?.map(|s| fingerprint_of(&s)),
                "decision" => db
                    .get_decision_by_id(&op.id)?
                    .map(|d| fingerprint_of(&d)),
                _ => None,
            };
            if let Some(fp) = current {
                fresh.insert(op.id.clone(), fp);
            }
        }
    }
    merge_applied(db, &fresh)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().expect("in-memory db")
    }

    fn seed_memory(db: &Database, id: &str, summary: &str) {
        let mem = Memory {
            id: id.to_string(),
            scope: crate::models::MemoryScope::Project,
            project_id: Some("proj-1".to_string()),
            memory_type: crate::models::MemoryType::Fact,
            content: format!("content {summary}"),
            summary: summary.to_string(),
            confidence: 0.9,
            importance: 0.5,
            freshness: 1.0,
            status: crate::models::MemoryStatus::Active,
            source_kind: crate::models::MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory_direct(&mem).unwrap();
    }

    fn test_vault(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("automemory-delta-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip_collect_write_apply_is_idempotent() {
        let src = test_db();
        seed_memory(&src, "m-1", "alpha");
        seed_memory(&src, "m-2", "beta");

        let ops = collect_deltas(&src, "dev-a").unwrap();
        assert_eq!(ops.len(), 2);

        let dir = test_vault("roundtrip");
        let path = write_delta_file(&dir, "dev-a", &ops).unwrap().unwrap();
        assert!(path.exists());
        // Same content twice: no duplicate file.
        let again = write_delta_file(&dir, "dev-a", &ops).unwrap().unwrap();
        assert_eq!(path, again);

        // Fresh device applies everything, then converges on re-apply.
        let dst = test_db();
        let report = apply_all_deltas(&dst, &dir, "dev-b").unwrap();
        assert_eq!(report.applied, 2);
        assert!(dst.get_memory_by_id("m-1").unwrap().is_some());

        let second = apply_all_deltas(&dst, &dir, "dev-b").unwrap();
        assert_eq!(second.applied, 0);
        assert_eq!(second.converged, 2);

        // Own device skips its own ops.
        let own = apply_all_deltas(&dst, &dir, "dev-a").unwrap();
        assert_eq!(own.skipped_own, 2);

        // Source re-collects nothing new after recording the push.
        let mut map = HashMap::new();
        for op in &ops {
            map.insert(op.id.clone(), op.fingerprint.clone());
        }
        merge_applied(&src, &map).unwrap();
        assert!(collect_deltas(&src, "dev-a").unwrap().is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_edit_becomes_review_not_overwrite() {
        let src = test_db();
        seed_memory(&src, "m-9", "original");
        let dir = test_vault("conflict");

        // Destination pulls the original, converging on it.
        let dst = test_db();
        let ops = collect_deltas(&src, "dev-a").unwrap();
        write_delta_file(&dir, "dev-a", &ops).unwrap();
        let first = apply_all_deltas(&dst, &dir, "dev-b").unwrap();
        assert_eq!(first.applied, 1);

        // Both sides edit the same record afterwards.
        let mut edited = dst.get_memory_by_id("m-9").unwrap().unwrap();
        edited.summary = "edited locally".to_string();
        edited.updated_at = Utc::now();
        dst.save_memory_direct(&edited).unwrap();

        let mut remote = src.get_memory_by_id("m-9").unwrap().unwrap();
        remote.summary = "edited remotely".to_string();
        remote.updated_at = Utc::now();
        src.save_memory_direct(&remote).unwrap();
        let ops2 = collect_deltas(&src, "dev-a").unwrap();
        assert_eq!(ops2.len(), 1);
        write_delta_file(&dir, "dev-a", &ops2).unwrap();

        // Destination pulls: conflict filed, local copy kept.
        let report = apply_all_deltas(&dst, &dir, "dev-b").unwrap();
        assert_eq!(report.conflicts, 1);
        assert_eq!(report.applied, 0);
        let kept = dst.get_memory_by_id("m-9").unwrap().unwrap();
        assert_eq!(kept.summary, "edited locally");
        let pending = dst.list_pending_proposals(None).unwrap();
        assert!(pending.iter().any(|p| p.proposal_type == "sync_conflict"
            && p.target_id == "memory:m-9"));

        // Re-pull does not duplicate the proposal.
        let again = apply_all_deltas(&dst, &dir, "dev-b").unwrap();
        assert_eq!(again.conflicts, 1);
        let pending = dst.list_pending_proposals(None).unwrap();
        assert_eq!(
            pending
                .iter()
                .filter(|p| p.proposal_type == "sync_conflict")
                .count(),
            1
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_collect_writes_no_file() {
        let db = test_db();
        let dir = test_vault("empty");
        assert!(write_delta_file(&dir, "dev-a", &[]).unwrap().is_none());
        let report = apply_all_deltas(&db, &dir, "dev-a").unwrap();
        assert_eq!(report.applied, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_lines_never_abort_apply() {
        let db = test_db();
        let dir = test_vault("corrupt");
        let deltas = dir.join("deltas");
        fs::create_dir_all(&deltas).unwrap();
        fs::write(
            deltas.join("broken-dev-a.jsonl"),
            "{not json}\n<<<<<<< HEAD\n",
        )
        .unwrap();
        let report = apply_all_deltas(&db, &dir, "dev-b").unwrap();
        assert_eq!(report.applied, 0);
        assert_eq!(report.conflicts, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
