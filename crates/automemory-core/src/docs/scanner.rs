use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::Database;
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
                "docs/architecture.md".to_string(),
                "docs/decisions/**/*.md".to_string(),
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
            if rel_path.contains(exc) || rel_path.starts_with('.') && !rel_path.starts_with(".cursor") {
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

        let existing = self.db.get_document_by_path(&project.id, &rel_path)?;

        let doc = match existing {
            Some(mut d) => {
                if d.content_hash != hash {
                    info!("Document updated: {} in project {}", rel_path, project.slug);
                    self.db.record_document_revision(&d.id, &content, &hash)?;
                    d.content_hash = hash;
                    d.last_scanned_at = Utc::now();
                    d.is_deleted = false;
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
                    content_hash: hash.clone(),
                    summary: None,
                    last_scanned_at: Utc::now(),
                    is_deleted: false,
                };
                self.db.track_document(&new_doc, Some(&content))?;
                new_doc
            }
        };

        Ok(doc)
    }
}
