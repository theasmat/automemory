use crate::db::Database;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

/// Canonical hook events (PRD §7.2). Native agent event names map onto these
/// via [`CanonicalEvent::from_cli`], so new agent adapters need no core change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanonicalEvent {
    SessionStart,
    PreToolUse,
    PostToolUse,
    PreCommit,
    PostCommit,
    Stop,
}

impl CanonicalEvent {
    /// Maps CLI/native event spellings (`pre-tool`, `pre_tool`, `session_boot`, …)
    /// to the canonical set. Unknown names return `None` (caller fails open).
    pub fn from_cli(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().replace('-', "_").as_str() {
            "boot" | "session_boot" | "sessionstart" | "session_start" => Some(Self::SessionStart),
            "pre_tool" | "pretooluse" | "pre_tool_use" => Some(Self::PreToolUse),
            "post_tool" | "post_tool_fail" | "posttooluse" | "post_tool_use" => {
                Some(Self::PostToolUse)
            }
            "pre_commit" | "precommit" => Some(Self::PreCommit),
            "post_commit" | "postcommit" => Some(Self::PostCommit),
            "stop" | "task_done" | "session_stop" => Some(Self::Stop),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PreCommit => "PreCommit",
            Self::PostCommit => "PostCommit",
            Self::Stop => "Stop",
        }
    }
}

/// How a handler failure (crash, bad output, unknown built-in) resolves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailureMode {
    #[default]
    Open,
    Closed,
}

/// Gate strictness. Ordering is load-bearing: [`merge_decision`] keeps the max.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GateDecision {
    Allow,
    Warn,
    Ask,
    Deny,
}

impl GateDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Warn => "warn",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "allow" => Some(Self::Allow),
            "warn" => Some(Self::Warn),
            "ask" => Some(Self::Ask),
            "deny" | "block" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HandlerOutput {
    pub decision: GateDecision,
    pub reason: String,
}

impl HandlerOutput {
    pub fn allow() -> Self {
        Self {
            decision: GateDecision::Allow,
            reason: String::new(),
        }
    }
}

/// One stage in the hook pipeline. `exec = None` means built-in (run by the
/// host via the `run_builtin` callback); `exec = Some(cmd)` runs an external
/// command with the event JSON on stdin (JSON out). An empty `events` list
/// subscribes to every event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerSpec {
    pub id: String,
    #[serde(default)]
    pub events: Vec<CanonicalEvent>,
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default = "default_budget_ms")]
    pub latency_budget_ms: u64,
    #[serde(default)]
    pub on_failure: FailureMode,
    #[serde(default)]
    pub shadow: bool,
    #[serde(default)]
    pub exec: Option<String>,
}

fn default_priority() -> u8 {
    50
}
fn default_budget_ms() -> u64 {
    100
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default)]
    pub handler: Vec<HandlerSpec>,
}

impl HooksConfig {
    /// Built-in pipeline: lease collision check first, guardrail second.
    pub fn with_defaults() -> Self {
        Self {
            handler: vec![
                HandlerSpec {
                    id: "lock-check".to_string(),
                    events: vec![CanonicalEvent::PreToolUse, CanonicalEvent::PreCommit],
                    priority: 10,
                    latency_budget_ms: 20,
                    on_failure: FailureMode::Open,
                    shadow: false,
                    exec: None,
                },
                HandlerSpec {
                    id: "guardrail".to_string(),
                    events: vec![CanonicalEvent::PreToolUse],
                    priority: 20,
                    latency_budget_ms: 20,
                    on_failure: FailureMode::Open,
                    shadow: false,
                    exec: None,
                },
            ],
        }
    }

    fn from_toml_str(s: &str) -> Self {
        toml::from_str(s).unwrap_or_default()
    }

    /// Layered config: built-in defaults, then global file, then project file.
    /// Later layers win per handler id. Missing files are ignored, so hooks
    /// are hot-reloaded on every out-of-process invocation for free.
    pub fn load(global_path: &Path, project_path: &Path) -> Self {
        let mut merged = Self::with_defaults();
        for path in [global_path, project_path] {
            if let Ok(text) = std::fs::read_to_string(path) {
                merged.merge(Self::from_toml_str(&text));
            }
        }
        merged
    }

    fn merge(&mut self, other: Self) {
        for h in other.handler {
            if let Some(pos) = self.handler.iter().position(|e| e.id == h.id) {
                self.handler[pos] = h;
            } else {
                self.handler.push(h);
            }
        }
    }
}

