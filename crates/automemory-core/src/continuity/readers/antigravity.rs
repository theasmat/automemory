//! Antigravity keeps each conversation as a SQLite database, `~/.gemini/<app>/conversations/<id>.db`
//! (`<app>` = `antigravity`, `antigravity-cli`, `antigravity-ide`), whose `steps` table holds protobuf payloads. Format seen: Oct 2026.
//! Only the fields this reader needs are decoded, by path, so unknown fields are ignored (notes: docs/research/antigravity_chat_format.md).
//!   step_type 14  user input         field 19 -> field 2 (text)
//!   step_type 15  planner response   field 20 -> field 1 (visible text), field 7 (repeated tool call: 2 name, 3 JSON arguments),
//!                                    field 3 is the model's private reasoning and is never read
//!   other types   tool results, injected hook messages, errors: skipped
//! The project of a conversation is the workspace folder in `trajectory_metadata_blob` (field 1 -> field 1, a file:// URI).
//! The conversation title is `title:"…"` in `<app>/annotations/<id>.pbtxt`; plans the agent wrote are `<app>/brain/<id>/*.md`.
use super::*;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

pub struct Antigravity;

const APPS: [&str; 3] = ["antigravity", "antigravity-cli", "antigravity-ide"];

/// Minimal protobuf reading: the length-delimited and varint fields of one message.
pub mod pb {
    #[derive(Debug, Clone, PartialEq)]
    pub enum Val<'a> {
        Varint(u64),
        Bytes(&'a [u8]),
    }

    fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
        let (mut r, mut shift) = (0u64, 0);
        loop {
            let x = *b.get(*i)?;
            *i += 1;
            r |= u64::from(x & 0x7f) << shift;
            if x & 0x80 == 0 {
                return Some(r);
            }
            shift += 7;
            if shift > 63 {
                return None;
            }
        }
    }

    /// Every (field number, value) of a message, in order; stops quietly at bytes that are not valid protobuf.
    pub fn fields(b: &[u8]) -> Vec<(u32, Val<'_>)> {
        let (mut out, mut i) = (Vec::new(), 0);
        while i < b.len() {
            let Some(tag) = varint(b, &mut i) else { break };
            let field = (tag >> 3) as u32;
            match tag & 7 {
                0 => match varint(b, &mut i) { Some(v) => out.push((field, Val::Varint(v))), None => break },
                1 => i += 8,
                5 => i += 4,
                2 => {
                    let Some(n) = varint(b, &mut i) else { break };
                    let end = i.saturating_add(n as usize);
                    let Some(chunk) = b.get(i..end) else { break };
                    out.push((field, Val::Bytes(chunk)));
                    i = end;
                }
                _ => break,
            }
        }
        out
    }

    /// The first value at a path of field numbers, descending through length-delimited submessages.
    pub fn at<'a>(b: &'a [u8], path: &[u32]) -> Option<Val<'a>> {
        let (first, rest) = path.split_first()?;
        let v = fields(b).into_iter().find(|(f, _)| f == first)?.1;
        if rest.is_empty() { Some(v) } else if let Val::Bytes(inner) = v { at(inner, rest) } else { None }
    }

    pub fn text<'a>(b: &'a [u8], path: &[u32]) -> Option<String> {
        match at(b, path)? {
            Val::Bytes(s) => std::str::from_utf8(s).ok().map(String::from),
            _ => None,
        }
    }

    /// Encoding for fixtures and tests.
    #[cfg(test)]
    pub fn field(num: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut put = |mut v: u64| loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 { out.push(byte); break } else { out.push(byte | 0x80) }
        };
        put(u64::from(num) << 3 | 2);
        put(payload.len() as u64);
        out.extend_from_slice(payload);
        out
    }

    #[cfg(test)]
    pub fn varint_field(num: u32, v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        let mut put = |mut x: u64| loop {
            let byte = (x & 0x7f) as u8;
            x >>= 7;
            if x == 0 { out.push(byte); break } else { out.push(byte | 0x80) }
        };
        put(u64::from(num) << 3);
        put(v);
        out
    }
}

fn open_ro(path: &Path) -> Option<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let uri = |q: &str| format!("file:{}?{q}", path.to_string_lossy().replace('?', "%3f").replace('#', "%23"));
    // A database the app has open in WAL mode may refuse plain read-only; `immutable` still reads the main file.
    Connection::open_with_flags(uri("mode=ro"), flags).ok().filter(|c| c.query_row("SELECT 1 FROM sqlite_master LIMIT 1", [], |_| Ok(())).is_ok())
        .or_else(|| Connection::open_with_flags(uri("immutable=1"), flags).ok())
}

