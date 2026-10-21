//! Codex keeps each session as JSON lines in `~/.codex/sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl` (finished ones may be moved to
//! `~/.codex/archived_sessions/`). Every line is `{"timestamp", "type", "payload"}`; the first is `session_meta` with the folder the session ran in.
//! Format source: public descriptions of the rollout files (Oct 2026), **not yet checked against a real file on the maintainer's machine** (it has no
//! Codex chats): see docs/research/codex_chat_format.md. The reader skips what it does not know, so a changed format loses text rather than breaking.
use super::*;
use regex::Regex;
use serde_json::Value;
use std::io::{BufRead, Read};
use std::sync::OnceLock;

pub struct Codex;

const FILES_MAX: usize = 5000;
const FIRST_LINE_MAX: u64 = 1_000_000;

/// `$CODEX_HOME` when the real home is in use, else `<home>/.codex`.
fn codex_home(home: &Path) -> PathBuf {
    match std::env::var("CODEX_HOME") {
        Ok(h) if !h.trim().is_empty() && Some(home) == dirs::home_dir().as_deref() => PathBuf::from(h),
        _ => home.join(".codex"),
    }
}

fn rollouts(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if out.len() >= FILES_MAX {
            return;
        }
        let p = e.path();
        if p.is_dir() {
            if depth < 4 {
                rollouts(&p, depth + 1, out);
            }
        } else if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("rollout-")) && p.extension().is_some_and(|x| x == "jsonl") {
            out.push(p);
        }
    }
}

/// The `session_meta` payload from the first line (read up to 1 MB: newer versions embed a long system prompt there).
fn meta(path: &Path) -> Option<Value> {
    let f = std::fs::File::open(path).ok()?;
    let mut line = String::new();
    std::io::BufReader::new(f.take(FIRST_LINE_MAX)).read_line(&mut line).ok()?;
    let v: Value = serde_json::from_str(&line).ok()?;
    (v.get("type")?.as_str()? == "session_meta").then(|| v.get("payload").cloned()).flatten()
}

/// Context Codex injects as user messages: not what the person said.
fn injected(text: &str) -> bool {
    let t = text.trim_start();
    ["<environment_context", "<user_instructions", "<permissions", "<INSTRUCTIONS>", "# AGENTS.md instructions", "<turn_aborted"].iter().any(|p| t.starts_with(p))
}

fn noise() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<system-reminder>.*?</system-reminder>").unwrap())
}

fn parts_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| matches!(p.get("type").and_then(|t| t.as_str()), Some("input_text" | "output_text" | "text")))
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// `→ shell cargo test`, `→ apply_patch src/a.rs`: one short line per tool call, never the output.
fn tool_line(p: &Value) -> Option<String> {
    let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
    let raw = p.get("arguments").or_else(|| p.get("input"));
    let args: Value = match raw {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(Value::String(s.clone())),
        Some(v) => v.clone(),
        None => Value::Null,
    };
    let detail = match &args {
        Value::String(s) => s.clone(),
        Value::Object(_) => ["command", "cmd", "path", "file_path", "query", "url"].iter().find_map(|k| args.get(*k)).map(|v| match v {
            Value::Array(a) => {
                let words: Vec<&str> = a.iter().filter_map(|x| x.as_str()).collect();
                // ["bash", "-lc", "cargo test"] is just "cargo test".
                match words.as_slice() { [sh, flag, cmd] if matches!(*sh, "bash" | "sh" | "zsh") && flag.starts_with('-') => cmd.to_string(), _ => words.join(" ") }
            }
            other => other.as_str().unwrap_or("").to_string(),
        }).unwrap_or_default(),
        _ => String::new(),
    };
    let detail = detail.lines().find(|l| !l.trim().is_empty() && !l.starts_with("*** Begin Patch")).unwrap_or("");
    let detail: String = ["*** Update File: ", "*** Add File: ", "*** Delete File: "].iter().find_map(|p| detail.strip_prefix(p)).unwrap_or(detail).chars().take(100).collect();
    Some(util::tool_note(name, &detail))
}

