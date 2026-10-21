//! Cursor (the IDE) keeps chats in SQLite: `<Cursor user folder>/globalStorage/state.vscdb`, table `cursorDiskKV`, with `composerData:<id>` (the conversation: name and the
//! ordered `fullConversationHeadersOnly`) and `bubbleId:<id>:<bubble>` rows (one message: `type` 1 user / 2 assistant, `text`, `createdAt` in ms). Which folder a chat
//! belongs to comes from `workspaceStorage/<id>/workspace.json` (`folder`) and that workspace's `composer.composerData` index (Cursor 3.0+ also keeps a central
//! `composer.composerHeaders` in the global `ItemTable`). Read-only: the database is opened read-only (never copied or written).
//! Format source: public reverse-engineering notes (Oct 2026); Cursor publishes no schema and changes it between versions, and there is **no Cursor data on the
//! maintainer's machine**, so this is unverified (docs/research/cursor_chat_format.md). Agent transcripts under `~/.cursor/projects/*/agent-transcripts/` are not read.
use super::antigravity::open_ro;
use super::{Cursor as Cursor_, *};
use rusqlite::Connection;
use serde_json::Value;

pub struct CursorIde;

fn user_dirs(home: &Path) -> Vec<PathBuf> {
    vec![home.join("Library/Application Support/Cursor/User"), home.join(".config/Cursor/User")]
}

