//! Command Code keeps each session as an append-only JSON-lines transcript at `~/.commandcode/projects/<slug of the working folder>/<session>.jsonl`,
//! with `<session>.meta.json` beside it (title, model). The first line is a header (session id, creation time, working folder); later lines are entries
//! (user messages, model replies, compaction summaries) forming a tree via parent pointers.
//! Format source: the public docs page /docs/sessions (Oct 2026), which gives the layout but **not the field names**, and no Command Code chat exists on the
//! maintainer's machine: the field handling below is tolerant guesswork, **unverified against a real file** (docs/research/command_code_chat_format.md).
use super::*;
use serde_json::Value;
use std::io::Read;

pub struct CommandCode;

fn root(home: &Path) -> PathBuf {
    home.join(".commandcode").join("projects")
}

/// `/Users/me/shop` -> `-Users-me-shop` (a guess at the slug: the header's folder is what is trusted when present).
fn slugs(project_root: &Path) -> Vec<String> {
    let s: String = project_root.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    vec![s.clone(), s.trim_start_matches('-').to_string()]
}

fn first_line(path: &Path) -> Option<Value> {
    let mut s = String::new();
    std::io::BufReader::new(std::fs::File::open(path).ok()?.take(1_000_000)).read_to_string(&mut s).ok()?;
    s.lines().next().and_then(|l| serde_json::from_str(l).ok())
}

fn header_cwd(h: &Value) -> Option<String> {
    ["cwd", "workingDirectory", "workingDir", "directory", "projectPath", "projectDir"].iter().find_map(|k| h.get(*k).and_then(|v| v.as_str())).map(String::from)
}

