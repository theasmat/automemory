use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::extraction::DocCompiler;
use automemory_core::models::{Project, TrackedDocument};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn track(
    file_path_str: String,
    category_opt: Option<String>,
    title_opt: Option<String>,
    project_opt: Option<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let target_path = cwd.join(&file_path_str);
    if !target_path.exists() {
        anyhow::bail!("File not found at {:?}", target_path);
    }

    let content = fs::read_to_string(&target_path)
        .with_context(|| format!("Failed to read file at {:?}", target_path))?;

    // Calculate SHA-256
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Relative path normalized
    let rel_path = target_path
        .strip_prefix(&cwd)
        .unwrap_or(&target_path)
        .to_string_lossy()
        .to_string();

    let project_id = project_opt.unwrap_or_else(|| infer_project_id(&cwd));

    // Determine category
    let category = category_opt.unwrap_or_else(|| {
        let lower = rel_path.to_lowercase();
        if lower.contains("prd") || lower.contains("requirement") {
            "prd".to_string()
        } else if lower.contains("adr") || lower.contains("decision") {
            "adr".to_string()
        } else if lower.contains("rule") || lower.contains("invariant") || lower.contains("policy") {
            "business-rules".to_string()
        } else if lower.contains("api") || lower.contains("spec") || lower.contains("schema") {
            "api".to_string()
        } else if lower.contains("plan") || lower.contains("roadmap") || lower.contains("todo") {
            "plan".to_string()
        } else {
            "general".to_string()
        }
    });

    // Determine title
    let title = title_opt.unwrap_or_else(|| {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("# ") {
                return trimmed[2..].trim().to_string();
            }
        }
        target_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Document".to_string())
    });

    let l0 = DocCompiler::extract_l0_abstract(&content, &title);
    let l1 = DocCompiler::extract_l1_overview(&content, &title, &category);

    let doc = TrackedDocument {
        id: format!("doc_{}", Uuid::new_v4()),
        project_id: project_id.clone(),
        relative_path: rel_path.clone(),
        category: category.clone(),
        title: title.clone(),
        content_hash: hash.clone(),
        remote_synced_hash: None,
        summary: Some(l0.clone()),
        l0_abstract: Some(l0.clone()),
        l1_overview: Some(l1),
        last_scanned_at: Utc::now(),
        is_deleted: false,
    };

    db.track_document(&doc, Some(&content))?;

    // Copy to canonical context directory: ~/.automemory/context/projects/{project_id}/docs/{category}/
    let context_root = config.home_dir.join("context").join("projects").join(&project_id);
    let target_context_dir = context_root.join("docs").join(&category);
    fs::create_dir_all(&target_context_dir)?;
    let target_filename = target_path.file_name().unwrap_or_default();
    let _ = fs::write(target_context_dir.join(target_filename), &content);

    // Compile project docs
    let compiler = DocCompiler::new(&db);
    let _ = compiler.compile_project_docs(&project_id, &context_root);

    println!("✓ Successfully tracked document for project '{}':", project_id);
    println!("  📄 File:        {}", rel_path);
    println!("  🏷️  Category:    [{}] {}", category.to_uppercase(), title);
    println!("  🔒 SHA-256:     {} ({} bytes)", &hash[..12], content.len());
    println!("  💡 L0 Abstract: {}", l0);
    println!("  📁 Context:     .context/docs/{}/{}", category, target_filename.to_string_lossy());

    Ok(())
}

pub fn list(project_opt: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_opt.unwrap_or_else(|| infer_project_id(&cwd));
    let docs = db.list_documents(&project_id)?;

    println!("=== Tracked Product Documents & Specs: '{}' ===\n", project_id);

    if docs.is_empty() {
        println!("  No documents tracked yet. Track a document using: 'automemory doc track <path>'");
        return Ok(());
    }

    for d in docs {
        let sync_status = match &d.remote_synced_hash {
            Some(remote) if remote == &d.content_hash => "✓ Synced",
            Some(_) => "⚡ Modified (Local Delta)",
            None => "☁ Pending Upload",
        };

        println!("• [{}] {}", d.category.to_uppercase(), d.title);
        println!("  Path:   {}", d.relative_path);
        println!("  Hash:   {} | Status: {}", &d.content_hash[..12], sync_status);
        if let Some(l0) = d.l0_abstract {
            println!("  L0:     {}", l0);
        }
        println!();
    }

    Ok(())
}

pub fn compile(project_opt: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_opt.unwrap_or_else(|| infer_project_id(&cwd));
    let context_root = config.home_dir.join("context").join("projects").join(&project_id);

    println!("Compiling L0/L1 summaries for project '{}'...", project_id);
    let compiler = DocCompiler::new(&db);
    let count = compiler.compile_project_docs(&project_id, &context_root)?;

    println!("✓ Successfully compiled {} product documents!", count);
    println!("  📁 Category abstracts: .context/docs/*/.abstract.md");
    println!("  📄 Root overview:      .context/.overview.md");

    Ok(())
}

pub fn scan(path_opt: Option<String>, project_opt: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let target_dir = path_opt.map(PathBuf::from).unwrap_or(cwd);
    let config = Config::default_config();
    let db = std::sync::Arc::new(Database::open(&config.db_path)?);

    let global_cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
    let registered_proj = global_cfg.find_project_by_path(&target_dir);

    let project_id = project_opt.unwrap_or_else(|| {
        registered_proj
            .map(|p| p.name.clone())
            .unwrap_or_else(|| infer_project_id(&target_dir))
    });

    let trackable_patterns = registered_proj
        .map(|p| p.trackable_files.clone())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(automemory_core::config::default_trackable_files);

    let mut project = match db.get_project_by_id(&project_id)? {
        Some(p) => p,
        None => Project {
            id: project_id.clone(),
            slug: project_id.clone(),
            display_name: project_id.clone(),
            document_config: Default::default(),
            sync_config: Default::default(),
            capture_policy: Default::default(),
            context_budget: Default::default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    };

    project.document_config.include_patterns = trackable_patterns;

    println!("Scanning project directory for specifications & architecture docs...");
    println!("  📁 Target:  {}", target_dir.display());
    println!("  🏷️  Project: {}", project_id);

    let scanner = automemory_core::docs::DocumentScanner::new(db.clone());
    let docs = scanner.scan_project(&project, &target_dir)?;

    println!("\n✓ Auto-discovery complete: tracked & compiled {} documents!", docs.len());
    for d in &docs {
        println!("  • [{}] {} -> {}", d.category.to_uppercase(), d.title, d.relative_path);
    }
    println!("  📄 Updated: .context/.overview.md");

    Ok(())
}

fn infer_project_id(cwd: &Path) -> String {
    // 1. Check global config (~/.automemory/config.toml)
    if let Ok(global_cfg) = automemory_core::config::GlobalConfig::load() {
        if let Some(p) = global_cfg.find_project_by_path(cwd) {
            return p.name.clone();
        }
    }

    // 2. Fallback to legacy .automemory/config.json if present
    let cfg_path = cwd.join(".automemory").join("config.json");
    if let Ok(content) = fs::read_to_string(&cfg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(id) = json.get("project_id").and_then(|v| v.as_str()) {
                return id.to_string();
            }
        }
    }

    // 3. Fallback to folder name
    cwd.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "default-project".to_string())
}
