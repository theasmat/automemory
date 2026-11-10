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
    pub fn scan_project(
        &self,
        project: &Project,
        root_path: &Path,
    ) -> Result<Vec<TrackedDocument>> {
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
    pub fn process_file(
        &self,
        project: &Project,
        root_path: &Path,
        file_path: &Path,
    ) -> Result<TrackedDocument> {
        let rel_path = file_path
            .strip_prefix(root_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // A symlink could point at a private file outside the project (e.g. AGENTS.md -> ~/.ssh/id_ed25519).
        if fs::symlink_metadata(file_path)?.file_type().is_symlink() {
            anyhow::bail!("{rel_path} is a symlink; not tracked");
        }
        if project.document_config.exclude_patterns.iter().any(|p| {
            glob::Pattern::new(p).is_ok_and(|g| g.matches(&rel_path))
        }) {
            anyhow::bail!("{rel_path} is excluded");
        }

        let content = fs::read_to_string(file_path)?;

        if contains_secrets(&content) {
            warn!(
                "Document {} in project {} contains secrets; not tracked.",
                rel_path, project.slug
            );
            anyhow::bail!("{rel_path} contains secrets; not tracked");
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
        let has = |words: &[&str]| lower_path.split('/').any(|seg| words.iter().any(|w| seg.starts_with(w)));
        let category = if has(&["prd"]) {
            "prd"
        } else if has(&["adr", "architect"]) {
            "adr"
        } else if has(&["rules", "business", "invariant"]) {
            "business-rules"
        } else if has(&["api", "spec", "rfc"]) {
            "api"
        } else if has(&["plan", "roadmap", "todo"]) {
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
                info!(
                    "Tracking new document: {} in project {}",
                    rel_path, project.slug
                );
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
            let _ = fs::write(target_dir.join(rel_path.replace(['/', '\\'], "__")), &content);
            let compiler = DocCompiler::new(&self.db);
            let _ = compiler.compile_project_docs(&project.id, &context_root);
            info!(
                "✓ Auto-compiled L0/L1 context overview at {:?}",
                context_root.join(".overview.md")
            );
        }

        Ok(doc)
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use crate::models::Project;
    use std::sync::Arc;

    fn setup() -> (DocumentScanner, Project, std::path::PathBuf) {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let dir = std::env::temp_dir().join(format!("am-scan-{}", Uuid::new_v4()));
        fs::create_dir_all(dir.join("docs")).unwrap();
        let project = Project {
            id: Uuid::new_v4().to_string(),
            slug: "p".into(),
            display_name: "p".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            capture_policy: Default::default(),
            context_budget: Default::default(),
            document_config: Default::default(),
            sync_config: Default::default(),
        };
        db.save_project(&project).unwrap();
        (DocumentScanner::new(db), project, dir)
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_document_is_not_tracked() {
        let (scanner, project, dir) = setup();
        let secret = dir.join("private.txt");
        fs::write(&secret, "# private\nnot for memory").unwrap();
        std::os::unix::fs::symlink(&secret, dir.join("AGENTS.md")).unwrap();
        assert!(scanner.process_file(&project, &dir, &dir.join("AGENTS.md")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_named_docs_do_not_collide_and_categories_use_whole_words() {
        let (scanner, project, dir) = setup();
        fs::write(dir.join("docs/address-book.md"), "# Address book").unwrap();
        let d = scanner.process_file(&project, &dir, &dir.join("docs/address-book.md")).unwrap();
        assert_eq!(d.category, "general");
        let _ = fs::remove_dir_all(&dir);
    }
}