fn kv_get(conn: &Connection, table: &str, key: &str) -> Option<Value> {
    let bytes: Vec<u8> = conn.query_row(&format!("SELECT CAST(value AS BLOB) FROM {table} WHERE key = ?1"), [key], |r| r.get(0)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn folder_of(workspace_dir: &Path) -> Option<String> {
    let v: Value = serde_json::from_slice(&std::fs::read(workspace_dir.join("workspace.json")).ok()?).ok()?;
    let uri = v.get("folder")?.as_str()?;
    let path = uri.strip_prefix("file://")?;
    Some(path.replace("%20", " ").trim_end_matches('/').to_string())
}

/// The ordered bubble ids of a conversation (newer `fullConversationHeadersOnly`, older inline `conversation`).
fn headers(composer: &Value) -> Vec<Value> {
    composer.get("fullConversationHeadersOnly").and_then(|h| h.as_array()).cloned().unwrap_or_default()
}

fn tool_line(b: &Value) -> Option<String> {
    let t = b.get("toolFormerData")?;
    let name = t.get("name").and_then(|n| n.as_str())?;
    let args: Value = match t.get("rawArgs").or_else(|| t.get("params")) {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(Value::Null),
        Some(v) => v.clone(),
        None => Value::Null,
    };
    Some(util::tool_note(name, &util::arg_detail(&args, &["command", "target_file", "file_path", "path", "query", "pattern"])))
}

fn message(b: &Value, out: &mut Vec<RawMessage>) {
    let ts = b.get("createdAt").and_then(|t| t.as_i64()).and_then(DateTime::<Utc>::from_timestamp_millis);
    let text = b.get("text").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
    match b.get("type").and_then(|t| t.as_i64()) {
        Some(1) if !text.is_empty() => out.push(RawMessage { role: Role::User, ts, text }),
        Some(2) => {
            if !text.is_empty() {
                out.push(RawMessage { role: Role::Assistant, ts, text });
            }
            if let Some(text) = tool_line(b) {
                out.push(RawMessage { role: Role::Tool, ts, text });
            }
        }
        _ => {}
    }
}

impl TranscriptReader for CursorIde {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn label(&self) -> &'static str {
        "Cursor"
    }

    fn locations(&self, home: &Path) -> Vec<PathBuf> {
        user_dirs(home)
    }

    fn sessions(&self, home: &Path, project_root: &Path) -> Vec<SessionRef> {
        let mut out = Vec::new();
        for user in user_dirs(home) {
            let global = user.join("globalStorage").join("state.vscdb");
            if !global.is_file() {
                continue;
            }
            let central = open_ro(&global).and_then(|c| kv_get(&c, "ItemTable", "composer.composerHeaders"));
            for ws in std::fs::read_dir(user.join("workspaceStorage")).into_iter().flatten().flatten() {
                let Some(folder) = folder_of(&ws.path()) else { continue };
                if !util::same_project(&folder, project_root) {
                    continue;
                }
                let mut ids: Vec<String> = Vec::new();
                if let Some(idx) = open_ro(&ws.path().join("state.vscdb")).and_then(|c| kv_get(&c, "ItemTable", "composer.composerData")) {
                    ids.extend(idx.get("allComposers").and_then(|a| a.as_array()).into_iter().flatten().filter_map(|c| c.get("composerId").and_then(|i| i.as_str()).map(String::from)));
                }
                // Cursor 3.0+: a central index names the workspace each chat belongs to.
                let wid = ws.file_name().to_string_lossy().to_string();
                let list = central.as_ref().and_then(|c| c.get("allComposers").or(Some(c))).and_then(|a| a.as_array());
                ids.extend(list.into_iter().flatten().filter(|c| c.get("workspaceIdentifier").and_then(|w| w.get("id")).and_then(|i| i.as_str()) == Some(wid.as_str())).filter_map(|c| c.get("composerId").and_then(|i| i.as_str()).map(String::from)));
                ids.sort();
                ids.dedup();
                let modified = std::fs::metadata(&global).ok().and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from);
                out.extend(ids.into_iter().map(|id| SessionRef { native_id: id, path: global.clone(), modified }));
            }
        }
        out
    }

    fn read(&self, session: &SessionRef, since: Option<&Cursor_>) -> Result<ReadResult> {
        let conn = open_ro(&session.path).ok_or_else(|| anyhow::anyhow!("cannot open Cursor's database read-only (is it locked?)"))?;
        let Some(composer) = kv_get(&conn, "cursorDiskKV", &format!("composerData:{}", session.native_id)) else {
            return Ok(ReadResult { cursor: since.cloned(), ..Default::default() });
        };
        let all = headers(&composer);
        let seen: usize = since.and_then(|c| c.0.strip_prefix("n:")).and_then(|n| n.parse().ok()).unwrap_or(0);
        let seen = if seen > all.len() { 0 } else { seen };
        let mut result = ReadResult { cursor: Some(Cursor_(format!("n:{}", all.len()))), title: composer.get("name").and_then(|n| n.as_str()).map(String::from), ..Default::default() };
        for h in &all[seen..] {
            let Some(bid) = h.get("bubbleId").and_then(|b| b.as_str()) else { continue };
            if let Some(b) = kv_get(&conn, "cursorDiskKV", &format!("bubbleId:{}:{bid}", session.native_id)) {
                message(&b, &mut result.messages);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/cursor/home")
    }

    #[test]
    fn passes_the_reader_contract() {
        let append = |path: &Path| {
            let c = Connection::open(path).unwrap();
            let cid = "cmp-001";
            c.execute("INSERT INTO cursorDiskKV VALUES (?1, ?2)", rusqlite::params![format!("bubbleId:{cid}:b-new"), r#"{"type":2,"text":"One more message appended later.","createdAt":1759309300000}"#]).unwrap();
            let mut data: Value = serde_json::from_slice(&c.query_row("SELECT CAST(value AS BLOB) FROM cursorDiskKV WHERE key = 'composerData:cmp-001'", [], |r| r.get::<_, Vec<u8>>(0)).unwrap()).unwrap();
            data["fullConversationHeadersOnly"].as_array_mut().unwrap().push(serde_json::json!({"bubbleId": "b-new", "type": 2}));
            c.execute("UPDATE cursorDiskKV SET value = ?1 WHERE key = 'composerData:cmp-001'", [data.to_string()]).unwrap();
        };
        super::super::testkit::conformance(&CursorIde, &fixture(), Path::new("/tmp/demo-shop"), &append, "FIXTURESECRET1234567890");
    }

    #[test]
    fn keeps_the_conversation_and_drops_everything_else() {
        let s = CursorIde.sessions(&fixture(), Path::new("/tmp/demo-shop"));
        assert_eq!(s.iter().map(|s| s.native_id.as_str()).collect::<Vec<_>>(), vec!["cmp-001"], "only the chat of this folder's workspace");
        let r = CursorIde.read(&s[0], None).unwrap();
        let joined = r.messages.iter().map(|m| format!("{}: {}", m.role.as_str(), m.text)).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("user: We want a shopping cart") && joined.contains("sticky routing") && joined.contains("tool: → run_terminal_cmd cargo test"), "{joined}");
        for never in ["PRIVATE REASONING", "TOOL RESULT", "A chat with no workspace", "another project"] {
            assert!(!joined.contains(never), "{never} must not be kept:\n{joined}");
        }
        assert_eq!(r.title.as_deref(), Some("Plan the demo shop cart"));
        assert!(CursorIde.sessions(&fixture(), Path::new("/tmp/nowhere")).is_empty());
    }
}
