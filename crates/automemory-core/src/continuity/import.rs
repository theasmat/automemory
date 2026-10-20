//! Turns what a reader found into archive rows: redacted, clipped, each message once. Opt-in is the caller's job (CLI and
//! hooks check `[continuity] read_chats` before calling).
use super::archive::session_id;
use super::readers::{RawMessage, TranscriptReader};
use crate::db::Database;
use crate::redaction::redact_text;
use anyhow::Result;
use std::path::Path;

/// One message is clipped to this many characters (a pasted log is not worth keeping whole).
pub const MESSAGE_MAX: usize = 4000;

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct ImportReport {
    pub sessions: usize,
    pub new_messages: usize,
}

/// Secrets removed and the home folder shown as `~`, then clipped; `None` when nothing is left.
pub fn clean(text: &str, home: &Path) -> Option<String> {
    let home = home.to_string_lossy();
    let scrubbed = if home.len() > 1 { text.replace(home.as_ref(), "~") } else { text.to_string() };
    let redacted = redact_text(scrubbed.trim());
    let t = redacted.trim();
    if t.is_empty() {
        return None;
    }
    Some(if t.chars().count() > MESSAGE_MAX { format!("{}…", t.chars().take(MESSAGE_MAX).collect::<String>()) } else { t.to_string() })
}

/// Reads the project's new chat messages with `reader` and stores them. Safe to run again: only what is new is added.
pub fn import_project(db: &Database, reader: &dyn TranscriptReader, home: &Path, project_id: Option<&str>, project_root: &Path) -> Result<ImportReport> {
    let mut report = ImportReport::default();
    for s in reader.sessions(home, project_root) {
        let id = session_id(reader.id(), &s.native_id);
        let known = db.chat_session(&id)?;
        let since = known.as_ref().and_then(|k| k.cursor.clone()).map(super::readers::Cursor);
        let read = reader.read(&s, since.as_ref())?;
        let msgs: Vec<(String, Option<chrono::DateTime<chrono::Utc>>, String)> = read
            .messages
            .iter()
            .filter_map(|m: &RawMessage| clean(&m.text, home).map(|t| (m.role.as_str().to_string(), m.ts, t)))
            .collect();
        let cursor_moved = read.cursor.as_ref().map(|c| c.0.clone()) != known.as_ref().and_then(|k| k.cursor.clone());
        if msgs.is_empty() && (known.is_some() || !cursor_moved) {
            if cursor_moved {
                db.chat_append(reader.id(), &s.native_id, project_id, read.title.as_deref(), &read.cursor.map(|c| c.0).unwrap_or_default(), &[])?;
            }
            continue;
        }
        let n = db.chat_append(reader.id(), &s.native_id, project_id, read.title.as_deref(), &read.cursor.map(|c| c.0).unwrap_or_default(), &msgs)?;
        report.sessions += 1;
        report.new_messages += n;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::readers::{claude_code::ClaudeCode, TranscriptReader};

    #[test]
    fn cleaning_hides_secrets_and_the_home_folder_and_clips() {
        let home = Path::new("/Users/alice");
        let t = clean("see /Users/alice/shop/a.py and key sk-abcdefghijklmnopqrstuvwxyz1234", home).unwrap();
        assert!(t.contains("~/shop/a.py") && !t.contains("alice") && !t.contains("abcdefghijklmnopqrstuvwxyz"), "{t}");
        assert!(clean("   ", home).is_none());
        assert!(clean(&"x".repeat(9000), home).unwrap().chars().count() <= MESSAGE_MAX + 1);
    }

    #[test]
    fn the_archive_is_searchable_by_word_and_forgettable() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/claude/home");
        let db = Database::open_in_memory().unwrap();
        let r = import_project(&db, &ClaudeCode, &fixture, Some("p"), Path::new("/tmp/demo-shop")).unwrap();
        assert!(r.sessions == 1 && r.new_messages >= 4, "{r:?}");

        let hits = db.chat_search("sticky routing", Some("p"), None, 5).unwrap();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].snippet.contains("[sticky]") || hits[0].snippet.contains("sticky"), "{}", hits[0].snippet);
        assert_eq!((hits[0].agent.as_str(), hits[0].title.as_deref()), ("claude", Some("Plan the demo shop cart")));
        assert!(db.chat_search("sticky", Some("other-project"), None, 5).unwrap().is_empty(), "scoped to the project");
        assert!(db.chat_search("PRIVATE REASONING", None, None, 5).unwrap().is_empty(), "thinking was never stored");

        let around = db.chat_get(&hits[0].session, (hits[0].seq - 1).max(0), 3).unwrap();
        assert!(around.len() >= 2 && around.windows(2).all(|w| w[0].seq < w[1].seq), "{around:?}");

        assert!(db.chat_forget(Some("p"), Some("claude")).unwrap() >= 4);
        assert!(db.chat_search("sticky", None, None, 5).unwrap().is_empty(), "forgotten text is gone from the index too");
        assert!(db.chat_sessions(None, None).unwrap().is_empty());
        // After forgetting, importing again starts from the beginning (the cursor went with the session).
        assert!(import_project(&db, &ClaudeCode, &fixture, Some("p"), Path::new("/tmp/demo-shop")).unwrap().new_messages >= 4);
        let _ = ClaudeCode.id();
    }
}
