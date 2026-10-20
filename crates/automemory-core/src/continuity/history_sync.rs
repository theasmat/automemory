//! Moving chat history between your computers. Never automatic: only `automemory continuity sync-history` writes or reads
//! these files. One encrypted file per session (`chat/<session>.enc` in the vault, same key as the rest of the vault).
//! Messages were redacted when they were imported; the file adds nothing raw.
use super::archive::ChatMessage;
use crate::crypto::{BlindCrypto, EncryptedBlob};
use crate::db::Database;
use anyhow::{bail, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct HistoryFile {
    version: u8,
    /// The project's identity across computers (git remote), not the local id.
    project_key: String,
    agent: String,
    native_id: String,
    title: Option<String>,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Default, Serialize)]
pub struct HistoryPlan {
    /// (session id, messages, bytes of the encrypted file)
    pub sessions: Vec<(String, usize, usize)>,
    pub no_project_key: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct PullReport {
    pub sessions: usize,
    pub messages: usize,
    pub already_here: usize,
    pub unknown_project: usize,
    pub unreadable: usize,
}

fn dir(vault: &Path) -> PathBuf {
    vault.join("chat")
}

fn file_name(session: &str) -> String {
    let safe: String = session.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("{safe}.enc")
}

fn build(db: &Database, project_id: &str) -> Result<Vec<(String, HistoryFile)>> {
    let Some(project_key) = db.project_key_of(project_id) else { bail!("this project has no identity yet (it needs a git remote): history cannot be matched on another computer") };
    let mut out = Vec::new();
    for s in db.chat_sessions(Some(project_id), None)? {
        let messages = db.chat_get(&s.id, 0, 100_000)?;
        out.push((s.id.clone(), HistoryFile { version: 1, project_key: project_key.clone(), agent: s.agent, native_id: s.native_id, title: s.title, messages }));
    }
    Ok(out)
}

/// What a push would send, with sizes. Writes nothing.
pub fn plan(db: &Database, project_id: &str, key: &[u8; 32]) -> Result<HistoryPlan> {
    let mut p = HistoryPlan::default();
    for (id, f) in build(db, project_id)? {
        let bytes = serde_json::to_vec(&BlindCrypto::encrypt(key, &serde_json::to_vec(&f)?)?)?.len();
        p.sessions.push((id, f.messages.len(), bytes));
    }
    Ok(p)
}

/// Writes the project's sessions into `<vault>/chat/`. Returns the number of files.
pub fn write(db: &Database, project_id: &str, vault: &Path, key: &[u8; 32]) -> Result<usize> {
    std::fs::create_dir_all(dir(vault))?;
    let all = build(db, project_id)?;
    for (id, f) in &all {
        let bytes = serde_json::to_vec(&BlindCrypto::encrypt(key, &serde_json::to_vec(f)?)?)?;
        let path = dir(vault).join(file_name(id));
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(all.len())
}

/// Reads `<vault>/chat/*.enc` into the archive. Sessions of projects this computer does not have, files that do not
/// decrypt with this key, and sessions already here are skipped.
pub fn read(db: &Database, vault: &Path, key: &[u8; 32]) -> Result<PullReport> {
    let mut r = PullReport::default();
    let Ok(entries) = std::fs::read_dir(dir(vault)) else { return Ok(r) };
    for e in entries.flatten().filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("enc")) {
        let file = std::fs::read(e.path())
            .ok()
            .and_then(|b| serde_json::from_slice::<EncryptedBlob>(&b).ok())
            .and_then(|b| BlindCrypto::decrypt(key, &b).ok())
            .and_then(|p| serde_json::from_slice::<HistoryFile>(&p).ok())
            .filter(|f| f.version == 1);
        let Some(f) = file else { r.unreadable += 1; continue };
        let Some(pid) = db.project_id_for_key(&f.project_key) else { r.unknown_project += 1; continue };
        if db.chat_session(&super::archive::session_id(&f.agent, &f.native_id))?.is_some() {
            r.already_here += 1;
            continue;
        }
        let msgs: Vec<_> = f.messages.iter().map(|m| (m.role.clone(), m.ts.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.to_utc()), m.text.clone())).collect();
        // Empty cursor: a local reader that later finds the same session starts from the top and the text is deduplicated by its id.
        let n = db.chat_append(&f.agent, &f.native_id, Some(&pid), f.title.as_deref(), "", &msgs)?;
        r.sessions += 1;
        r.messages += n;
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(db: &Database, id: &str, key: &str) {
        let now = chrono::Utc::now();
        db.save_project(&crate::models::Project {
            id: id.into(), slug: id.into(), display_name: id.into(), created_at: now, updated_at: now,
            capture_policy: Default::default(), context_budget: Default::default(), document_config: Default::default(), sync_config: Default::default(),
        }).unwrap();
        db.set_project_key(id, key).unwrap();
    }

    #[test]
    fn history_moves_only_through_the_files_and_only_with_the_key() {
        let (a, b) = (Database::open_in_memory().unwrap(), Database::open_in_memory().unwrap());
        project(&a, "pa", "git:github.com/me/shop");
        project(&b, "pb", "git:github.com/me/shop");
        a.chat_append("claude", "s1", Some("pa"), Some("Cart"), "c1", &[("user".into(), None, "We want a cart".into()), ("assistant".into(), None, "Use SQLite".into())]).unwrap();
        let vault = std::env::temp_dir().join(format!("am-hist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&vault);
        let key = [7u8; 32];

        let p = plan(&a, "pa", &key).unwrap();
        assert_eq!(p.sessions.len(), 1);
        assert!(!vault.join("chat").exists(), "planning writes nothing");
        assert_eq!(write(&a, "pa", &vault, &key).unwrap(), 1);
        let raw = std::fs::read_to_string(vault.join("chat/claude_s1.enc")).unwrap();
        assert!(!raw.contains("SQLite") && !raw.contains("cart"), "the file is ciphertext");

        assert_eq!(read(&b, &vault, &[9u8; 32]).unwrap().unreadable, 1, "another key reads nothing");
        let r = read(&b, &vault, &key).unwrap();
        assert_eq!((r.sessions, r.messages), (1, 2), "{r:?}");
        assert_eq!(b.chat_search("sqlite", Some("pb"), None, 5).unwrap().len(), 1, "searchable under the local project id");
        assert_eq!(read(&b, &vault, &key).unwrap().already_here, 1, "twice adds nothing");
        let c = Database::open_in_memory().unwrap();
        assert_eq!(read(&c, &vault, &key).unwrap().unknown_project, 1, "a computer without the project skips it");
        let _ = std::fs::remove_dir_all(&vault);
    }
}