/// Read-only view handed to every handler in one pipeline run.
/// Callers pass the redacted tool input as `args` where available, so
/// external `exec` handlers stay data-only by construction.
pub struct PipelineCtx<'a> {
    pub event: CanonicalEvent,
    pub agent: &'a str,
    pub session_id: &'a str,
    pub tool: &'a str,
    pub args: &'a serde_json::Value,
    pub paths: &'a [String],
    pub symbols: &'a [String],
    pub is_mutation: bool,
}

pub struct HandlerResult {
    pub id: String,
    pub decision: GateDecision,
    pub reason: String,
    pub elapsed_ms: u128,
    pub over_budget: bool,
    pub shadowed: bool,
    pub failed: bool,
}

/// Runs handlers ordered by (priority, id). Failures resolve via the
/// handler's own `on_failure` mode; `shadow` downgrades any non-allow to an
/// observed allow. Budgets are measured and logged, not preemptive:
/// ponytail: kill-on-timeout deferred until an async runner exists; agent
/// surfaces already bound hook execution time (e.g. agy `timeout: 5`).
pub fn run_pipeline(
    specs: &[HandlerSpec],
    ctx: &PipelineCtx<'_>,
    run_builtin: &dyn Fn(&str, &PipelineCtx<'_>) -> HandlerOutput,
) -> Vec<HandlerResult> {
    let mut ordered: Vec<&HandlerSpec> = specs
        .iter()
        .filter(|s| s.events.is_empty() || s.events.contains(&ctx.event))
        .collect();
    ordered.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));

    let mut results = Vec::with_capacity(ordered.len());
    for spec in ordered {
        let start = Instant::now();
        let (mut out, failed) = if let Some(cmd) = spec.exec.as_deref() {
            match run_exec(cmd, ctx) {
                Ok(o) => (o, false),
                Err(e) => (
                    HandlerOutput {
                        decision: GateDecision::Allow,
                        reason: e,
                    },
                    true,
                ),
            }
        } else {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_builtin(&spec.id, ctx)
            }));
            match out {
                Ok(o) => (o, false),
                Err(_) => (
                    HandlerOutput {
                        decision: GateDecision::Allow,
                        reason: format!("built-in '{}' panicked", spec.id),
                    },
                    true,
                ),
            }
        };
        let elapsed_ms = start.elapsed().as_millis();
        let over_budget = elapsed_ms > u128::from(spec.latency_budget_ms);

        if failed {
            eprintln!(
                "[AutoMemory Hook] handler '{}' failed ({}); resolving {}",
                spec.id,
                out.reason,
                match spec.on_failure {
                    FailureMode::Open => "fail-open → allow",
                    FailureMode::Closed => "fail-closed → deny",
                }
            );
            out.decision = match spec.on_failure {
                FailureMode::Open => GateDecision::Allow,
                FailureMode::Closed => GateDecision::Deny,
            };
        }
        if over_budget {
            eprintln!(
                "[AutoMemory Hook] handler '{}' took {}ms over {}ms budget",
                spec.id, elapsed_ms, spec.latency_budget_ms
            );
        }

        let mut shadowed = false;
        if spec.shadow && out.decision != GateDecision::Allow {
            eprintln!(
                "[AutoMemory Hook] shadow: '{}' would have {:?} ({}); allowing",
                spec.id, out.decision, out.reason
            );
            out.decision = GateDecision::Allow;
            shadowed = true;
        }

        results.push(HandlerResult {
            id: spec.id.clone(),
            decision: out.decision,
            reason: out.reason,
            elapsed_ms,
            over_budget,
            shadowed,
            failed,
        });
    }
    results
}

/// Expands file targets to indexed symbols (and symbols to files) so
/// symbol-level leases collide with file edits across naming styles
/// (`auth::login` vs `auth.login`). Empty index = no expansion (fail-open).
pub fn expand_scope_via_graph(
    db: &Database,
    project_id: Option<&str>,
    paths: &[String],
    symbols: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut all_paths: Vec<String> = paths.to_vec();
    let mut all_symbols: Vec<String> = symbols.to_vec();
    for p in paths.iter().take(10) {
        for e in db
            .query_graph_entities(project_id, None, None, Some(p), 20)
            .unwrap_or_default()
        {
            if !all_symbols.contains(&e.name) {
                all_symbols.push(e.name);
            }
        }
    }
    for s in symbols.iter().take(10) {
        for spelling in [s.clone(), s.replace("::", "."), s.replace('.', "::")] {
            for e in db
                .query_graph_entities(project_id, None, Some(&spelling), None, 5)
                .unwrap_or_default()
            {
                if !all_paths.contains(&e.path) {
                    all_paths.push(e.path);
                }
            }
        }
    }
    (all_paths, all_symbols)
}