fn workspace(conn: &Connection) -> Option<String> {
    let blob: Vec<u8> = conn.query_row("SELECT data FROM trajectory_metadata_blob LIMIT 1", [], |r| r.get(0)).ok()?;
    pb::text(&blob, &[1, 1]).map(|u| u.trim_start_matches("file://").trim_end_matches('/').to_string())
}

fn when(payload: &[u8]) -> Option<DateTime<Utc>> {
    match pb::at(payload, &[5, 1, 1])? {
        pb::Val::Varint(s) => DateTime::from_timestamp(s as i64, 0),
        _ => None,
    }
}

/// `→ read_file /path`, `→ run_command cargo test`.
fn tool_line(call: &[u8]) -> Option<String> {
    let name = pb::text(call, &[2])?;
    let args: Value = pb::text(call, &[3]).and_then(|a| serde_json::from_str(&a).ok()).unwrap_or(Value::Null);
    let detail = ["CommandLine", "Path", "AbsolutePath", "TargetFile", "SearchPath", "Query", "Url", "Pattern"].iter().find_map(|k| args.get(*k).and_then(|v| v.as_str())).unwrap_or("");
    let detail: String = detail.lines().next().unwrap_or("").chars().take(100).collect();
    Some(format!("→ {name} {detail}").trim_end().to_string())
}

fn app_dir(db_path: &Path) -> Option<&Path> {
    db_path.parent()?.parent()
}

