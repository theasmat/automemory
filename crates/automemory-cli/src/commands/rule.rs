use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::guardrails::{CustomGuardrailRule, GuardrailRule};
use chrono::Utc;
use uuid::Uuid;

pub fn list(project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    println!("=== AutoMemory Pro Built-in Rules ===");
    for r in GuardrailRule::default_rules() {
        let crit = if r.is_critical { "CRITICAL" } else { "HIGH-RISK" };
        println!("  • [{}] {} (Pattern: `{}`)", crit, r.name, r.pattern);
    }

    println!("\n=== AutoMemory Pro Custom Guardrail Rules ===");
    let custom_rules = db.list_custom_guardrail_rules(project.as_deref())?;
    if custom_rules.is_empty() {
        println!("  (No custom rules configured. Use 'automemory rule add' to create one)");
    } else {
        for cr in custom_rules {
            let status = if cr.enabled { "ENABLED" } else { "DISABLED" };
            let crit = if cr.is_critical { "CRITICAL" } else { "STANDARD" };
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

    println!("✓ Custom guardrail rule '{}' successfully registered!", name);
    println!("  ID:       {}", rule_id);
    println!("  Pattern:  `{}`", pattern);
    println!("  Critical: {}", critical);
    println!("  Action:   {}", action);

    Ok(())
}

pub fn remove(id: &str) -> Result<()> {
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
