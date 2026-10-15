use super::rules::GuardrailRule;
use super::tokenizer::ShellCommandTree;
use crate::db::Database;
use crate::models::AutopilotMode;
use crate::ports::RuleMatcher;
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
    extra: Vec<Box<dyn RuleMatcher>>,
}

/// Tool names that execute shell commands (canonical hook names and raw variants).
fn is_command_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "command" | "run_command" | "shell" | "bash" | "sh" | "zsh" | "exec" | "run_shell_command"
    )
}

/// True when the payload carries inspectable command text under a known key.
fn has_command_text(args: &Value) -> bool {
    let effective = args
        .get("args")
        .or_else(|| args.get("toolArgs"))
        .unwrap_or(args);
    effective
        .get("CommandLine")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
        || effective
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty())
}

/// Bundled inputs for a high-risk guardrail decision.
struct HighRiskInput<'a> {
    rule_id: &'a str,
    title: &'a str,
    risk_summary: &'a str,
    mitigation_hint: Option<String>,
    command_str: &'a str,
    project_id: Option<&'a str>,
    mode: AutopilotMode,
    risk_score: f64,
    matched_failure_id: Option<String>,
}

impl<'a> GuardrailEvaluator<'a> {
    pub fn new(db: Option<&'a Database>) -> Self {
        let mut rules = GuardrailRule::default_rules();
        if let Some(d) = db {
            if let Ok(custom_rules) = d.list_custom_guardrail_rules(None) {
                for cr in custom_rules {
                    if cr.enabled {
                        rules.push(cr.into());
                    }
                }
            }
        }
        Self {
            db,
            rules,
            extra: Vec::new(),
        }
    }

