//! What every reader needs and should not re-implement: tailing a JSON-lines file by byte offset, matching a chat's folder to the project, and the one-line tool note.
use super::Cursor;
use anyhow::Result;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The whole lines added to `path` since `since` (a byte offset) and the new cursor. A half-written last line waits for the next read;
/// a file shorter than the cursor (replaced) is read from the start.
pub fn tail_lines(path: &Path, since: Option<&Cursor>) -> Result<(Vec<String>, Cursor)> {
    let start: u64 = since.and_then(|c| c.0.parse().ok()).unwrap_or(0);
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = if start > len { 0 } else { start };
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let complete = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let lines = String::from_utf8_lossy(&bytes[..complete]).lines().map(String::from).collect();
    Ok((lines, Cursor((start + complete as u64).to_string())))
}

/// Whether a chat that ran in `folder` belongs to the project at `root` (the folder itself or anything inside it).
pub fn same_project(folder: &str, root: &Path) -> bool {
    let (f, r) = (folder.trim_end_matches('/'), root.to_string_lossy().trim_end_matches('/').to_string());
    !r.is_empty() && (f == r || f.starts_with(&format!("{r}/")))
}

/// The first of `keys` found in a tool call's arguments, as text (a list of words is joined).
pub fn arg_detail(args: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| args.get(*k))
        .map(|v| match v {
            Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" "),
            other => other.as_str().unwrap_or("").to_string(),
        })
        .unwrap_or_default()
}

/// `→ Edit src/a.rs`: the tool's name and the first line of its detail. Never the tool's output.
pub fn tool_note(name: &str, detail: &str) -> String {
    let first: String = detail.lines().next().unwrap_or("").chars().take(100).collect();
    format!("→ {name} {first}").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_reads_whole_lines_once_and_waits_for_the_rest() {
        let p = std::env::temp_dir().join(format!("am-tail-{}.jsonl", std::process::id()));
        std::fs::write(&p, "a\nb\nc").unwrap();
        let (l1, c1) = tail_lines(&p, None).unwrap();
        assert_eq!(l1, vec!["a", "b"]);
        std::fs::write(&p, "a\nb\nc\nd\n").unwrap();
        let (l2, c2) = tail_lines(&p, Some(&c1)).unwrap();
        assert_eq!(l2, vec!["c", "d"]);
        assert!(tail_lines(&p, Some(&c2)).unwrap().0.is_empty());
        std::fs::write(&p, "x\n").unwrap();
        assert_eq!(tail_lines(&p, Some(&c2)).unwrap().0, vec!["x"], "a replaced, shorter file starts over");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn project_match_and_tool_notes() {
        assert!(same_project("/a/shop/api", Path::new("/a/shop")) && same_project("/a/shop/", Path::new("/a/shop")));
        assert!(!same_project("/a/shop2", Path::new("/a/shop")) && !same_project("/a", Path::new("/a/shop")));
        assert_eq!(tool_note("Bash", "cargo test\nsecond line"), "→ Bash cargo test");
        assert_eq!(tool_note("Task", ""), "→ Task");
        assert_eq!(arg_detail(&serde_json::json!({"command": ["bash", "-lc", "ls"]}), &["command"]), "bash -lc ls");
    }
}
