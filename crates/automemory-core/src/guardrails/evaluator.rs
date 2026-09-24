use super::rules::GuardrailRule;
use super::tokenizer::ShellCommandTree;
use crate::db::Database;
use crate::models::AutopilotMode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailAction {
    Allow,
    Warn,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailDecision {
    pub action: GuardrailAction,
    pub severity: GuardrailSeverity,
    pub risk_score: f64,
    pub title: String,
    pub risk_summary: String,
    pub mitigation_hint: Option<String>,
    pub rule_matched: Option<String>,
    pub matched_failure_id: Option<String>,
    pub requires_confirmation: bool,
}

pub struct GuardrailEvaluator<'a> {
    db: Option<&'a Database>,
    rules: Vec<GuardrailRule>,
}

impl<'a> GuardrailEvaluator<'a> {
    pub fn new(db: Option<&'a Database>) -> Self {
        Self {
            db,
            rules: GuardrailRule::default_rules(),
        }
    }

    /// Evaluates a pre-action tool call against AST shell tokenizer checks,
    /// guardrail policies, current mode, and historical failures.
    pub fn evaluate(
        &self,
        tool_name: &str,
        args: &Value,
        project_id: Option<&str>,
        mode: AutopilotMode,
    ) -> GuardrailDecision {
        let command_str = self.extract_command_string(tool_name, args);
        let tree = ShellCommandTree::parse(&command_str);

        // 1. ── AST-Aware Structured Shell Interception (Highest Precision) ──
        if tree.is_destructive_rm_root_or_home() {
            return GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Critical,
                risk_score: 1.0,
                title: "CRITICAL STOP: Destructive Root/Home Deletion".to_string(),
                risk_summary: "Detected recursive deletion targeting system root '/' or user home directory.".to_string(),
                mitigation_hint: Some("Specify explicit project relative paths instead of root or home.".to_string()),
                rule_matched: Some("rule_rm_root_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        if tree.is_git_force_push_prod() {
            return GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Critical,
                risk_score: 1.0,
                title: "CRITICAL STOP: Force Push to Protected Branch".to_string(),
                risk_summary: "Attempting destructive git force push to production branch (main/master/prod).".to_string(),
                mitigation_hint: Some("Use standard git push or rebase safely without overwriting remote history.".to_string()),
                rule_matched: Some("rule_git_force_push_prod_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        if tree.is_raw_disk_format() {
            return GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Critical,
                risk_score: 1.0,
                title: "CRITICAL STOP: Raw Disk Write or Format".to_string(),
                risk_summary: "Detected raw block device overwrite or direct mkfs disk formatting.".to_string(),
                mitigation_hint: Some("Direct block device modifications are strictly prohibited.".to_string()),
                rule_matched: Some("rule_raw_disk_format_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        if tree.is_raw_db_drop() {
            return GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Critical,
                risk_score: 1.0,
                title: "CRITICAL STOP: Raw Database Drop or Truncate".to_string(),
                risk_summary: "Detected raw unescaped DROP DATABASE or TRUNCATE TABLE query.".to_string(),
                mitigation_hint: Some("Use reversible versioned migrations instead of raw drops.".to_string()),
                rule_matched: Some("rule_raw_db_drop_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        if tree.contains_plaintext_secret() {
            return GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Critical,
                risk_score: 1.0,
                title: "CRITICAL STOP: Plaintext Secret in Command".to_string(),
                risk_summary: "Plaintext API secret or access token detected directly in command arguments.".to_string(),
                mitigation_hint: Some("Pass credentials via environment variables instead of hardcoding.".to_string()),
                rule_matched: Some("rule_plain_secret_cli_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        // Mode-dependent High-Risk AST Checks (Strict blocks, Vibe warns)
        if tree.is_db_migration() {
            let (action, requires_confirmation) = match mode {
                AutopilotMode::Strict => (GuardrailAction::Block, true),
                _ => (GuardrailAction::Warn, false),
            };
            return GuardrailDecision {
                action,
                severity: GuardrailSeverity::Warning,
                risk_score: 0.75,
                title: "High-Risk: Database Schema Migration".to_string(),
                risk_summary: format!("Database schema migration command detected. (Enforced by {} mode)", mode),
                mitigation_hint: Some("Verify backup and ensure schema changes are backwards compatible.".to_string()),
                rule_matched: Some("rule_db_migration_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation,
            };
        }

        if tree.is_cloud_infra_delete() {
            let (action, requires_confirmation) = match mode {
                AutopilotMode::Strict => (GuardrailAction::Block, true),
                _ => (GuardrailAction::Warn, false),
            };
            return GuardrailDecision {
                action,
                severity: GuardrailSeverity::Warning,
                risk_score: 0.8,
                title: "High-Risk: Cloud Infrastructure Teardown".to_string(),
                risk_summary: format!("Destructive cloud/container teardown detected. (Enforced by {} mode)", mode),
                mitigation_hint: Some("Verify target cluster and namespace before destroying resources.".to_string()),
                rule_matched: Some("rule_cloud_infra_delete_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation,
            };
        }

        if tree.is_destructive_git_reset() {
            let (action, requires_confirmation) = match mode {
                AutopilotMode::Strict => (GuardrailAction::Block, true),
                _ => (GuardrailAction::Warn, false),
            };
            return GuardrailDecision {
                action,
                severity: GuardrailSeverity::Warning,
                risk_score: 0.7,
                title: "High-Risk: Destructive Git Reset/Clean".to_string(),
                risk_summary: format!("Git reset --hard or git clean will discard uncommitted changes. (Enforced by {} mode)", mode),
                mitigation_hint: Some("Run git stash before resetting to prevent unrecoverable loss.".to_string()),
                rule_matched: Some("rule_git_hard_reset_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation,
            };
        }

        // 2. ── Regex Rule Matching (File Tampering & Supplemental Patterns) ──
        for rule in &self.rules {
            if let Ok(re) = Regex::new(&rule.pattern) {
                if re.is_match(&command_str) {
                    if rule.is_critical {
                        return GuardrailDecision {
                            action: GuardrailAction::Block,
                            severity: GuardrailSeverity::Critical,
                            risk_score: 1.0,
                            title: format!("CRITICAL STOP: {}", rule.name),
                            risk_summary: rule.description.clone(),
                            mitigation_hint: Some(rule.mitigation_hint.clone()),
                            rule_matched: Some(rule.id.clone()),
                            matched_failure_id: None,
                            requires_confirmation: true,
                        };
                    }

                    let (action, requires_confirmation) = match mode {
                        AutopilotMode::Strict => (GuardrailAction::Block, true),
                        AutopilotMode::Vibe => (GuardrailAction::Warn, false),
                        AutopilotMode::Review => (GuardrailAction::Warn, false),
                    };

                    return GuardrailDecision {
                        action,
                        severity: GuardrailSeverity::Warning,
                        risk_score: 0.75,
                        title: format!("High-Risk Guardrail: {}", rule.name),
                        risk_summary: format!("{} (Enforced by {} mode)", rule.description, mode),
                        mitigation_hint: Some(rule.mitigation_hint.clone()),
                        rule_matched: Some(rule.id.clone()),
                        matched_failure_id: None,
                        requires_confirmation,
                    };
                }
            }
        }

        // 3. ── Check for Matching Historical Failures ──
        if let Some(db) = self.db {
            if let Ok(failures) = db.list_failures(project_id, Some("unresolved"), 20) {
                for f in failures {
                    if self.matches_failure_signature(&command_str, &f.error_message, f.failed_tool.as_deref()) {
                        let (action, requires_confirmation) = match mode {
                            AutopilotMode::Strict => (GuardrailAction::Block, true),
                            _ => (GuardrailAction::Warn, false),
                        };

                        return GuardrailDecision {
                            action,
                            severity: GuardrailSeverity::Warning,
                            risk_score: 0.8,
                            title: format!("Known Gotcha Alert: {}", f.task_title),
                            risk_summary: format!(
                                "This action matches a known unresolved failure: {}. Root cause: {}",
                                f.error_message,
                                f.root_cause_hypothesis.unwrap_or_else(|| "Unknown".to_string())
                            ),
                            mitigation_hint: Some("Review past failure resolution or test cautiously.".to_string()),
                            rule_matched: Some("known_failure_match".to_string()),
                            matched_failure_id: Some(f.id),
                            requires_confirmation,
                        };
                    }
                }
            }
        }

        // Permitted / Standard Safe Action
        GuardrailDecision {
            action: GuardrailAction::Allow,
            severity: GuardrailSeverity::Info,
            risk_score: 0.0,
            title: "Action Permitted".to_string(),
            risk_summary: "No safety violations or risk triggers detected.".to_string(),
            mitigation_hint: None,
            rule_matched: None,
            matched_failure_id: None,
            requires_confirmation: false,
        }
    }

    fn extract_command_string(&self, tool_name: &str, args: &Value) -> String {
        let mut parts = Vec::new();
        parts.push(tool_name.to_string());

        if let Some(cmd) = args.get("CommandLine").and_then(|v| v.as_str()) {
            parts.push(cmd.to_string());
        }
        if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
            parts.push(cmd.to_string());
        }
        if let Some(path) = args.get("TargetFile").and_then(|v| v.as_str()) {
            parts.push(path.to_string());
        }
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            parts.push(path.to_string());
        }
        if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
            parts.push(query.to_string());
        }

        if parts.len() == 1 {
            parts.push(args.to_string());
        }

        parts.join(" ")
    }

    fn matches_failure_signature(&self, command_str: &str, error_msg: &str, failed_tool: Option<&str>) -> bool {
        if let Some(tool) = failed_tool {
            if !command_str.contains(tool) {
                return false;
            }
        }
        let keywords = ["asan", "leak", "segfault", "migration", "lock", "timeout"];
        for kw in keywords {
            if error_msg.to_lowercase().contains(kw) && command_str.to_lowercase().contains(kw) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_catastrophic_blocking_in_vibe_mode() {
        let evaluator = GuardrailEvaluator::new(None);
        let args = serde_json::json!({ "CommandLine": "rm -rf /" });
        let decision = evaluator.evaluate("run_command", &args, None, AutopilotMode::Vibe);

        assert_eq!(decision.action, GuardrailAction::Block);
        assert_eq!(decision.severity, GuardrailSeverity::Critical);
        assert!(decision.requires_confirmation);
    }

    #[test]
    fn test_subshell_wrapped_rm_root() {
        let evaluator = GuardrailEvaluator::new(None);
        let args = serde_json::json!({ "CommandLine": "sudo sh -c 'rm -rf /'" });
        let decision = evaluator.evaluate("run_command", &args, None, AutopilotMode::Vibe);

        assert_eq!(decision.action, GuardrailAction::Block);
        assert_eq!(decision.severity, GuardrailSeverity::Critical);
    }

    #[test]
    fn test_high_risk_migration_strict_vs_vibe() {
        let evaluator = GuardrailEvaluator::new(None);
        let args = serde_json::json!({ "CommandLine": "prisma migrate deploy" });

        // In Strict mode: must block and require confirmation
        let strict_decision = evaluator.evaluate("run_command", &args, None, AutopilotMode::Strict);
        assert_eq!(strict_decision.action, GuardrailAction::Block);
        assert!(strict_decision.requires_confirmation);

        // In Vibe mode: warns without blocking
        let vibe_decision = evaluator.evaluate("run_command", &args, None, AutopilotMode::Vibe);
        assert_eq!(vibe_decision.action, GuardrailAction::Warn);
        assert!(!vibe_decision.requires_confirmation);
    }

    #[test]
    fn test_safe_test_execution() {
        let evaluator = GuardrailEvaluator::new(None);
        let args = serde_json::json!({ "CommandLine": "cargo test --workspace" });
        let decision = evaluator.evaluate("run_command", &args, None, AutopilotMode::Strict);

        assert_eq!(decision.action, GuardrailAction::Allow);
        assert_eq!(decision.risk_score, 0.0);
    }
}
