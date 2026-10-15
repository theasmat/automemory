use anyhow::{Context, Result};
use automemory_core::auth;
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use automemory_core::guardrails::{CustomGuardrailRule, GuardrailRule};
use chrono::Utc;
use uuid::Uuid;

/// Sets (or replaces) the human password guarding safety control-plane
/// writes. With `--from-env`, reads the secret from AUTOMEMORY_PASSWORD
/// instead of prompting (for non-TTY setup).
pub fn passwd(from_env: bool) -> Result<()> {
    let secret = if from_env {
        std::env::var(auth::PASSWORD_ENV_VAR).with_context(|| {
            format!(
                "{} is not set; nothing changed.",
                auth::PASSWORD_ENV_VAR
            )
        })?
    } else {
        auth::prompt_new_password()?
    };
    if secret.trim().is_empty() {
        anyhow::bail!("Empty password rejected; nothing changed.");
    }
    let mut cfg = GlobalConfig::load().unwrap_or_default();
    cfg.security.password_hash = Some(auth::hash_password(&secret));
    cfg.save()?;
    println!("✓ Human password set. Safety writes (rule/hooks/mode changes, dashboard rule edits) now require it.");
    println!("  Non-interactive use: export {}=<password>.", auth::PASSWORD_ENV_VAR);
    Ok(())
}

pub fn list(project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    println!("=== AutoMemory Built-in Rules ===");
    for r in GuardrailRule::default_rules() {
        let crit = if r.is_critical {
            "CRITICAL"
        } else {
            "HIGH-RISK"
        };
        println!("  • [{}] {} (Pattern: `{}`)", crit, r.name, r.pattern);
    }

    println!("\n=== AutoMemory Custom Guardrail Rules ===");
    let custom_rules = db.list_custom_guardrail_rules(project.as_deref())?;
    if custom_rules.is_empty() {
        println!("  (No custom rules configured. Use 'automemory rule add' to create one)");
    } else {
        for cr in custom_rules {
            let status = if cr.enabled { "ENABLED" } else { "DISABLED" };
            let crit = if cr.is_critical {
                "CRITICAL"
            } else {
                "STANDARD"
            };
            let scope = cr.project_id.as_deref().unwrap_or("global");
            println!(
                "  • [{}] [{}] [{}] ID: {}\n    Name: {}\n    Pattern: `{}`\n    Description: {}\n    Scope: {}",
                status, cr.action.to_uppercase(), crit, cr.id, cr.name, cr.pattern, cr.description, scope
            );
        }
    }

    Ok(())
}

pub fn add(
    name: String,
    pattern: String,
    critical: bool,
    action: String,
    description: String,
    mitigation: String,
    project: Option<String>,
) -> Result<()> {
    auth::require_human(
        auth::stored_password_hash().as_deref(),
        "rule add",
    )?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    // Verify regex compiles
    regex::Regex::new(&pattern).with_context(|| format!("Invalid regex pattern: '{}'", pattern))?;

    let rule_id = format!("custom_{}", Uuid::now_v7());
    let custom_rule = CustomGuardrailRule {
        id: rule_id.clone(),
        project_id: project,
        name: name.clone(),
        category: "custom_rule".to_string(),
        pattern: pattern.clone(),
        is_critical: critical,
        action: action.to_lowercase(),
        description,
        mitigation_hint: mitigation,
        enabled: true,
        created_at: Utc::now(),
    };

    db.save_custom_guardrail_rule(&custom_rule)?;

    println!(
        "✓ Custom guardrail rule '{}' successfully registered!",
        name
    );
    println!("  ID:       {}", rule_id);
    println!("  Pattern:  `{}`", pattern);
    println!("  Critical: {}", critical);
    println!("  Action:   {}", action);

    Ok(())
}

pub fn remove(id: &str) -> Result<()> {
    auth::require_human(
        auth::stored_password_hash().as_deref(),
        "rule remove",
    )?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    if db.delete_custom_guardrail_rule(id)? {
        println!("✓ Custom guardrail rule '{}' removed successfully.", id);
    } else {
        println!("✗ Custom guardrail rule '{}' not found.", id);
    }

    Ok(())
}

pub fn toggle(id: &str, enabled: bool) -> Result<()> {
    auth::require_human(
        auth::stored_password_hash().as_deref(),
        "rule toggle",
    )?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    if db.toggle_custom_guardrail_rule(id, enabled)? {
        let status = if enabled { "enabled" } else { "disabled" };
        println!("✓ Custom guardrail rule '{}' is now {}.", id, status);
    } else {
        println!("✗ Custom guardrail rule '{}' not found.", id);
    }

    Ok(())
}