fn parts_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| matches!(p.get("type").and_then(|t| t.as_str()), None | Some("text" | "input_text" | "output_text")))
            .filter_map(|p| p.get("text").and_then(|t| t.as_str()).or_else(|| p.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => content.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn tool_lines(content: &Value, out: &mut Vec<RawMessage>, ts: Option<DateTime<Utc>>) {
    for p in content.as_array().into_iter().flatten() {
        if !matches!(p.get("type").and_then(|t| t.as_str()), Some("tool_use" | "tool-call" | "tool_call" | "toolCall")) {
            continue;
        }
        let name = p.get("name").or_else(|| p.get("toolName")).and_then(|n| n.as_str()).unwrap_or("tool");
        let args = p.get("input").or_else(|| p.get("args")).or_else(|| p.get("arguments")).unwrap_or(&Value::Null);
        out.push(RawMessage { role: Role::Tool, ts, text: util::tool_note(name, &util::arg_detail(args, &["command", "file_path", "path", "pattern", "query", "url"])) });
    }
}

impl TranscriptReader for CommandCode {
    fn id(&self) -> &'static str {
        "command-code"
    }

    fn label(&self) -> &'static str {
        "Command Code"
    }

    fn locations(&self, home: &Path) -> Vec<PathBuf> {
        vec![root(home)]
    }

    fn sessions(&self, home: &Path, project_root: &Path) -> Vec<SessionRef> {
        let want = slugs(project_root);
        let mut out = Vec::new();
        for folder in std::fs::read_dir(root(home)).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
            let by_name = want.iter().any(|s| folder.file_name().to_string_lossy() == s.as_str());
            for e in std::fs::read_dir(folder.path()).into_iter().flatten().flatten() {
                let path = e.path();
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                // Only the transcripts, not <id>.checkpoints.jsonl or <id>.prompts.jsonl.
                if !name.ends_with(".jsonl") || name.ends_with(".checkpoints.jsonl") || name.ends_with(".prompts.jsonl") {
                    continue;
                }
                let head = first_line(&path);
                let ok = match head.as_ref().and_then(header_cwd) {
                    Some(cwd) => util::same_project(&cwd, project_root),
                    None => by_name,
                };
                if !ok {
                    continue;
                }
                let native_id = head.as_ref().and_then(|h| h.get("sessionId").or_else(|| h.get("id")).and_then(|v| v.as_str()).map(String::from)).or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()));
                let Some(native_id) = native_id else { continue };
                out.push(SessionRef { native_id, modified: e.metadata().ok().and_then(|m| m.modified().ok()).map(DateTime::<Utc>::from), path });
            }
        }
        out.sort_by_key(|s| s.modified);
        out
    }

    fn read(&self, session: &SessionRef, since: Option<&Cursor>) -> Result<ReadResult> {
        let (lines, cursor) = util::tail_lines(&session.path, since)?;
        let start: u64 = since.and_then(|c| c.0.parse().ok()).unwrap_or(0);
        let mut result = ReadResult { cursor: Some(cursor), ..Default::default() };
        for (i, line) in lines.iter().enumerate() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if start == 0 && i == 0 && header_cwd(&v).is_some() && v.get("role").is_none() && v.get("message").is_none() {
                continue; // the header
            }
            let ts = ["timestamp", "createdAt", "ts", "time"].iter().find_map(|k| v.get(*k).and_then(|t| t.as_str())).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc));
            let msg = v.get("message").unwrap_or(&v);
            let kind = v.get("type").or_else(|| v.get("kind")).and_then(|t| t.as_str()).unwrap_or("");
            let role = msg.get("role").or_else(|| v.get("role")).and_then(|r| r.as_str()).unwrap_or(kind);
            let content = msg.get("content").or_else(|| msg.get("text")).or_else(|| v.get("content")).unwrap_or(&Value::Null);
            let text = parts_text(content).trim().to_string();
            if kind.contains("compaction") || kind.contains("summary") {
                let t = v.get("summary").and_then(|s| s.as_str()).map(String::from).unwrap_or(text);
                if !t.trim().is_empty() {
                    result.messages.push(RawMessage { role: Role::Artifact, ts, text: format!("Summary of the conversation so far (by Command Code): {}", t.trim()) });
                }
                continue;
            }
            match role {
                "user" | "human" if !text.is_empty() => result.messages.push(RawMessage { role: Role::User, ts, text }),
                "assistant" | "model" | "agent" => {
                    if !text.is_empty() {
                        result.messages.push(RawMessage { role: Role::Assistant, ts, text });
                    }
                    tool_lines(content, &mut result.messages, ts);
                }
                _ => {} // model/effort changes, tool results, anything unknown
            }
        }
        // The title is in <id>.meta.json beside the transcript.
        let meta = session.path.with_extension("meta.json");
        result.title = std::fs::read(&meta).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|m| m.get("title").and_then(|t| t.as_str()).map(String::from));
        if result.title.is_none() {
            result.title = result.messages.iter().find(|m| m.role == Role::User).map(|m| m.text.lines().next().unwrap_or("").chars().take(80).collect());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/command-code/home")
    }

    #[test]
    fn passes_the_reader_contract() {
        let append = |path: &Path| {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
            writeln!(f, "this line is not valid for any format {{{{").unwrap();
            writeln!(f, r#"{{"id":"e-new","parentId":"e5","type":"assistant","timestamp":"2026-10-01T09:05:00.000Z","message":{{"role":"assistant","content":[{{"type":"text","text":"One more message appended later."}}]}}}}"#).unwrap();
        };
        super::super::testkit::conformance(&CommandCode, &fixture(), Path::new("/tmp/demo-shop"), &append, "FIXTURESECRET1234567890");
    }

    #[test]
    fn keeps_the_conversation_and_drops_everything_else() {
        let s = CommandCode.sessions(&fixture(), Path::new("/tmp/demo-shop"));
        assert_eq!(s.len(), 1, "this project's transcript only (not checkpoints, prompts or another project's): {s:?}");
        let r = CommandCode.read(&s[0], None).unwrap();
        let joined = r.messages.iter().map(|m| format!("{}: {}", m.role.as_str(), m.text)).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("user: We want a shopping cart") && joined.contains("sticky routing") && joined.contains("tool: → Bash cargo test"), "{joined}");
        assert!(joined.contains("Summary of the conversation so far (by Command Code)"), "{joined}");
        for never in ["PRIVATE REASONING", "TOOL RESULT", "claude-fixture", "the header"] {
            assert!(!joined.contains(never), "{never} must not be kept:\n{joined}");
        }
        assert_eq!(r.title.as_deref(), Some("Plan the demo shop cart"));
    }
}