impl TranscriptReader for Antigravity {
    fn id(&self) -> &'static str {
        "agy"
    }

    fn label(&self) -> &'static str {
        "Antigravity"
    }

    fn locations(&self, home: &Path) -> Vec<PathBuf> {
        APPS.iter().map(|a| home.join(".gemini").join(a)).collect()
    }

    fn sessions(&self, home: &Path, project_root: &Path) -> Vec<SessionRef> {
        let want = project_root.to_string_lossy().trim_end_matches('/').to_string();
        let mut out = Vec::new();
        for app in APPS {
            let dir = home.join(".gemini").join(app).join("conversations");
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = e.path();
                if path.extension().is_none_or(|x| x != "db") {
                    continue;
                }
                let Some(conn) = open_ro(&path) else { continue };
                if workspace(&conn).as_deref() == Some(want.as_str()) {
                    out.push(SessionRef {
                        native_id: path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                        modified: e.metadata().ok().and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from),
                        path,
                    });
                }
            }
        }
        out.sort_by_key(|s| s.modified);
        out
    }

    fn read(&self, session: &SessionRef, since: Option<&Cursor>) -> Result<ReadResult> {
        // Cursor: "<last step index>;<newest plan file time>".
        let (last_idx, last_art) = since
            .and_then(|c| c.0.split_once(';'))
            .map(|(a, b)| (a.parse::<i64>().unwrap_or(-1), b.parse::<i64>().unwrap_or(0)))
            .unwrap_or((-1, 0));
        let conn = open_ro(&session.path).ok_or_else(|| anyhow::anyhow!("cannot open {}", session.path.display()))?;
        let mut result = ReadResult::default();
        let (mut max_idx, mut max_art) = (last_idx, last_art);

        let mut stmt = conn.prepare("SELECT idx, step_type, step_payload FROM steps WHERE idx > ?1 ORDER BY idx")?;
        let rows = stmt.query_map([last_idx], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Vec<u8>>(2).unwrap_or_default())))?;
        for row in rows.flatten() {
            let (idx, step_type, payload) = row;
            max_idx = max_idx.max(idx);
            let ts = when(&payload);
            match step_type {
                14 => {
                    if let Some(text) = pb::text(&payload, &[19, 2]).or_else(|| pb::text(&payload, &[19, 3, 1])).filter(|t| !t.trim().is_empty()) {
                        result.messages.push(RawMessage { role: Role::User, ts, text });
                    }
                }
                15 => {
                    // Field 3 is the model's reasoning summary: private, never read.
                    let Some(Some(body)) = pb::at(&payload, &[20]).map(|v| if let pb::Val::Bytes(b) = v { Some(b) } else { None }) else { continue };
                    if let Some(text) = pb::text(body, &[1]).filter(|t| !t.trim().is_empty()) {
                        result.messages.push(RawMessage { role: Role::Assistant, ts, text });
                    }
                    for (f, v) in pb::fields(body) {
                        if let (7, pb::Val::Bytes(call)) = (f, v) {
                            if let Some(text) = tool_line(call) {
                                result.messages.push(RawMessage { role: Role::Tool, ts, text });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // The conversation's title and the plans the agent wrote for it (already the agent's own summaries).
        if let Some(app) = app_dir(&session.path) {
            if let Ok(t) = std::fs::read_to_string(app.join("annotations").join(format!("{}.pbtxt", session.native_id))) {
                result.title = t.split("title:\"").nth(1).and_then(|s| s.split('"').next()).map(String::from).filter(|s| !s.is_empty());
            }
            for e in std::fs::read_dir(app.join("brain").join(&session.native_id)).into_iter().flatten().flatten() {
                let path = e.path();
                if path.extension().is_none_or(|x| x != "md") {
                    continue;
                }
                let secs = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                if secs > last_art {
                    max_art = max_art.max(secs);
                    if let Ok(body) = std::fs::read_to_string(&path) {
                        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        result.messages.push(RawMessage { role: Role::Artifact, ts: DateTime::from_timestamp(secs, 0), text: format!("[plan {name}]\n{body}") });
                    }
                }
            }
        }
        result.cursor = Some(Cursor(format!("{max_idx};{max_art}")));
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/antigravity/home")
    }

    fn next_idx(conn: &Connection) -> i64 {
        conn.query_row("SELECT COALESCE(MAX(idx), -1) + 1 FROM steps", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn passes_the_reader_contract() {
        let append = |path: &Path| {
            // The agent adds one reply; a damaged step (bytes that are not protobuf) sits before it.
            let conn = Connection::open(path).unwrap();
            let idx = next_idx(&conn);
            conn.execute("INSERT INTO steps (idx, step_type, step_payload) VALUES (?1, 15, ?2)", rusqlite::params![idx, vec![0xffu8, 0xff, 0xff, 0x01]]).unwrap();
            let reply = [pb::varint_field(1, 15), pb::field(20, &pb::field(1, b"One more message appended later."))].concat();
            conn.execute("INSERT INTO steps (idx, step_type, step_payload) VALUES (?1, 15, ?2)", rusqlite::params![idx + 1, reply]).unwrap();
        };
        super::super::testkit::conformance(&Antigravity, &fixture(), Path::new("/tmp/demo-shop"), &append, "FIXTURESECRET1234567890");
    }

    #[test]
    fn keeps_the_conversation_and_the_agents_plan_and_drops_the_rest() {
        let s = Antigravity.sessions(&fixture(), Path::new("/tmp/demo-shop"));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].native_id, "agy-sess-001");
        assert!(Antigravity.sessions(&fixture(), Path::new("/tmp/other-project")).is_empty(), "a conversation belongs to its workspace only");
        let r = Antigravity.read(&s[0], None).unwrap();
        let joined: String = r.messages.iter().map(|m| format!("{}: {}", m.role.as_str(), m.text)).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("user: We want a shopping cart") && joined.contains("assistant: I suggest SQLite keyed by a cookie id"), "{joined}");
        assert!(joined.contains("tool: → read_file /Users/someone/demo-shop/shop/cart.py") && joined.contains("tool: → run_command python3 -m unittest"), "{joined}");
        assert!(joined.contains("artifact: [plan plan.md]") && joined.contains("Ruled out: server-side sessions"), "{joined}");
        for dropped in ["PRIVATE REASONING", "TOOL OUTPUT", "injected hook message", "network issue", "metadata.json"] {
            assert!(!joined.contains(dropped), "{dropped} must not be kept: {joined}");
        }
        assert_eq!(r.title.as_deref(), Some("Plan the demo shop cart"));
        // Reading again from the cursor finds nothing, plan files included.
        assert!(Antigravity.read(&s[0], r.cursor.as_ref()).unwrap().messages.is_empty());
    }

    #[test]
    fn protobuf_fields_are_read_by_path_and_garbage_is_survived() {
        let msg = [pb::varint_field(1, 7), pb::field(19, &[pb::field(2, b"hello"), pb::varint_field(9, 300)].concat())].concat();
        assert_eq!(pb::text(&msg, &[19, 2]).as_deref(), Some("hello"));
        assert_eq!(pb::at(&msg, &[19, 9]), Some(pb::Val::Varint(300)));
        assert!(pb::text(&msg, &[19, 5]).is_none() && pb::text(&msg, &[4, 1]).is_none());
        for junk in [&[][..], &[0xff][..], &[0x0a, 0x7f, 0x01][..], &[0x08][..]] {
            let _ = pb::fields(junk);
            assert!(pb::text(junk, &[1, 2]).is_none());
        }
    }
}