/// Strictest decision wins; the reason comes from the strictest handler.
pub fn merge_decision(results: &[HandlerResult]) -> HandlerOutput {
    let mut best = HandlerOutput::allow();
    for r in results {
        if r.decision > best.decision {
            best.decision = r.decision;
            best.reason = r.reason.clone();
        }
    }
    best
}

fn run_exec(cmd: &str, ctx: &PipelineCtx<'_>) -> Result<HandlerOutput, String> {
    let stdin_json = serde_json::json!({
        "event": ctx.event.as_str(),
        "agent": ctx.agent,
        "session_id": ctx.session_id,
        "tool": ctx.tool,
        "args": ctx.args,
        "paths": ctx.paths,
        "symbols": ctx.symbols,
        "mutation": ctx.is_mutation,
    });
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn failed: {e}"))?;
    if let Some(stdin) = child.stdin.take() {
        let _ = write_json_stdin(stdin, &stdin_json);
    }
    let output = child.wait_with_output().map_err(|e| format!("wait failed: {e}"))?;
    if !output.status.success() {
        return Err(format!("exited with {}", output.status));
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("bad JSON: {e}"))?;
    let decision = parsed
        .get("decision")
        .and_then(|v| v.as_str())
        .and_then(GateDecision::from_str)
        .ok_or_else(|| "missing/invalid `decision`".to_string())?;
    Ok(HandlerOutput {
        decision,
        reason: parsed
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

fn write_json_stdin(
    mut stdin: std::process::ChildStdin,
    v: &serde_json::Value,
) -> std::io::Result<()> {
    writeln!(stdin, "{v}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> PipelineCtx<'static> {
        // Leaked empties keep the borrowed slices alive for the test.
        let args: &'static serde_json::Value = Box::leak(Box::new(serde_json::json!({})));
        let paths: &'static [String] = Box::leak(Box::new(Vec::new()));
        let symbols: &'static [String] = Box::leak(Box::new(Vec::new()));
        PipelineCtx {
            event: CanonicalEvent::PreToolUse,
            agent: "test",
            session_id: "s",
            tool: "edit",
            args,
            paths,
            symbols,
            is_mutation: true,
        }
    }

    #[test]
    fn canonical_mapping_covers_cli_spellings() {
        assert_eq!(
            CanonicalEvent::from_cli("pre-tool"),
            Some(CanonicalEvent::PreToolUse)
        );
        assert_eq!(
            CanonicalEvent::from_cli("post-tool-fail"),
            Some(CanonicalEvent::PostToolUse)
        );
        assert_eq!(
            CanonicalEvent::from_cli("task-done"),
            Some(CanonicalEvent::Stop)
        );
        assert_eq!(
            CanonicalEvent::from_cli("pre-commit"),
            Some(CanonicalEvent::PreCommit)
        );
        assert_eq!(CanonicalEvent::from_cli("bogus"), None);
    }

    #[test]
    fn pipeline_orders_by_priority_and_merges_strictest() {
        let specs = vec![
            HandlerSpec {
                id: "b".to_string(),
                events: vec![],
                priority: 20,
                latency_budget_ms: 100,
                on_failure: FailureMode::Open,
                shadow: false,
                exec: None,
            },
            HandlerSpec {
                id: "a".to_string(),
                events: vec![],
                priority: 10,
                latency_budget_ms: 100,
                on_failure: FailureMode::Open,
                shadow: false,
                exec: None,
            },
        ];
        let c = ctx();
        let run = |id: &str, _: &PipelineCtx<'_>| match id {
            "a" => HandlerOutput {
                decision: GateDecision::Warn,
                reason: "w".to_string(),
            },
            _ => HandlerOutput {
                decision: GateDecision::Ask,
                reason: "a".to_string(),
            },
        };
        let results = run_pipeline(&specs, &c, &run);
        assert_eq!(results[0].id, "a");
        assert_eq!(results[1].id, "b");
        let merged = merge_decision(&results);
        assert_eq!(merged.decision, GateDecision::Ask);
        assert_eq!(merged.reason, "a");
    }

    #[test]
    fn failure_mode_open_vs_closed() {
        let mk = |mode| HandlerSpec {
            id: "x".to_string(),
            events: vec![],
            priority: 1,
            latency_budget_ms: 100,
            on_failure: mode,
            shadow: false,
            exec: Some("/nonexistent-automemory-handler-xyz".to_string()),
        };
        let c = ctx();
        let never = |_: &str, _: &PipelineCtx<'_>| HandlerOutput::allow();
        let open = run_pipeline(&[mk(FailureMode::Open)], &c, &never);
        assert_eq!(open[0].decision, GateDecision::Allow);
        assert!(open[0].failed);
        let closed = run_pipeline(&[mk(FailureMode::Closed)], &c, &never);
        assert_eq!(closed[0].decision, GateDecision::Deny);
    }

    #[test]
    fn shadow_downgrades_deny_to_observed_allow() {
        let specs = vec![HandlerSpec {
            id: "strict".to_string(),
            events: vec![],
            priority: 1,
            latency_budget_ms: 100,
            on_failure: FailureMode::Open,
            shadow: true,
            exec: None,
        }];
        let c = ctx();
        let run = |_: &str, _: &PipelineCtx<'_>| HandlerOutput {
            decision: GateDecision::Deny,
            reason: "would block".to_string(),
        };
        let results = run_pipeline(&specs, &c, &run);
        assert_eq!(results[0].decision, GateDecision::Allow);
        assert!(results[0].shadowed);
    }

    #[test]
    fn config_layers_project_wins_per_id() {
        let base = HooksConfig::with_defaults();
        assert_eq!(base.handler.len(), 2);
        let overlay = HooksConfig::from_toml_str(
            r#"
            [[handler]]
            id = "guardrail"
            events = ["PreToolUse"]
            priority = 5
            latency_budget_ms = 10
            on_failure = "closed"
            shadow = true
            "#,
        );
        let mut merged = base;
        merged.merge(overlay);
        assert_eq!(merged.handler.len(), 2);
        let g = merged.handler.iter().find(|h| h.id == "guardrail").unwrap();
        assert_eq!(g.priority, 5);
        assert_eq!(g.on_failure, FailureMode::Closed);
        assert!(g.shadow);
    }

    #[test]
    fn exec_handler_json_roundtrip() {
        let specs = vec![HandlerSpec {
            id: "ext".to_string(),
            events: vec![],
            priority: 1,
            latency_budget_ms: 5000,
            on_failure: FailureMode::Closed,
            shadow: false,
            exec: Some("printf '{\"decision\":\"warn\",\"reason\":\"custom\"}'".to_string()),
        }];
        let c = ctx();
        let never = |_: &str, _: &PipelineCtx<'_>| HandlerOutput::allow();
        let results = run_pipeline(&specs, &c, &never);
        assert_eq!(results[0].decision, GateDecision::Warn);
        assert_eq!(results[0].reason, "custom");
    }

    #[test]
    fn scope_expansion_bridges_files_and_symbols() {
        use crate::graph::GraphEntity;
        use chrono::Utc;
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();
        db.batch_insert_graph_entities(&[GraphEntity {
            id: "e1".to_string(),
            project_id: None,
            entity_type: "function".to_string(),
            name: "auth.login".to_string(),
            path: "src/auth/login.ts".to_string(),
            line_start: None,
            line_end: None,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }])
        .unwrap();

        // File edit discovers the symbol defined inside it.
        let (_, syms) = expand_scope_via_graph(
            &db,
            None,
            &["src/auth/login.ts".to_string()],
            &[],
        );
        assert!(syms.contains(&"auth.login".to_string()));

        // Symbol lease in either spelling discovers the file.
        for spelling in ["auth::login", "auth.login"] {
            let (paths, _) =
                expand_scope_via_graph(&db, None, &[], &[spelling.to_string()]);
            assert!(
                paths.contains(&"src/auth/login.ts".to_string()),
                "spelling {spelling}"
            );
        }

        // Empty index expands nothing.
        let empty = Database::open_in_memory().unwrap();
        let (p, s) = expand_scope_via_graph(
            &empty,
            None,
            &["x.ts".to_string()],
            &["y".to_string()],
        );
        assert_eq!(p, vec!["x.ts".to_string()]);
        assert_eq!(s, vec!["y".to_string()]);
    }
}