impl TranscriptReader for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn label(&self) -> &'static str {
        "Codex"
    }

    fn locations(&self, home: &Path) -> Vec<PathBuf> {
        let c = codex_home(home);
        vec![c.join("sessions"), c.join("archived_sessions")]
    }

    fn sessions(&self, home: &Path, project_root: &Path) -> Vec<SessionRef> {
        let c = codex_home(home);
        let mut files = Vec::new();
        rollouts(&c.join("sessions"), 0, &mut files);
        rollouts(&c.join("archived_sessions"), 0, &mut files);
        let mut out: Vec<SessionRef> = files
            .into_iter()
            .filter_map(|path| {
                let m = meta(&path)?;
                util::same_project(m.get("cwd")?.as_str()?, project_root).then_some(())?;
                let native_id = m.get("id").or_else(|| m.get("session_id")).and_then(|v| v.as_str()).map(String::from).or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()))?;
                Some(SessionRef { native_id, modified: std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from), path })
            })
            .collect();
        out.sort_by_key(|s| s.modified);
        out
    }

    fn read(&self, session: &SessionRef, since: Option<&Cursor>) -> Result<ReadResult> {
        let (lines, cursor) = util::tail_lines(&session.path, since)?;
        let mut result = ReadResult { cursor: Some(cursor), ..Default::default() };
        // Codex writes each message twice (as an event for the screen and as a response item for the model). Events are the clean text; a file that has
        // none (an older or changed format) falls back to the response items.
        let (mut events, mut items, mut tools, mut artifacts): (Vec<RawMessage>, Vec<RawMessage>, Vec<RawMessage>, Vec<RawMessage>) = (vec![], vec![], vec![], vec![]);
        for line in &lines {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let ts = v.get("timestamp").and_then(|t| t.as_str()).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc));
            let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let Some(p) = v.get("payload").or_else(|| v.get("item")) else { continue };
            let ptype = p.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let push = |list: &mut Vec<RawMessage>, role: Role, text: String| {
                let text = noise().replace_all(&text, "").trim().to_string();
                if !text.is_empty() {
                    list.push(RawMessage { role, ts, text });
                }
            };
            match (kind, ptype) {
                ("event_msg", "user_message") => push(&mut events, Role::User, p.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string()),
                ("event_msg", "agent_message" | "assistant_message") => push(&mut events, Role::Assistant, p.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string()),
                ("response_item", "message") => {
                    let text = parts_text(p.get("content").unwrap_or(&Value::Null));
                    match p.get("role").and_then(|r| r.as_str()) {
                        Some("user") if !injected(&text) => push(&mut items, Role::User, text),
                        Some("assistant") => push(&mut items, Role::Assistant, text),
                        _ => {} // developer / system messages and injected context
                    }
                }
                ("response_item", "function_call" | "local_shell_call" | "custom_tool_call") => {
                    if let Some(text) = tool_line(p) {
                        tools.push(RawMessage { role: Role::Tool, ts, text });
                    }
                }
                // The streaming form of `codex exec --json`.
                ("item.completed", "agent_message") => push(&mut events, Role::Assistant, p.get("text").and_then(|m| m.as_str()).unwrap_or("").to_string()),
                ("item.completed", "command_execution") => {
                    let c = p.get("command").and_then(|c| c.as_str()).unwrap_or("");
                    tools.push(RawMessage { role: Role::Tool, ts, text: format!("→ shell {}", c.lines().next().unwrap_or("").chars().take(100).collect::<String>()).trim_end().to_string() });
                }
                // The agent's own summary of the conversation so far.
                ("compacted", _) | ("response_item", "compaction") => {
                    let text = p.get("message").or_else(|| p.get("summary")).map(|m| parts_text(m)).filter(|t| !t.is_empty()).or_else(|| p.get("message").and_then(|m| m.as_str()).map(String::from)).unwrap_or_default();
                    push(&mut artifacts, Role::Artifact, format!("Summary of the conversation so far (by Codex): {text}"));
                }
                // Reasoning, function outputs, token counts, turn context and session_meta are not kept.
                _ => {}
            }
        }
        let mut chosen = if !events.is_empty() { events } else { items };
        chosen.extend(tools);
        chosen.extend(artifacts);
        result.messages = chosen;
        result.title = result.messages.iter().find(|m| m.role == Role::User).map(|m| m.text.lines().next().unwrap_or("").chars().take(80).collect());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/codex/home")
    }

    #[test]
    fn passes_the_reader_contract() {
        let append = |path: &Path| {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
            writeln!(f, "this line is not valid for any format {{{{").unwrap();
            writeln!(f, r#"{{"timestamp":"2026-10-01T09:05:00.000Z","type":"event_msg","payload":{{"type":"agent_message","message":"One more message appended later."}}}}"#).unwrap();
        };
        super::super::testkit::conformance(&Codex, &fixture(), Path::new("/tmp/demo-shop"), &append, "FIXTURESECRET1234567890");
    }

    #[test]
    fn keeps_the_conversation_and_drops_everything_else() {
        let s = Codex.sessions(&fixture(), Path::new("/tmp/demo-shop"));
        assert_eq!(s.len(), 1, "only the chat that ran in this project (a sub-folder counts), not the other project's: {s:?}");
        let r = Codex.read(&s[0], None).unwrap();
        let joined = r.messages.iter().map(|m| format!("{}: {}", m.role.as_str(), m.text)).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("user: We want a shopping cart for the demo shop"), "{joined}");
        assert!(joined.contains("assistant: I suggest storing carts in SQLite") && joined.contains("sticky routing"), "{joined}");
        assert!(joined.contains("tool: → shell cargo test") && joined.contains("tool: → apply_patch shop/cart.py"), "{joined}");
        assert!(joined.contains("Summary of the conversation so far (by Codex)"), "{joined}");
        for never in ["PRIVATE REASONING", "TOOL OUTPUT", "environment_context", "AGENTS.md instructions", "developer rules", "do not show this"] {
            assert!(!joined.contains(never), "{never} must not be kept:\n{joined}");
        }
        assert_eq!(joined.matches("We want a shopping cart").count(), 1, "a message written twice by Codex is read once");
        assert!(r.title.as_deref().unwrap().starts_with("We want a shopping cart for the demo shop."));
    }

    #[test]
    fn an_older_file_with_only_response_items_still_reads() {
        let dir = std::env::temp_dir().join(format!("am-codex-old-{}", std::process::id()));
        let day = dir.join(".codex/sessions/2026/09/01");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(day.join("rollout-2026-09-01T10-00-00-abc.jsonl"), concat!(
            r#"{"timestamp":"2026-09-01T10:00:00Z","type":"session_meta","payload":{"id":"abc","cwd":"/tmp/old-shop"}}"#, "\n",
            r#"{"timestamp":"2026-09-01T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Fix the login bug"}]}}"#, "\n",
            r#"{"timestamp":"2026-09-01T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Fixed: the token was compared as text."}]}}"#, "\n",
        )).unwrap();
        let s = Codex.sessions(&dir, Path::new("/tmp/old-shop"));
        assert_eq!(s[0].native_id, "abc");
        let r = Codex.read(&s[0], None).unwrap();
        assert_eq!(r.messages.iter().map(|m| m.role.as_str()).collect::<Vec<_>>(), vec!["user", "assistant"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
