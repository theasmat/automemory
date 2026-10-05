use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::guardrails::{GuardrailAction, GuardrailEvaluator};
use automemory_core::models::AutopilotMode;
use serde_json::json;
use std::str::FromStr;

/// Evaluate a shell command against guardrail rules without executing it.
/// Exit codes are script-friendly: 0 allow, 1 warn, 2 block.
pub fn test_command(command: &str, mode: Option<String>, project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let autopilot_mode = mode
        .as_deref()
        .and_then(|m| AutopilotMode::from_str(m).ok())
        .unwrap_or(AutopilotMode::Vibe);

    let evaluator = GuardrailEvaluator::new(Some(&db));
    let args = json!({ "command": command });
    let decision = evaluator.evaluate("run_command", &args, project.as_deref(), autopilot_mode);

    let verdict = match decision.action {
        GuardrailAction::Block => "BLOCK",
        GuardrailAction::Warn => "WARN",
        GuardrailAction::Allow => "ALLOW",
    };
    println!(
        "[{}] {} (risk {:.2})",
        verdict, decision.title, decision.risk_score
    );
    println!("  Summary: {}", decision.risk_summary);
    if let Some(ref hint) = decision.mitigation_hint {
        println!("  Mitigation: {}", hint);
    }

    match decision.action {
        GuardrailAction::Block => std::process::exit(2),
        GuardrailAction::Warn => std::process::exit(1),
        GuardrailAction::Allow => Ok(()),
    }
}
