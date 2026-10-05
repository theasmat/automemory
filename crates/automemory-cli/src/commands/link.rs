use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use std::fs;
use std::path::Path;

pub fn execute(project_id_arg: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let config = Config::default_config();

    // 1. Determine project ID
    let project_id = match project_id_arg {
        Some(p) => p,
        None => infer_project_id(&cwd),
    };

    println!(
        "Linking project '{}' to OpenViking-style single-source context...",
        project_id
    );

    // 2. Canonical context directory
    let context_root = config
        .home_dir
        .join("context")
        .join("projects")
        .join(&project_id);
    let memories_dir = context_root.join("memories");
    let skills_dir = context_root.join("skills");
    let docs_dir = context_root.join("docs");

    fs::create_dir_all(&memories_dir)?;
    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(docs_dir.join("prd"))?;
    fs::create_dir_all(docs_dir.join("adr"))?;
    fs::create_dir_all(docs_dir.join("business-rules"))?;
    fs::create_dir_all(docs_dir.join("api"))?;

    // Seed L0 and L1 summaries if absent
    let abstract_file = context_root.join(".abstract.md");
    if !abstract_file.exists() {
        let abstract_content = format!(
            "# Project: {}\nCanonical context store and single source of truth for {}.\n",
            project_id, project_id
        );
        fs::write(&abstract_file, abstract_content)?;
    }

    let overview_file = context_root.join(".overview.md");
    if !overview_file.exists() {
        let overview_content = format!(
            "# {} Context Overview\n\n\
            ### System Information\n\
            - **Project Slug**: `{}`\n\
            - **Canonical Storage**: `{}`\n\n\
            ### Active Decisions & Guidelines\n\
            (AutoMemory automatically compiles active decisions and playbooks here)\n",
            project_id,
            project_id,
            context_root.display()
        );
        fs::write(&overview_file, overview_content)?;
    }

    // 3. Sync existing DB memories to markdown files in canonical store
    if let Ok(db) = Database::open(&config.db_path) {
        if let Ok(mems) = db.list_memories(Some(&project_id), None, None, 100) {
            for m in mems {
                let filename = format!(
                    "{}-{}.md",
                    sanitize_filename(&m.memory_type.to_string()),
                    sanitize_filename(&m.summary)
                );
                let filepath = memories_dir.join(filename);
                let content = format!(
                    "---\nid: \"{}\"\ntype: \"{}\"\nconfidence: {}\ntags: {:?}\nupdated_at: \"{}\"\n---\n\n# {}\n\n{}\n",
                    m.id, m.memory_type, m.confidence, m.tags, m.updated_at.to_rfc3339(), m.summary, m.content
                );
                let _ = fs::write(filepath, content);
            }
        }

        if let Ok(skills) = db.list_skills(Some(&project_id), 100) {
            for s in skills {
                let filename = format!("skill-{}.md", sanitize_filename(&s.name));
                let filepath = skills_dir.join(filename);
                let steps_md = s
                    .steps
                    .iter()
                    .enumerate()
                    .map(|(i, step)| format!("{}. {}\n", i + 1, step))
                    .collect::<Vec<_>>()
                    .join("\n");

                let content = format!(
                    "---\nid: \"{}\"\nname: \"{}\"\nsuccess_count: {}\n---\n\n# {}\n\n{}\n\n### Executable Procedure\n\n{}\n",
                    s.id, s.name, s.success_count, s.name, s.description, steps_md
                );
                let _ = fs::write(filepath, content);
            }
        }
    }

    // 4. Create local .context symlink in project directory
    let symlink_path = cwd.join(".context");
    create_or_update_symlink(&context_root, &symlink_path)?;

    // 5. Ensure .context and .automemory are added to .gitignore
    update_gitignore(&cwd)?;

    println!("✓ Successfully linked OpenViking-style single source of truth!");
    println!("  📁 Canonical Storage: {}", context_root.display());
    println!(
        "  🔗 Local Symlink:     {} -> {}",
        symlink_path.display(),
        context_root.display()
    );
    println!(
        "  📄 Tiered Context:    L0 (.abstract.md), L1 (.overview.md), L2 (memories/ & docs/)"
    );
    println!("  🛡️  Git Protection:    .context and .automemory verified in .gitignore");

    Ok(())
}

fn infer_project_id(cwd: &Path) -> String {
    // 1. Try reading .automemory/config.json if present
    let cfg_path = cwd.join(".automemory").join("config.json");
    if let Ok(content) = fs::read_to_string(&cfg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(id) = json.get("project_id").and_then(|v| v.as_str()) {
                return id.to_string();
            }
        }
    }

    // 2. Try git root directory name or fallback to cwd folder name
    cwd.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "default-project".to_string())
}

fn create_or_update_symlink(target: &Path, link_path: &Path) -> Result<()> {
    if link_path.exists() || link_path.is_symlink() {
        if let Ok(existing_target) = fs::read_link(link_path) {
            if existing_target == target {
                return Ok(());
            }
        }
        // Remove existing link/file before recreating
        let _ = fs::remove_file(link_path);
        let _ = fs::remove_dir(link_path);
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link_path).with_context(|| {
            format!(
                "Failed to create symlink from {:?} to {:?}",
                link_path, target
            )
        })?;
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link_path).with_context(|| {
            format!(
                "Failed to create directory junction from {:?} to {:?}",
                link_path, target
            )
        })?;
    }

    Ok(())
}

fn update_gitignore(cwd: &Path) -> Result<()> {
    let gitignore_path = cwd.join(".gitignore");
    let mut content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut modified = false;
    if !content.contains(".context") {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str("\n# AutoMemory single-source local context\n.context\n");
        modified = true;
    }

    if !content.contains(".automemory") {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(".automemory\n");
        modified = true;
    }

    if modified {
        fs::write(gitignore_path, content)?;
    }

    Ok(())
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}
