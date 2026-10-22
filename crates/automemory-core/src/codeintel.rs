//! CodeIntel backed by the external `codebase-memory-mcp` binary (PRD §7.1).
//!
//! AutoMemory builds no AST graph of its own. Symbol questions go to
//! `codebase-memory-mcp cli --json <tool> <args>`; when the binary is missing,
//! the project is not indexed, or a call fails, the answer is negative ([`NoCodeIntel`]) so verification never hard-fails.

use crate::ports::{CodeIntel, NoCodeIntel, Port, CODE_INTEL_API_VERSION};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const BIN_NAME: &str = "codebase-memory-mcp";
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Locates the binary: `AUTOMEMORY_CBM_BIN`, then `PATH`, then `~/.local/bin`.
pub fn find_binary() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("AUTOMEMORY_CBM_BIN").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(BIN_NAME))
            .find(|p| p.is_file())
    });
    on_path.or_else(|| {
        dirs::home_dir()
            .map(|h| h.join(".local/bin").join(BIN_NAME))
            .filter(|p| p.is_file())
    })
}

/// Version string for `doctor` (`codebase-memory-mcp 0.11.0`), if the binary runs.
pub fn binary_version() -> Option<String> {
    let bin = find_binary()?;
    let out = Command::new(bin).arg("--version").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Oldest engine whose `cli --json` output this module was written against.
pub const MIN_ENGINE_VERSION: (u32, u32) = (0, 11);

/// True when a `codebase-memory-mcp X.Y.Z` version string is new enough.
pub fn version_supported(version: &str) -> bool {
    let nums: Vec<u32> = version
        .split_whitespace()
        .last()
        .unwrap_or("")
        .split('.')
        .filter_map(|n| n.parse().ok())
        .collect();
    matches!(nums.as_slice(), [maj, min, ..] if (*maj, *min) >= MIN_ENGINE_VERSION)
}

/// The engine names a project after its root: `/a/b/c` becomes `a-b-c`.
pub fn project_name_for(root: &Path) -> String {
    root.to_string_lossy()
        .trim_matches('/')
        .replace(['/', '\\', ':'], "-")
}

pub struct CbmCodeIntel {
    fallback: NoCodeIntel,
    bin: PathBuf,
    project: String,
}

impl CbmCodeIntel {
    /// `None` when the binary is not installed.
    pub fn detect(project_root: &Path) -> Option<Self> {
        Some(Self {
            fallback: NoCodeIntel,
            bin: find_binary()?,
            project: project_name_for(project_root),
        })
    }

    /// Runs one engine tool and returns its `structuredContent`, or the text content
    /// wrapped as `{"text": ...}`. `None` on spawn failure, timeout, or `isError`.
    fn call(&self, tool: &str, mut args: Value) -> Option<Value> {
        args["project"] = json!(self.project);
        let mut child = Command::new(&self.bin)
            .args(["cli", "--quiet", "--json", tool, &args.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        // Drain stdout on a thread: a reply over the pipe buffer (~64 KB) would otherwise
        // block the engine on write and look like a timeout.
        let mut stdout = child.stdout.take()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stdout.read_to_string(&mut buf);
            let _ = tx.send(buf);
        });
        let Ok(raw) = rx.recv_timeout(CALL_TIMEOUT) else {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        };
        let _ = child.wait();
        let envelope: Value = serde_json::from_str(&raw).ok()?;
        if envelope["isError"].as_bool() == Some(true) {
            return None;
        }
        let structured = &envelope["structuredContent"];
        if structured.get("error").is_some() {
            return None;
        }
        if !structured.is_null() {
            return Some(structured.clone());
        }
        let text = envelope["content"][0]["text"].as_str()?;
        Some(json!({ "text": text }))
    }

    /// Qualified names of symbols called exactly `symbol`.
    fn find_qualified(&self, symbol: &str) -> Vec<String> {
        let pattern = format!("^{}$", regex_escape(symbol));
        let Some(v) = self.call(
            "search_graph",
            json!({ "name_pattern": pattern, "limit": 5, "format": "json" }),
        ) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for g in v["groups"].as_array().into_iter().flatten() {
            let prefix = g["qn_prefix"].as_str().unwrap_or("");
            for row in g["rows"].as_array().into_iter().flatten() {
                if let Some(name) = row[0].as_str() {
                    out.push(if prefix.is_empty() {
                        name.to_string()
                    } else {
                        format!("{prefix}.{name}")
                    });
                }
            }
        }
        out
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The `source: |` block of a `get_code_snippet` reply, de-indented.
fn snippet_source(text: &str) -> Option<String> {
    let body = text.split_once("source: |\n")?.1;
    let lines: Vec<&str> = body.lines().collect();
    let indent = lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(0);
    Some(
        lines
            .iter()
            .map(|l| l.get(indent..).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

impl Port for CbmCodeIntel {
    fn port_name(&self) -> &'static str {
        "CodeIntel"
    }
    fn api_version(&self) -> &'static str {
        CODE_INTEL_API_VERSION
    }
}

impl CodeIntel for CbmCodeIntel {
    fn symbol_exists(&self, project_id: Option<&str>, symbol: &str) -> bool {
        !self.find_qualified(symbol).is_empty() || self.fallback.symbol_exists(project_id, symbol)
    }

    /// Hash of the symbol's own source text, so unrelated edits to the same file
    /// do not change it (the fallback hashes the whole file).
    fn symbol_hash(&self, project_id: Option<&str>, symbol: &str) -> Option<String> {
        for qn in self.find_qualified(symbol) {
            let snippet = self.call("get_code_snippet", json!({ "qualified_name": qn }));
            if let Some(src) = snippet
                .as_ref()
                .and_then(|v| v["text"].as_str())
                .and_then(snippet_source)
            {
                return Some(format!("{:x}", Sha256::digest(src.as_bytes())));
            }
        }
        self.fallback.symbol_hash(project_id, symbol)
    }

    fn callers(&self, project_id: Option<&str>, symbol: &str) -> Vec<String> {
        let Some(v) = self.call(
            "trace_path",
            json!({ "function_name": symbol, "direction": "inbound", "depth": 1, "format": "json" }),
        ) else {
            return self.fallback.callers(project_id, symbol);
        };
        let mut out: Vec<String> = Vec::new();
        for g in v["callers"]["groups"].as_array().into_iter().flatten() {
            for row in g["rows"].as_array().into_iter().flatten() {
                if let Some(name) = row[0].as_str().filter(|n| *n != "__file__") {
                    if !out.iter().any(|o| o == name) {
                        out.push(name.to_string());
                    }
                }
            }
        }
        out
    }
}

/// The CodeIntel to use for `project_root`: the engine when installed, else one that answers negatively.
pub fn code_intel_for(project_root: &Path) -> Box<dyn CodeIntel> {
    // The engine names a project after the repository root, not the folder we happen to run in.
    let root = crate::sync::GitSync::resolve_git_canonical_root(project_root)
        .unwrap_or_else(|| project_root.to_path_buf());
    match CbmCodeIntel::detect(&root) {
        Some(c) => Box::new(c),
        None => Box::new(NoCodeIntel),
    }
}

#[cfg(all(test, unix))] // the stand-in engine is a shell script
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in engine that answers from canned JSON, chosen by tool name.
    fn fake_engine(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("am-cbm-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(BIN_NAME);
        let script = r#"#!/bin/sh
# args: cli --quiet --json <tool> <json>
case "$4" in
  search_graph) cat <<'J'
{"content":[],"structuredContent":{"groups":[{"qn_prefix":"proj.mod","rows":[["target_fn","Function","1-3",1,0]]}]},"isError":false}
J
  ;;
  get_code_snippet) cat <<'J'
{"content":[{"type":"text","text":"name: target_fn\nsource_mode: full\nsource: |\n      fn target_fn() {\n          body();\n      }\n"}],"isError":false}
J
  ;;
  trace_path) cat <<'J'
{"content":[],"structuredContent":{"callers":{"groups":[{"qn_prefix":"proj.a","rows":[["__file__",1]]},{"qn_prefix":"proj.b","rows":[["caller_one",1]]}]}},"isError":false}
J
  ;;
  *) echo '{"content":[],"isError":true}' ;;
esac
"#;
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn engine_answers_replace_the_local_graph() {
        let bin = fake_engine("answers");
        let intel = CbmCodeIntel {
            fallback: NoCodeIntel,
            bin,
            project: "proj".into(),
        };
        assert!(intel.symbol_exists(None, "target_fn"));
        assert_eq!(intel.callers(None, "target_fn"), ["caller_one"]); // __file__ dropped

        let h1 = intel.symbol_hash(None, "target_fn").unwrap();
        assert_eq!(h1.len(), 64);
        // De-indented source: hash is stable regardless of the engine's indentation.
        let expected = format!("{:x}", Sha256::digest("fn target_fn() {\n    body();\n}".as_bytes()));
        assert_eq!(h1, expected);
    }

    #[test]
    fn a_reply_larger_than_the_pipe_buffer_is_still_read() {
        let dir = std::env::temp_dir().join(format!("am-cbm-big-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(BIN_NAME);
        // 200 KB reply: more than a pipe holds, so the engine blocks until we read.
        let script = "#!/bin/sh\nhead -c 200000 /dev/zero | tr '\\0' 'x' | sed 's/^/{\"structuredContent\":{\"groups\":[{\"qn_prefix\":\"p\",\"rows\":[[\"target_fn\"]]}],\"pad\":\"/; s/$/\"}}/'\n";
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let intel = CbmCodeIntel { fallback: NoCodeIntel, bin, project: "p".into() };
        let start = std::time::Instant::now();
        assert!(intel.symbol_exists(None, "target_fn"));
        assert!(start.elapsed() < Duration::from_secs(3), "must not wait for the timeout");
    }

    #[test]
    fn missing_or_failing_engine_answers_negative() {
        let intel = CbmCodeIntel {
            fallback: NoCodeIntel,
            bin: PathBuf::from("/nonexistent/codebase-memory-mcp"),
            project: "proj".into(),
        };
        // A failing engine answers negative, never panics.
        assert!(!intel.symbol_exists(None, "anything"));
        assert!(intel.symbol_hash(None, "anything").is_none());
        assert!(intel.callers(None, "anything").is_empty());
    }

    #[test]
    fn engine_version_gate() {
        assert!(version_supported("codebase-memory-mcp 0.11.0"));
        assert!(version_supported("codebase-memory-mcp 1.0.2"));
        assert!(!version_supported("codebase-memory-mcp 0.9.9"));
        assert!(!version_supported("garbage"));
    }

    #[test]
    fn helpers() {
        assert_eq!(project_name_for(Path::new("/Users/as/Dev/app")), "Users-as-Dev-app");
        assert_eq!(regex_escape("a.b(c)"), "a\\.b\\(c\\)");
    }
}
