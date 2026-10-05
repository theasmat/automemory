use crate::db::Database;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub struct DocCompiler<'a> {
    db: &'a Database,
}

impl<'a> DocCompiler<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Extract L0 (one-sentence abstract) from markdown content
    pub fn extract_l0_abstract(content: &str, title: &str) -> String {
        for line in content.lines() {
            let trimmed = line.trim();
            // Skip title heading or empty lines
            if trimmed.is_empty() || trimmed.starts_with("# ") {
                continue;
            }
            // First substantial prose line or block
            if !trimmed.starts_with('#') && !trimmed.starts_with("```") && trimmed.len() > 10 {
                let first_sentence = trimmed.split('.').next().unwrap_or(trimmed).trim();
                let clean = first_sentence.replace(['*', '_', '`'], "");
                if clean.len() > 120 {
                    return format!("{}: {}...", title, &clean[..117]);
                } else {
                    return format!("{}: {}.", title, clean.trim_end_matches('.'));
                }
            }
        }
        format!("{}: Documentation and specification reference.", title)
    }

    /// Extract L1 (synthesized architectural overview) from markdown content
    pub fn extract_l1_overview(content: &str, title: &str, category: &str) -> String {
        let mut overview = format!("#### [{}] {}\n", category.to_uppercase(), title);
        let mut key_points = Vec::new();
        let mut in_code_block = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block || trimmed.is_empty() {
                continue;
            }

            // Headings as sections
            if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
                let h_title = trimmed.trim_start_matches('#').trim();
                key_points.push(format!("- **Section**: {}", h_title));
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                // Bullet points with bold words
                if trimmed.contains("**") || trimmed.contains('`') {
                    if key_points.len() < 8 {
                        key_points.push(trimmed.to_string());
                    }
                }
            }
        }

        if key_points.is_empty() {
            overview.push_str("- Core specifications and implementation guidelines.\n");
        } else {
            overview.push_str(&key_points.join("\n"));
            overview.push('\n');
        }

        overview
    }

    /// Compile all tracked documents for a project and update canonical context files
    pub fn compile_project_docs(&self, project_id: &str, context_root: &Path) -> Result<usize> {
        let docs = self.db.list_documents(project_id)?;
        if docs.is_empty() {
            return Ok(0);
        }

        let docs_dir = context_root.join("docs");
        fs::create_dir_all(&docs_dir)?;

        let mut project_l1_sections = Vec::new();
        let mut category_map: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();

        for doc in &docs {
            let file_path = context_root.join(&doc.relative_path);
            let content = if file_path.exists() {
                fs::read_to_string(&file_path).unwrap_or_default()
            } else {
                let cat_path = context_root.join("docs").join(&doc.category).join(
                    Path::new(&doc.relative_path)
                        .file_name()
                        .unwrap_or_default(),
                );
                if cat_path.exists() {
                    fs::read_to_string(&cat_path).unwrap_or_default()
                } else {
                    continue;
                }
            };

            let title = if !doc.title.is_empty() {
                doc.title.clone()
            } else {
                Path::new(&doc.relative_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Document".to_string())
            };

            let l0 = Self::extract_l0_abstract(&content, &title);
            let l1 = Self::extract_l1_overview(&content, &title, &doc.category);

            // Update in DB
            let _ = self.db.update_document_l0_l1(&doc.id, Some(&l0), Some(&l1));

            category_map
                .entry(doc.category.clone())
                .or_default()
                .push((l0.clone(), l1.clone()));

            project_l1_sections.push(l1);
        }

        // Generate category-level .overview.md and .abstract.md
        for (category, items) in &category_map {
            let cat_dir = docs_dir.join(category);
            fs::create_dir_all(&cat_dir)?;

            let cat_abstract = format!(
                "# Category: {}\n{}\n",
                category,
                items
                    .iter()
                    .map(|(l0, _)| format!("- {}", l0))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let cat_overview = format!(
                "# {} Overview\n\n{}\n",
                category.to_uppercase(),
                items
                    .iter()
                    .map(|(_, l1)| l1.as_str())
                    .collect::<Vec<_>>()
                    .join("\n---\n\n")
            );

            let _ = fs::write(cat_dir.join(".abstract.md"), cat_abstract);
            let _ = fs::write(cat_dir.join(".overview.md"), cat_overview);
        }

        // Update root .context/.overview.md with product docs section
        let root_overview_path = context_root.join(".overview.md");
        let mut root_overview = if root_overview_path.exists() {
            fs::read_to_string(&root_overview_path).unwrap_or_default()
        } else {
            format!("# {} Context Overview\n\n", project_id)
        };

        // Append or replace Product Specifications section
        let doc_section_header = "\n### Active Product Specifications & Guidelines\n";
        if let Some(pos) = root_overview.find(doc_section_header) {
            root_overview.truncate(pos);
        }

        root_overview.push_str(doc_section_header);
        root_overview.push_str(&project_l1_sections.join("\n"));
        root_overview.push('\n');

        fs::write(&root_overview_path, root_overview).with_context(|| {
            format!("Failed to write root overview at {:?}", root_overview_path)
        })?;

        Ok(docs.len())
    }
}
