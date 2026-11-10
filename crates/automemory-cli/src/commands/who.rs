use anyhow::Result;
use automemory_core::history;

/// Who changed `path` and why, from Tracewise when it is installed.
pub fn run(path: &str, limit: usize, json: bool) -> Result<()> {
    let root = std::env::current_dir().unwrap_or_default();
    let installed = history::find_tw().is_some();
    let changes = history::detect(&root).recent_changes(path, limit);
    if json {
        let rows: Vec<_> = changes
            .iter()
            .map(|c| serde_json::json!({"actor": c.actor, "kind": c.kind, "session": c.session, "when": c.when, "why": c.why}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"path": path, "tracewise": installed, "changes": rows}))?);
        return Ok(());
    }
    if !installed {
        println!("Tracewise (`tw`) is not installed, so there is no change history. Install it to see who changed a file and why.");
    } else if changes.is_empty() {
        println!("No recorded changes to {path}.");
    } else {
        println!("Recent changes to {path}");
        for c in &changes {
            println!("  {}", c.describe());
        }
    }
    Ok(())
}
