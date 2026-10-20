//! Claude Code keeps each session as JSON lines in `~/.claude/projects/<project folder>/<session>.jsonl`
//! (project folder = the path with every non-alphanumeric character replaced by `-`). Format seen: Oct 2026, v2.x; unknown
//! fields and lines are skipped.
use super::*;
use regex::Regex;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::sync::OnceLock;

pub struct ClaudeCode;

/// `/Users/me/shop` -> `-Users-me-shop`.
fn folder_name(project_root: &Path) -> String {
    project_root.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

fn noise() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<system-reminder>.*?</system-reminder>").unwrap())
}

/// A tool call as one short line: `→ Edit src/a.rs`, `→ Bash cargo test`.
fn tool_line(block: &Value) -> Option<String> {
    let name = block.get("name")?.as_str()?;
    let input = block.get("input")?;
    let detail = ["file_path", "path", "command", "pattern", "url", "description"].iter().find_map(|k| input.get(*k).and_then(|v| v.as_str())).unwrap_or("");
    let detail: String = detail.lines().next().unwrap_or("").chars().take(100).collect();
    Some(format!("→ {name} {detail}").trim_end().to_string())
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks.iter().filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")).filter_map(|b| b.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

impl TranscriptReader for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn label(&self) -> &'static str {
        "Claude Code"
    }

    fn locations(&self, home: &Path) -> Vec<PathBuf> {
        vec![home.join(".claude").join("projects")]
    }

    fn sessions(&self, home: &Path, project_root: &Path) -> Vec<SessionRef> {
        let dir = home.join(".claude").join("projects").join(folder_name(project_root));
        let mut out: Vec<SessionRef> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .filter_map(|e| {
                let path = e.path();
                Some(SessionRef {
                    native_id: path.file_stem()?.to_string_lossy().to_string(),
                    modified: e.metadata().ok().and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from),
                    path,
                })
            })
            .collect();
        out.sort_by_key(|s| s.modified);
        out
    }

    fn read(&self, session: &SessionRef, since: Option<&Cursor>) -> Result<ReadResult> {
        let start: u64 = since.and_then(|c| c.0.parse().ok()).unwrap_or(0);
        let mut file = std::fs::File::open(&session.path)?;
        let len = file.metadata()?.len();
        let start = if start > len { 0 } else { start }; // the file was replaced by a shorter one: start over
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        // Only whole lines count; a half-written last line is read next time.
        let complete = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        let mut result = ReadResult { cursor: Some(Cursor((start + complete as u64).to_string())), ..Default::default() };
        for line in String::from_utf8_lossy(&bytes[..complete]).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let ts = v.get("timestamp").and_then(|t| t.as_str()).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc));
            let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if kind == "ai-title" {
                result.title = v.get("aiTitle").and_then(|t| t.as_str()).map(String::from);
                continue;
            }
            // Meta entries (command echoes, injected context) and sub-agent side chains are not the conversation.
            if v.get("isMeta").and_then(|m| m.as_bool()) == Some(true) || v.get("isSidechain").and_then(|m| m.as_bool()) == Some(true) {
                continue;
            }
            let Some(message) = v.get("message") else { continue };
            let content = message.get("content").unwrap_or(&Value::Null);
            match kind {
                "user" => {
                    let text = noise().replace_all(&text_of(content), "").trim().to_string();
                    if !text.is_empty() && !text.starts_with("<local-command-") && !text.starts_with("<command-") {
                        result.messages.push(RawMessage { role: Role::User, ts, text });
                    }
                }
                "assistant" => {
                    // Thinking blocks are the agent's private reasoning and are never kept.
                    let text = text_of(content).trim().to_string();
                    if !text.is_empty() {
                        result.messages.push(RawMessage { role: Role::Assistant, ts, text });
                    }
                    if let Some(blocks) = content.as_array() {
                        for b in blocks.iter().filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use")) {
                            if let Some(text) = tool_line(b) {
                                result.messages.push(RawMessage { role: Role::Tool, ts, text });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/claude/home")
    }

    #[test]
    fn passes_the_reader_contract() {
        let line = r#"{"type":"assistant","isSidechain":false,"sessionId":"sess-001","timestamp":"2026-10-01T09:05:00.000Z","message":{"role":"assistant","content":[{"type":"text","text":"One more message appended later."}]}}"#;
        let append = |path: &Path| {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
            writeln!(f, "this line is not valid for any format {{{{").unwrap();
            writeln!(f, "{line}").unwrap();
        };
        super::super::testkit::conformance(&ClaudeCode, &fixture(), Path::new("/tmp/demo-shop"), &append, "FIXTURESECRET1234567890");
    }

    #[test]
    fn keeps_the_conversation_and_drops_everything_else() {
        let s = ClaudeCode.sessions(&fixture(), Path::new("/tmp/demo-shop"));
        let r = ClaudeCode.read(&s[0], None).unwrap();
        let all: Vec<String> = r.messages.iter().map(|m| format!("{}: {}", m.role.as_str(), m.text)).collect();
        let joined = all.join("\n");
        assert!(joined.contains("user: We want a shopping cart") && joined.contains("assistant: I suggest storing carts in SQLite"), "{joined}");
        assert!(joined.contains("tool: → Edit /Users/someone/demo-shop/shop/cart.py"), "{joined}");
        for dropped in ["PRIVATE REASONING", "TOOL OUTPUT", "sub-agent chatter", "do not show this", "local-command-caveat"] {
            assert!(!joined.contains(dropped), "{dropped} must not be kept: {joined}");
        }
        assert_eq!(r.title.as_deref(), Some("Plan the demo shop cart"));
    }

    #[test]
    fn a_half_written_last_line_waits_for_the_next_read() {
        let dir = std::env::temp_dir().join(format!("am-claude-partial-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        std::fs::write(&path, "{\"type\":\"user\",\"message\":{\"content\":\"first\"}}\n{\"type\":\"user\",\"message\":{\"content\":\"seco").unwrap();
        let s = SessionRef { native_id: "s".into(), path: path.clone(), modified: None };
        let a = ClaudeCode.read(&s, None).unwrap();
        assert_eq!(a.messages.len(), 1);
        std::fs::write(&path, "{\"type\":\"user\",\"message\":{\"content\":\"first\"}}\n{\"type\":\"user\",\"message\":{\"content\":\"second\"}}\n").unwrap();
        let b = ClaudeCode.read(&s, a.cursor.as_ref()).unwrap();
        assert_eq!(b.messages.len(), 1);
        assert_eq!(b.messages[0].text, "second");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
