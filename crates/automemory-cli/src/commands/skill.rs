use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::extraction::SkillDistiller;
use std::path::{Path, PathBuf};

pub fn list(project_slug: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let slug = project_slug.unwrap_or_else(detect_current_project_slug);
    let project = db.get_project_by_slug(&slug)?;
    let project_id = project.as_ref().map(|p| p.id.as_str());

    // Also run on-demand distiller to pick up any freshly synthesized skills
    let distiller = SkillDistiller::new(&db);
    let _ = distiller.distill_skills_with_threshold(project_id, 1, 0.0);

    let skills = db.list_skills(project_id, 100)?;

    println!("┌────────────────────────────────────────────────────────────────────────┐");
    println!("│             AutoMemory Synthesized Procedural Playbooks                │");
    println!("├────────────────────────────────────────────────────────────────────────┤");
    println!("│  Project: {:<60} │", slug);
    println!("│  Total Skills: {:<55} │", skills.len());
    println!("└────────────────────────────────────────────────────────────────────────┘");

    if skills.is_empty() {
        println!(
            "No procedural skills synthesized yet for project '{}'.",
            slug
        );
        println!("Skills are automatically synthesized when successful execution traces repeat.");
        return Ok(());
    }

    for (idx, skill) in skills.iter().enumerate() {
        println!(
            "{}. [{}] {} (Runs: {}, Failures: {})",
            idx + 1,
            skill.skill_slug(),
            skill.name,
            skill.success_count,
            skill.failure_count
        );
        println!("   Description: {}", skill.description);
        if !skill.steps.is_empty() {
            println!("   Steps: {} procedure step(s)", skill.steps.len());
        }
        println!();
    }

    Ok(())
}

pub fn export(
    project_slug: Option<String>,
    target_dir_arg: Option<String>,
    min_runs: usize,
) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let slug = project_slug.unwrap_or_else(detect_current_project_slug);
    let project = db.get_project_by_slug(&slug)?;
    let project_id = project.as_ref().map(|p| p.id.as_str());

    // Distill with minimum runs threshold
    let distiller = SkillDistiller::new(&db);
    let distilled = distiller.distill_skills_with_threshold(project_id, min_runs, 0.0)?;
    for s in &distilled {
        let _ = db.save_skill(s);
    }

    let skills = db.list_skills(project_id, 100)?;
    let target_base: PathBuf = if let Some(dir) = target_dir_arg {
        PathBuf::from(dir)
    } else {
        Path::new(".agents").join("skills")
    };

    println!(
        "Exporting {} procedural skill(s) to {:?}",
        skills.len(),
        target_base
    );

    let mut exported_count = 0;
    for skill in &skills {
        if (skill.success_count as usize) < min_runs {
            continue;
        }
        let file_path = skill.export_to_skill_dir(&target_base)?;
        println!(
            "  ✓ Exported skill '{}' -> {:?}",
            skill.skill_slug(),
            file_path
        );
        exported_count += 1;
    }

    println!(
        "Successfully exported {} verified skill(s).",
        exported_count
    );
    Ok(())
}

fn detect_current_project_slug() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "default".to_string())
        .to_lowercase()
        .replace(' ', "-")
}
