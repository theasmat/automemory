use anyhow::{anyhow, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::hooks::{
    merge_decision, run_pipeline, CanonicalEvent, HandlerSpec, HooksConfig, PipelineCtx,
};
use automemory_core::models::AutopilotMode;
use automemory_core::ports::{HandlerEnv, HookHandler};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Prints the merged pipeline handlers (builtin defaults + hooks.toml layers).
pub fn list() -> Result<()> {
    let config = Config::default_config();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = HooksConfig::load(
        &config.home_dir.join("hooks.toml"),
        &cwd.join(".automemory").join("hooks.toml"),
    );
    let mut handlers = cfg.handler.clone();
    handlers.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    println!("Hook pipeline ({} handlers):", handlers.len());
    for h in handlers {
        let kinds: Vec<&str> = h.events.iter().map(|e| e.as_str()).collect();
        println!(
            "  [{:>3}] {:<12} {:<7} budget={}ms on_failure={:?}{} events={}",
            h.priority,
            h.id,
            if h.enabled { "on" } else { "off" },
            h.latency_budget_ms,
            h.on_failure,
            if h.shadow { " shadow" } else { "" },
            if kinds.is_empty() {
                "all".to_string()
            } else {
                kinds.join(",")
            }
        );
    }
    Ok(())
}

/// Dry-runs the pipeline for an event with JSON input. Read-only: the
/// pipeline itself never writes (ledger appends live outside it), and no
/// built-ins are registered, so custom exec handlers run in isolation.
pub fn test(event_name: &str, input: &str) -> Result<()> {
    let event = CanonicalEvent::from_cli(event_name)
        .ok_or_else(|| anyhow!("unknown event: {event_name}"))?;
    let input: Value = serde_json::from_str(input).unwrap_or_else(|_| json!({}));
    let leak = |v: Vec<String>| -> &'static [String] { Box::leak(v.into_boxed_slice()) };
    let ctx = PipelineCtx {
        event,
        agent: input.get("agent").and_then(|v| v.as_str()).unwrap_or("cli"),
        session_id: input
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("dry-run"),
        tool: input
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("test-tool"),
        args: input.get("args").unwrap_or(&Value::Null),
        paths: leak(paths_of(&input)),
        symbols: leak(symbols_of(&input)),
        is_mutation: input
            .get("mutation")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    };
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cfg = HooksConfig::load(
        &config.home_dir.join("hooks.toml"),
        &cwd.join(".automemory").join("hooks.toml"),
    );
    let env = HandlerEnv {
        db: &db,
        project_id: None,
        mode: AutopilotMode::Vibe,
    };
    let lock_check = automemory_core::hooks::LockCheckHandler;
    let guardrail = automemory_core::hooks::GuardrailHandler;
    let handlers: Vec<&dyn HookHandler> = vec![&lock_check, &guardrail];
    let results = run_pipeline(&cfg.handler, &ctx, &env, &handlers);
    let merged = merge_decision(&results);
    for r in &results {
        println!(
            "  {:<12} {:?}{} {}ms{}",
            r.id,
            r.decision,
            if r.reason.is_empty() {
                String::new()
            } else {
                format!(" ({})", r.reason)
            },
            r.elapsed_ms,
            if r.failed { " FAILED" } else { "" }
        );
    }
    if merged.reason.is_empty() {
        println!("{}", json!({ "decision": merged.decision.as_str() }));
    } else {
        println!(
            "{}",
            json!({ "decision": merged.decision.as_str(), "reason": merged.reason })
        );
    }
    Ok(())
}

fn paths_of(input: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["paths", "files", "file_path", "path", "file"] {
        match input.get(key) {
            Some(Value::String(s)) => out.push(s.clone()),
            Some(Value::Array(arr)) => {
                out.extend(arr.iter().filter_map(|v| v.as_str().map(str::to_string)));
            }
            _ => {}
        }
    }
    out
}

fn symbols_of(input: &Value) -> Vec<String> {
    match input.get("symbols") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// Edits one handler field in the project hooks.toml (creates the file and a
/// minimal entry when absent). The global file is never touched.
pub fn set_field(id: &str, f: impl FnOnce(&mut HandlerSpec)) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let path = cwd.join(".automemory").join("hooks.toml");
    let mut cfg: HooksConfig = if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        toml::from_str(&text).unwrap_or_default()
    } else {
        HooksConfig::default()
    };
    match cfg.handler.iter_mut().find(|h| h.id == id) {
        Some(h) => f(h),
        None => {
            // Seed from the builtin default when one exists, so edits keep
            // the handler's events and budget instead of a blank entry.
            let mut fresh = HooksConfig::with_defaults()
                .handler
                .into_iter()
                .find(|h| h.id == id)
                .unwrap_or(HandlerSpec {
                    id: id.to_string(),
                    events: Vec::new(),
                    priority: 50,
                    latency_budget_ms: 100,
                    on_failure: automemory_core::hooks::FailureMode::Open,
                    shadow: false,
                    enabled: true,
                    exec: None,
                });
            f(&mut fresh);
            cfg.handler.push(fresh);
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, toml::to_string_pretty(&cfg)?)?;
    println!("✓ hooks.toml updated: {id} ({})", path.display());
    Ok(())
}
