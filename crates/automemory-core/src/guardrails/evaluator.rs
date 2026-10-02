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

        // Mode-dependent High-Risk AST Checks (Strict blocks, Review queues proposal, Vibe warns)
        if tree.is_db_migration() {
            return self.handle_high_risk(
                "rule_db_migration_ast",
                "High-Risk: Database Schema Migration",
                "Database schema migration command detected.",
                Some("Verify backup and ensure schema changes are backwards compatible.".to_string()),
                &command_str,
                project_id,
                mode,
                0.75,
                None,
            );
        }

        if tree.is_cloud_infra_delete() {
            return self.handle_high_risk(
                "rule_cloud_infra_delete_ast",
                "High-Risk: Cloud Infrastructure Teardown",
                "Destructive cloud/container teardown detected.",
                Some("Verify target cluster and namespace before destroying resources.".to_string()),
                &command_str,
                project_id,
                mode,
                0.8,
                None,
            );
        }

        if tree.is_destructive_git_reset() {
            return self.handle_high_risk(
                "rule_git_hard_reset_ast",
                "High-Risk: Destructive Git Reset/Clean",
                "Git reset --hard or git clean will discard uncommitted changes.",
                Some("Run git stash before resetting to prevent unrecoverable loss.".to_string()),
                &command_str,
                project_id,
                mode,
                0.7,
                None,
            );
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

                    return self.handle_high_risk(
                        &rule.id,
                        &format!("High-Risk Guardrail: {}", rule.name),
                        &rule.description,
                        Some(rule.mitigation_hint.clone()),
                        &command_str,
                        project_id,
                        mode,
                        0.75,
                        None,
                    );
                }
            }
        }

        // 3. ── Check for Matching Historical Failures ──
        if let Some(db) = self.db {
            if let Ok(failures) = db.list_failures(project_id, Some("unresolved"), 20) {
                for f in failures {
                    if self.matches_failure_signature(&command_str, &f.error_message, f.failed_tool.as_deref()) {
                        return self.handle_high_risk(
                            "known_failure_match",
                            &format!("Known Gotcha Alert: {}", f.task_title),
                            &format!(
                                "This action matches a known unresolved failure: {}. Root cause: {}",
                                f.error_message,
                                f.root_cause_hypothesis.unwrap_or_else(|| "Unknown".to_string())
                            ),
                            Some("Review past failure resolution or test cautiously.".to_string()),
                            &command_str,
                            project_id,
                            mode,
                            0.8,
                            Some(f.id),
                        );
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
        let effective_args = if let Some(sub_args) = args.get("args").or_else(|| args.get("toolArgs")) {
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

    fn handle_high_risk(
        &self,
        rule_id: &str,
        title: &str,
        risk_summary: &str,
        mitigation_hint: Option<String>,
        command_str: &str,
        project_id: Option<&str>,
        mode: AutopilotMode,
        risk_score: f64,
        matched_failure_id: Option<String>,
    ) -> GuardrailDecision {
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
                    if let Ok(Some(_approved)) = db.find_approved_proposal(project_id, rule_id, &desc) {
                        return GuardrailDecision {
                            action: GuardrailAction::Allow,
                            severity: GuardrailSeverity::Info,
                            risk_score: 0.0,
                            title: format!("Approved: {}", title),
                            risk_summary: "Action was previously reviewed and approved by operator.".to_string(),
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
                        risk_summary: format!("{} (Review mode active; queued for review)", risk_summary),
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

    #[test]
    fn test_high_risk_migration_review_mode_proposals_lifecycle() {
        let db = Database::open_in_memory().unwrap();

        let evaluator = GuardrailEvaluator::new(Some(&db));
        let args = serde_json::json!({ "CommandLine": "npx prisma migrate reset --force" });

        // 1. In Review mode: initial run must block and create a pending proposal
        let review_decision = evaluator.evaluate("run_command", &args, Some("proj-test"), AutopilotMode::Review);
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
        let approved_decision = evaluator.evaluate("run_command", &args, Some("proj-test"), AutopilotMode::Review);
        assert_eq!(approved_decision.action, GuardrailAction::Allow);
        assert_eq!(approved_decision.risk_score, 0.0);
    }
}
