use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::db::Database;
use crate::extraction::DocCompiler;
use crate::models::{Project, TrackedDocument};
use crate::redaction::contains_secrets;

pub struct DocumentScanner {
    db: std::sync::Arc<Database>,
}

impl DocumentScanner {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self { db }
    }

    /// Scan all tracked documents for a given project.
    pub fn scan_project(&self, project: &Project, root_path: &Path) -> Result<Vec<TrackedDocument>> {
        if !root_path.exists() || !root_path.is_dir() {
            return Ok(Vec::new());
        }

        let include_patterns = if project.document_config.include_patterns.is_empty() {
            vec![
                "PRD.md".to_string(),
                "docs/**/*.md".to_string(),
                "specs/**/*.md".to_string(),
                "prd/**/*.md".to_string(),
                "adr/**/*.md".to_string(),
                "rfc/**/*.md".to_string(),
                "plans/**/*.md".to_string(),
                "plan.md".to_string(),
                "PLAN.md".to_string(),
                "ROADMAP.md".to_string(),
                "ARCHITECTURE.md".to_string(),
                ".cursor/rules/**/*.md".to_string(),
                "AGENTS.md".to_string(),
                "CLAUDE.md".to_string(),
            ]
        } else {
            project.document_config.include_patterns.clone()
        };

        let mut scanned_docs = Vec::new();

        for pattern in &include_patterns {
            let full_glob = root_path.join(pattern).to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&full_glob) {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        if let Ok(doc) = self.process_file(project, root_path, &entry) {
                            scanned_docs.push(doc);
                        }
                    }
                }
            }
        }

        Ok(scanned_docs)
    }

    /// Process a single file: read, hash, check secrets, and record revision if changed.
    pub fn process_file(&self, project: &Project, root_path: &Path, file_path: &Path) -> Result<TrackedDocument> {
        let rel_path = file_path
            .strip_prefix(root_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Exclude check
        for exc in &project.document_config.exclude_patterns {
            if rel_path.contains(exc) || (rel_path.starts_with('.') && !rel_path.starts_with(".cursor")) {
                // skip excluded files
            }
        }

        let content = fs::read_to_string(file_path)?;

        // Secret detection
        if contains_secrets(&content) {
            warn!(
                "Document {} in project {} contains unredacted secrets. Blocking sync to vault.",
                rel_path, project.slug
            );
        }

        // Hash content
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let title = content
            .lines()
            .find(|l| l.starts_with("# "))
            .map(|l| l.trim_start_matches("# ").trim().to_string())
            .unwrap_or_else(|| {
                Path::new(&rel_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Document".to_string())
            });

        let lower_path = rel_path.to_lowercase();
        let category = if lower_path.contains("/prd/") || lower_path.contains("prd") {
            "prd"
        } else if lower_path.contains("/adr/") || lower_path.contains("adr") || lower_path.contains("architect") {
            "adr"
        } else if lower_path.contains("/rules/") || lower_path.contains("business") || lower_path.contains("invariant") {
            "business-rules"
        } else if lower_path.contains("/api/") || lower_path.contains("spec") || lower_path.contains("rfc") {
            "api"
        } else if lower_path.contains("plan") || lower_path.contains("roadmap") || lower_path.contains("todo") {
            "plan"
        } else {
            "general"
        }
        .to_string();

        let l0 = DocCompiler::extract_l0_abstract(&content, &title);
        let l1 = DocCompiler::extract_l1_overview(&content, &title, &category);

        let existing = self.db.get_document_by_path(&project.id, &rel_path)?;

        let doc = match existing {
            Some(mut d) => {
                if d.content_hash != hash {
                    info!("Document updated: {} in project {}", rel_path, project.slug);
                    self.db.record_document_revision(&d.id, &content, &hash)?;
                    d.content_hash = hash;
                    d.title = title;
                    d.category = category.clone();
                    d.l0_abstract = Some(l0.clone());
                    d.l1_overview = Some(l1.clone());
                    d.last_scanned_at = Utc::now();
                    d.is_deleted = false;
                    self.db.track_document(&d, Some(&content))?;
                }
                d
            }
            None => {
                info!("Tracking new document: {} in project {}", rel_path, project.slug);
                let new_id = Uuid::new_v4().to_string();
                let new_doc = TrackedDocument {
                    id: new_id,
                    project_id: project.id.clone(),
                    relative_path: rel_path.clone(),
                    category: category.clone(),
                    title,
                    content_hash: hash.clone(),
                    remote_synced_hash: None,
                    summary: Some(l0.clone()),
                    l0_abstract: Some(l0.clone()),
                    l1_overview: Some(l1.clone()),
                    last_scanned_at: Utc::now(),
                    is_deleted: false,
                };
                self.db.track_document(&new_doc, Some(&content))?;
                new_doc
            }
        };

        // Materialize in canonical context if project context exists
        let config = Config::default_config();
        let base_projects = config.home_dir.join("context").join("projects");
        let target_root = if base_projects.join(&project.slug).exists() {
            Some(base_projects.join(&project.slug))
        } else if base_projects.join(&project.id).exists() {
            Some(base_projects.join(&project.id))
        } else {
            None
        };

        if let Some(context_root) = target_root {
            let target_dir = context_root.join("docs").join(&category);
            let _ = fs::create_dir_all(&target_dir);
            let fname = Path::new(&rel_path).file_name().unwrap_or_default();
            let _ = fs::write(target_dir.join(fname), &content);
            let compiler = DocCompiler::new(&self.db);
            let _ = compiler.compile_project_docs(&project.id, &context_root);
            info!("✓ Auto-compiled L0/L1 context overview at {:?}", context_root.join(".overview.md"));
        }

        Ok(doc)
    }
}