    /// Adds a custom rule implementation (e.g. an org policy plugin).
    /// Extra rules run before the built-in set.
    pub fn with_extra_rule(mut self, rule: Box<dyn RuleMatcher>) -> Self {
        self.extra.push(rule);
        self
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
        // 0. ── Fail-closed envelope check: a command-execution tool carrying no
        // inspectable command text must never silently pass as benign.
        if is_command_tool(tool_name) && !has_command_text(args) {
            return GuardrailDecision {
                action: GuardrailAction::Warn,
                severity: GuardrailSeverity::Warning,
                risk_score: 0.5,
                title: "Uninspectable Command Payload".to_string(),
                risk_summary: "Command-execution tool call carried no inspectable command text (missing args.command). Treated as untrusted.".to_string(),
                mitigation_hint: Some("Send run_command with args.command set, or probe the payload with `automemory guardrail test --command`.".to_string()),
                rule_matched: Some("rule_uninspectable_payload".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        let command_str = self.extract_command_string(tool_name, args);
        let tree = ShellCommandTree::parse(&command_str);

        // 1. ── AST-Aware Structured Shell Interception (Highest Precision) ──
        if tree.is_destructive_rm_root_or_home() {
            return GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Critical,
                risk_score: 1.0,
                title: "CRITICAL STOP: Destructive Root/Home Deletion".to_string(),
                risk_summary:
                    "Detected recursive deletion targeting system root '/' or user home directory."
                        .to_string(),
                mitigation_hint: Some(
                    "Specify explicit project relative paths instead of root or home.".to_string(),
                ),
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
                risk_summary:
                    "Attempting destructive git force push to production branch (main/master/prod)."
                        .to_string(),
                mitigation_hint: Some(
                    "Use standard git push or rebase safely without overwriting remote history."
                        .to_string(),
                ),
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
                risk_summary: "Detected raw block device overwrite or direct mkfs disk formatting."
                    .to_string(),
                mitigation_hint: Some(
                    "Direct block device modifications are strictly prohibited.".to_string(),
                ),
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
                risk_summary: "Detected raw unescaped DROP DATABASE or TRUNCATE TABLE query."
                    .to_string(),
                mitigation_hint: Some(
                    "Use reversible versioned migrations instead of raw drops.".to_string(),
                ),
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
                risk_summary:
                    "Plaintext API secret or access token detected directly in command arguments."
                        .to_string(),
                mitigation_hint: Some(
                    "Pass credentials via environment variables instead of hardcoding.".to_string(),
                ),
                rule_matched: Some("rule_plain_secret_cli_ast".to_string()),
                matched_failure_id: None,
                requires_confirmation: true,
            };
        }

        // Mode-dependent High-Risk AST Checks (Strict blocks, Review queues proposal, Vibe warns)
        if tree.is_db_migration() {
            return self.handle_high_risk(HighRiskInput {
                rule_id: "rule_db_migration_ast",
                title: "High-Risk: Database Schema Migration",
                risk_summary: "Database schema migration command detected.",
                mitigation_hint: Some(
                    "Verify backup and ensure schema changes are backwards compatible.".to_string(),
                ),
                command_str: &command_str,
                project_id,
                mode,
                risk_score: 0.75,
                matched_failure_id: None,
            });
        }

        if tree.is_cloud_infra_delete() {
            return self.handle_high_risk(HighRiskInput {
                rule_id: "rule_cloud_infra_delete_ast",
                title: "High-Risk: Cloud Infrastructure Teardown",
                risk_summary: "Destructive cloud/container teardown detected.",
                mitigation_hint: Some(
                    "Verify target cluster and namespace before destroying resources.".to_string(),
                ),
                command_str: &command_str,
                project_id,
                mode,
                risk_score: 0.8,
                matched_failure_id: None,
            });
        }

        if tree.is_destructive_git_reset() {
            return self.handle_high_risk(HighRiskInput {
                rule_id: "rule_git_hard_reset_ast",
                title: "High-Risk: Destructive Git Reset/Clean",
                risk_summary: "Git reset --hard or git clean will discard uncommitted changes.",
                mitigation_hint: Some(
                    "Run git stash before resetting to prevent unrecoverable loss.".to_string(),
                ),
                command_str: &command_str,
                project_id,
                mode,
                risk_score: 0.7,
                matched_failure_id: None,
            });
        }

        // 1b. ── Extra RuleMatcher plugins (custom org policies first) ──
        for rule in &self.extra {
            if rule.matches_text(&command_str) {
                return self.handle_high_risk(HighRiskInput {
                    rule_id: "custom_plugin_rule",
                    title: &format!("Custom Guardrail: {}", rule.rule_id()),
                    risk_summary: "An installed guardrail plugin flagged this command.",
                    mitigation_hint: Some(
                        "Review the plugin policy or run in shadow mode to audit it.".to_string(),
                    ),
                    command_str: &command_str,
                    project_id,
                    mode,
                    risk_score: 0.75,
                    matched_failure_id: None,
                });
            }
        }

        // 2. ── Regex Rule Matching (File Tampering & Supplemental Patterns) ──
        for rule in &self.rules {
            if rule.matches_text(&command_str) {
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

                return self.handle_high_risk(HighRiskInput {
                    rule_id: &rule.id,
                    title: &format!("High-Risk Guardrail: {}", rule.name),
                    risk_summary: &rule.description,
                    mitigation_hint: Some(rule.mitigation_hint.clone()),
                    command_str: &command_str,
                    project_id,
                    mode,
                    risk_score: 0.75,
                    matched_failure_id: None,
                });
            }
        }

        // 3. ── Check for Matching Historical Failures ──
        if let Some(db) = self.db {
            if let Ok(failures) = db.list_failures(project_id, Some("unresolved"), 20) {
                for f in failures {
                    if self.matches_failure_signature(
                        &command_str,
                        &f.error_message,
                        f.failed_tool.as_deref(),
                    ) {
                        return self.handle_high_risk(HighRiskInput {
                            rule_id: "known_failure_match",
                            title: &format!("Known Gotcha Alert: {}", f.task_title),
                            risk_summary: &format!(
                                "This action matches a known unresolved failure: {}. Root cause: {}",
                                f.error_message,
                                f.root_cause_hypothesis.unwrap_or_else(|| "Unknown".to_string())
                            ),
                            mitigation_hint: Some("Review past failure resolution or test cautiously.".to_string()),
                            command_str: &command_str,
                            project_id,
                            mode,
                            risk_score: 0.8,
                            matched_failure_id: Some(f.id),
                        });
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
        let effective_args =
            if let Some(sub_args) = args.get("args").or_else(|| args.get("toolArgs")) {
                sub_args
            } else {
                args
            };

        if let Some(cmd) = effective_args.get("CommandLine").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
        if let Some(cmd) = effective_args.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }

        let mut parts = Vec::new();
        parts.push(tool_name.to_string());

        if let Some(path) = effective_args.get("TargetFile").and_then(|v| v.as_str()) {
            parts.push(path.to_string());
        }
        if let Some(path) = effective_args.get("path").and_then(|v| v.as_str()) {
            parts.push(path.to_string());
        }
        if let Some(query) = effective_args.get("query").and_then(|v| v.as_str()) {
            parts.push(query.to_string());
        }

        if parts.len() == 1 {
            parts.push(effective_args.to_string());
        }

        parts.join(" ")
    }

    fn matches_failure_signature(
        &self,
        command_str: &str,
        error_msg: &str,
        failed_tool: Option<&str>,
    ) -> bool {
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

    fn handle_high_risk(&self, input: HighRiskInput) -> GuardrailDecision {
        let HighRiskInput {
            rule_id,
            title,
            risk_summary,
            mitigation_hint,
            command_str,
            project_id,
            mode,
            risk_score,
            matched_failure_id,
        } = input;
        match mode {
            AutopilotMode::Strict => GuardrailDecision {
                action: GuardrailAction::Block,
                severity: GuardrailSeverity::Warning,
                risk_score,
                title: title.to_string(),
                risk_summary: format!("{} (Enforced by strict mode)", risk_summary),
                mitigation_hint,
                rule_matched: Some(rule_id.to_string()),
                matched_failure_id,
                requires_confirmation: true,
            },
            AutopilotMode::Review => {
                let desc = format!("Command requires operator review: {}", command_str);
                if let Some(db) = self.db {
                    if let Ok(Some(_approved)) =
                        db.find_approved_proposal(project_id, rule_id, &desc)
                    {
                        return GuardrailDecision {
                            action: GuardrailAction::Allow,
                            severity: GuardrailSeverity::Info,
                            risk_score: 0.0,
                            title: format!("Approved: {}", title),
                            risk_summary:
                                "Action was previously reviewed and approved by operator."
                                    .to_string(),
                            mitigation_hint: None,
                            rule_matched: Some(rule_id.to_string()),
                            matched_failure_id,
                            requires_confirmation: false,
                        };
                    }

                    let prop_id = match db.find_pending_proposal(project_id, rule_id, &desc) {
                        Ok(Some(existing)) => existing.id,
                        _ => {
                            let new_id = uuid::Uuid::now_v7().to_string();
                            let prop = crate::models::ReviewProposal {
                                id: new_id.clone(),
                                project_id: project_id.map(String::from),
                                proposal_type: "tool_execution_review".to_string(),
                                target_id: rule_id.to_string(),
                                description: desc,
                                suggested_action: format!("allow {}", command_str),
                                status: "pending".to_string(),
                                created_at: chrono::Utc::now(),
                                decided_at: None,
                            };
                            let _ = db.save_proposal(&prop);
                            new_id
                        }
                    };

                    GuardrailDecision {
                        action: GuardrailAction::Block,
                        severity: GuardrailSeverity::Warning,
                        risk_score,
                        title: format!("Review Required: {}", title),
                        risk_summary: format!(
                            "Command queued for operator review in REVIEW mode (Proposal ID: {}). Use memory.proposals.decide to approve or reject.",
                            prop_id
                        ),
                        mitigation_hint: Some(format!("Operator approval required for proposal '{}' before command can proceed.", prop_id)),
                        rule_matched: Some(rule_id.to_string()),
                        matched_failure_id,
                        requires_confirmation: true,
                    }
                } else {
                    GuardrailDecision {
                        action: GuardrailAction::Block,
                        severity: GuardrailSeverity::Warning,
                        risk_score,
                        title: format!("Review Required: {}", title),
                        risk_summary: format!(
                            "{} (Review mode active; queued for review)",
                            risk_summary
                        ),
                        mitigation_hint,
                        rule_matched: Some(rule_id.to_string()),
                        matched_failure_id,
                        requires_confirmation: true,
                    }
                }
            }
            AutopilotMode::Vibe => GuardrailDecision {
                action: GuardrailAction::Warn,
                severity: GuardrailSeverity::Warning,
                risk_score,
                title: title.to_string(),
                risk_summary: format!("{} (Allowed with warning in vibe mode)", risk_summary),
                mitigation_hint,
                rule_matched: Some(rule_id.to_string()),
                matched_failure_id,
                requires_confirmation: false,
            },
        }
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
    fn test_extra_rule_plugin_runs_before_builtins() {
        struct OrgDenyDeploy;
        impl crate::ports::RuleMatcher for OrgDenyDeploy {
            fn rule_id(&self) -> &str {
                "org-deny-deploy"
            }
            fn matches_text(&self, text: &str) -> bool {
                text.contains("deploy-to-prod")
            }
        }
        let evaluator = GuardrailEvaluator::new(None).with_extra_rule(Box::new(OrgDenyDeploy));
        let args = serde_json::json!({ "CommandLine": "run deploy-to-prod now" });
        let decision = evaluator.evaluate("run_command", &args, None, AutopilotMode::Vibe);
        assert_eq!(decision.action, GuardrailAction::Warn);
        assert_eq!(decision.rule_matched.as_deref(), Some("custom_plugin_rule"));
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
    fn test_uninspectable_payload_warns_instead_of_allowing() {
        let evaluator = GuardrailEvaluator::new(None);
        for tool in ["run_command", "command", "shell"] {
            for args in [
                serde_json::json!({}),
                serde_json::json!({ "args": {} }),
                serde_json::json!({ "command": "   " }),
            ] {
                let decision = evaluator.evaluate(tool, &args, None, AutopilotMode::Vibe);
                assert_eq!(
                    decision.action,
                    GuardrailAction::Warn,
                    "tool={tool} args={args}"
                );
                assert_eq!(
                    decision.rule_matched.as_deref(),
                    Some("rule_uninspectable_payload")
                );
            }
        }
        // Non-command tools with empty args still pass.
        let decision =
            evaluator.evaluate("read", &serde_json::json!({}), None, AutopilotMode::Vibe);
        assert_eq!(decision.action, GuardrailAction::Allow);
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

    #[test]
    fn test_high_risk_migration_review_mode_proposals_lifecycle() {
        let db = Database::open_in_memory().unwrap();

        let evaluator = GuardrailEvaluator::new(Some(&db));
        let args = serde_json::json!({ "CommandLine": "npx prisma migrate reset --force" });

        // 1. In Review mode: initial run must block and create a pending proposal
        let review_decision = evaluator.evaluate(
            "run_command",
            &args,
            Some("proj-test"),
            AutopilotMode::Review,
        );
        assert_eq!(review_decision.action, GuardrailAction::Block);
        assert!(review_decision.requires_confirmation);
        assert!(review_decision.risk_summary.contains("Proposal ID:"));

        // 2. Verify the pending proposal exists in the database
        let pending = db.list_pending_proposals(Some("proj-test")).unwrap();
        assert_eq!(pending.len(), 1);
        let prop = &pending[0];
        assert_eq!(prop.proposal_type, "tool_execution_review");
        assert_eq!(prop.target_id, "rule_db_migration_ast");

        // 3. Operator approves the proposal
        let decided = db.decide_proposal(&prop.id, true).unwrap();
        assert!(decided);

        // 4. In Review mode: subsequent run now succeeds with Allow
        let approved_decision = evaluator.evaluate(
            "run_command",
            &args,
            Some("proj-test"),
            AutopilotMode::Review,
        );
        assert_eq!(approved_decision.action, GuardrailAction::Allow);
        assert_eq!(approved_decision.risk_score, 0.0);
    }
}
