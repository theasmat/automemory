use super::model::{GraphEdge, GraphEntity, ImpactReport};
use crate::db::Database;
use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub struct GraphBuilder<'a> {
    db: &'a Database,
}

impl<'a> GraphBuilder<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Scans a workspace directory and builds structural entities and edges into the database.
    pub fn scan_and_index_project(&self, project_id: &str, root_path: &str) -> Result<(usize, usize)> {
        let mut entities = Vec::new();
        let mut edges = Vec::new();

        let root = Path::new(root_path);
        if !root.exists() {
            return Ok((0, 0));
        }

        // Map relative path -> file entity id
        let mut file_entity_map: HashMap<String, String> = HashMap::new();
        // Map symbol name -> Vec<entity id>
        let mut symbol_map: HashMap<String, Vec<String>> = HashMap::new();

        self.walk_dir(root, root, project_id, &mut entities, &mut file_entity_map)?;

        // Now parse file contents for symbols and links
        let fn_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\b")?;
        let struct_re = Regex::new(r"(?m)^\s*(?:pub\s+)?(?:struct|enum|trait)\s+([a-zA-Z0-9_]+)\b")?;
        let py_def_re = Regex::new(r"(?m)^\s*def\s+([a-zA-Z0-9_]+)\s*\(")?;
        let py_class_re = Regex::new(r"(?m)^\s*class\s+([a-zA-Z0-9_]+)\b")?;
        let ts_fn_re = Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)\b")?;
        let ts_class_re = Regex::new(r"(?m)^\s*(?:export\s+)?(?:class|interface|type)\s+([a-zA-Z0-9_]+)\b")?;
        let rs_use_re = Regex::new(r"(?m)^\s*use\s+([^;]+);")?;
        let ts_import_re = Regex::new(r#"(?m)^\s*import\s+.*?from\s+['"]([^'"]+)['"]"#)?;
        let py_import_re = Regex::new(r"(?m)^\s*(?:from\s+([a-zA-Z0-9_.]+)\s+import|import\s+([a-zA-Z0-9_.]+))")?;

        for (rel_path, file_id) in &file_entity_map {
            let full_path = root.join(rel_path);
            if let Ok(content) = fs::read_to_string(&full_path) {
                let lines: Vec<&str> = content.lines().collect();

                // 1. Rust symbols
                if rel_path.ends_with(".rs") {
                    for (line_idx, line) in lines.iter().enumerate() {
                        let line_num = (line_idx + 1) as i32;
                        if let Some(caps) = fn_re.captures(line) {
                            let name = caps[1].to_string();
                            let entity_id = Uuid::new_v4().to_string();
                            entities.push(GraphEntity {
                                id: entity_id.clone(),
                                project_id: Some(project_id.to_string()),
                                entity_type: "function".to_string(),
                                name: name.clone(),
                                path: rel_path.clone(),
                                line_start: Some(line_num),
                                line_end: Some(line_num),
                                metadata: serde_json::json!({ "language": "rust" }),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            });
                            edges.push(GraphEdge {
                                source_id: file_id.clone(),
                                target_id: entity_id.clone(),
                                relation_type: "defines".to_string(),
                                confidence: 1.0,
                                created_at: Utc::now(),
                            });
                            symbol_map.entry(name).or_default().push(entity_id);
                        } else if let Some(caps) = struct_re.captures(line) {
                            let name = caps[1].to_string();
                            let entity_id = Uuid::new_v4().to_string();
                            entities.push(GraphEntity {
                                id: entity_id.clone(),
                                project_id: Some(project_id.to_string()),
                                entity_type: "struct".to_string(),
                                name: name.clone(),
                                path: rel_path.clone(),
                                line_start: Some(line_num),
                                line_end: Some(line_num),
                                metadata: serde_json::json!({ "language": "rust" }),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            });
                            edges.push(GraphEdge {
                                source_id: file_id.clone(),
                                target_id: entity_id.clone(),
                                relation_type: "defines".to_string(),
                                confidence: 1.0,
                                created_at: Utc::now(),
                            });
                            symbol_map.entry(name).or_default().push(entity_id);
                        } else if let Some(caps) = rs_use_re.captures(line) {
                            let imp = caps[1].trim().to_string();
                            // Check if this imports another file in workspace
                            for other_path in file_entity_map.keys() {
                                let stem = Path::new(other_path).file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                if !stem.is_empty() && (imp.contains(stem) || imp.contains(&stem.replace('-', "_"))) {
                                    if let Some(target_fid) = file_entity_map.get(other_path) {
                                        if target_fid != file_id {
                                            edges.push(GraphEdge {
                                                source_id: file_id.clone(),
                                                target_id: target_fid.clone(),
                                                relation_type: "imports".to_string(),
                                                confidence: 0.9,
                                                created_at: Utc::now(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // 2. Python symbols
                else if rel_path.ends_with(".py") {
                    for (line_idx, line) in lines.iter().enumerate() {
                        let line_num = (line_idx + 1) as i32;
                        if let Some(caps) = py_def_re.captures(line) {
                            let name = caps[1].to_string();
                            let entity_id = Uuid::new_v4().to_string();
                            entities.push(GraphEntity {
                                id: entity_id.clone(),
                                project_id: Some(project_id.to_string()),
                                entity_type: "function".to_string(),
                                name: name.clone(),
                                path: rel_path.clone(),
                                line_start: Some(line_num),
                                line_end: Some(line_num),
                                metadata: serde_json::json!({ "language": "python" }),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            });
                            edges.push(GraphEdge {
                                source_id: file_id.clone(),
                                target_id: entity_id.clone(),
                                relation_type: "defines".to_string(),
                                confidence: 1.0,
                                created_at: Utc::now(),
                            });
                            symbol_map.entry(name).or_default().push(entity_id);
                        } else if let Some(caps) = py_class_re.captures(line) {
                            let name = caps[1].to_string();
                            let entity_id = Uuid::new_v4().to_string();
                            entities.push(GraphEntity {
                                id: entity_id.clone(),
                                project_id: Some(project_id.to_string()),
                                entity_type: "class".to_string(),
                                name: name.clone(),
                                path: rel_path.clone(),
                                line_start: Some(line_num),
                                line_end: Some(line_num),
                                metadata: serde_json::json!({ "language": "python" }),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            });
                            edges.push(GraphEdge {
                                source_id: file_id.clone(),
                                target_id: entity_id.clone(),
                                relation_type: "defines".to_string(),
                                confidence: 1.0,
                                created_at: Utc::now(),
                            });
                            symbol_map.entry(name).or_default().push(entity_id);
                        } else if let Some(caps) = py_import_re.captures(line) {
                            let imp = caps.get(1).or_else(|| caps.get(2)).map(|m| m.as_str()).unwrap_or("");
                            for other_path in file_entity_map.keys() {
                                let stem = Path::new(other_path).file_stem().and_then(|s| s.to_str()).unwrap_or("");
                                if !stem.is_empty() && (imp.contains(stem) || imp.contains(&stem.replace('-', "_"))) {
                                    if let Some(target_fid) = file_entity_map.get(other_path) {
                                        if target_fid != file_id {
                                            edges.push(GraphEdge {
                                                source_id: file_id.clone(),
                                                target_id: target_fid.clone(),
                                                relation_type: "imports".to_string(),
                                                confidence: 0.9,
                                                created_at: Utc::now(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // 3. TS / JS symbols
                else if rel_path.ends_with(".ts") || rel_path.ends_with(".js") || rel_path.ends_with(".tsx") || rel_path.ends_with(".jsx") {
                    for (line_idx, line) in lines.iter().enumerate() {
                        let line_num = (line_idx + 1) as i32;
                        if let Some(caps) = ts_fn_re.captures(line) {
                            let name = caps[1].to_string();
                            let entity_id = Uuid::new_v4().to_string();
                            entities.push(GraphEntity {
                                id: entity_id.clone(),
                                project_id: Some(project_id.to_string()),
                                entity_type: "function".to_string(),
                                name: name.clone(),
                                path: rel_path.clone(),
                                line_start: Some(line_num),
                                line_end: Some(line_num),
                                metadata: serde_json::json!({ "language": "javascript" }),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            });
                            edges.push(GraphEdge {
                                source_id: file_id.clone(),
                                target_id: entity_id.clone(),
                                relation_type: "defines".to_string(),
                                confidence: 1.0,
                                created_at: Utc::now(),
                            });
                            symbol_map.entry(name).or_default().push(entity_id);
                        } else if let Some(caps) = ts_class_re.captures(line) {
                            let name = caps[1].to_string();
                            let entity_id = Uuid::new_v4().to_string();
                            entities.push(GraphEntity {
                                id: entity_id.clone(),
                                project_id: Some(project_id.to_string()),
                                entity_type: "class".to_string(),
                                name: name.clone(),
                                path: rel_path.clone(),
                                line_start: Some(line_num),
                                line_end: Some(line_num),
                                metadata: serde_json::json!({ "language": "javascript" }),
                                created_at: Utc::now(),
                                updated_at: Utc::now(),
                            });
                            edges.push(GraphEdge {
                                source_id: file_id.clone(),
                                target_id: entity_id.clone(),
                                relation_type: "defines".to_string(),
                                confidence: 1.0,
                                created_at: Utc::now(),
                            });
                            symbol_map.entry(name).or_default().push(entity_id);
                        } else if let Some(caps) = ts_import_re.captures(line) {
                            let imp = caps[1].to_string();
                            for (other_path, target_fid) in &file_entity_map {
                                if other_path.contains(&imp) || imp.contains(other_path) {
                                    if target_fid != file_id {
                                        edges.push(GraphEdge {
                                            source_id: file_id.clone(),
                                            target_id: target_fid.clone(),
                                            relation_type: "imports".to_string(),
                                            confidence: 0.95,
                                            created_at: Utc::now(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Store batch in database
        self.db.batch_insert_graph_entities(&entities)?;
        self.db.batch_insert_graph_edges(&edges)?;

        Ok((entities.len(), edges.len()))
    }

    fn walk_dir(
        &self,
        root: &Path,
        current: &Path,
        project_id: &str,
        entities: &mut Vec<GraphEntity>,
        file_map: &mut HashMap<String, String>,
    ) -> Result<()> {
        let entries = match fs::read_dir(current) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden, target, and ignored folders
            if file_name.starts_with('.')
                || file_name == "target"
                || file_name == "node_modules"
                || file_name == "dist"
                || file_name == "build"
                || file_name == "vendor"
            {
                continue;
            }

            if path.is_dir() {
                self.walk_dir(root, &path, project_id, entities, file_map)?;
            } else if path.is_file() {
                if let Ok(rel_path) = path.strip_prefix(root) {
                    let rel_str = rel_path.to_string_lossy().to_string();
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let is_code_or_doc = matches!(
                        ext,
                        "rs" | "ts" | "js" | "tsx" | "jsx" | "py" | "go" | "md" | "json" | "toml" | "yaml" | "yml" | "sql"
                    );

                    if is_code_or_doc {
                        let entity_id = Uuid::new_v4().to_string();
                        let entity_type = if ext == "md" { "doc" } else { "file" };

                        entities.push(GraphEntity {
                            id: entity_id.clone(),
                            project_id: Some(project_id.to_string()),
                            entity_type: entity_type.to_string(),
                            name: file_name,
                            path: rel_str.clone(),
                            line_start: None,
                            line_end: None,
                            metadata: serde_json::json!({ "extension": ext }),
                            created_at: Utc::now(),
                            updated_at: Utc::now(),
                        });
                        file_map.insert(rel_str, entity_id);
                    }
                }
            }
        }
        Ok(())
    }

    /// Calculate blast-radius impact of modifying an entity or file path.
    pub fn trace_impact(&self, project_id: Option<&str>, target_path_or_symbol: &str, max_depth: usize) -> Result<Option<ImpactReport>> {
        // 1. Find target entity
        let entities = self.db.query_graph_entities(project_id, None, Some(target_path_or_symbol), None, 1)?;
        let target = match entities.into_iter().next() {
            Some(e) => e,
            None => {
                // Try searching by exact path
                let by_path = self.db.query_graph_entities(project_id, None, None, Some(target_path_or_symbol), 1)?;
                match by_path.into_iter().next() {
                    Some(e) => e,
                    None => return Ok(None),
                }
            }
        };

        // BFS traversal for incoming edges (things that depend on target)
        let mut queue = VecDeque::new();
        queue.push_back((target.id.clone(), 0));

        let mut visited = HashSet::new();
        visited.insert(target.id.clone());

        let mut affected_entities = Vec::new();
        let mut affected_files = HashSet::new();

        while let Some((curr_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            // Find all edges where target_id == curr_id (incoming dependencies)
            let incoming = self.db.get_incoming_graph_edges(&curr_id)?;
            for edge in incoming {
                if !visited.contains(&edge.source_id) {
                    visited.insert(edge.source_id.clone());
                    if let Ok(Some(ent)) = self.db.get_graph_entity_by_id(&edge.source_id) {
                        affected_files.insert(ent.path.clone());
                        affected_entities.push(ent);
                        queue.push_back((edge.source_id, depth + 1));
                    }
                }
            }
        }

        let mut sorted_files: Vec<String> = affected_files.into_iter().collect();
        sorted_files.sort();

        Ok(Some(ImpactReport {
            target_entity: target,
            affected_entities,
            affected_files: sorted_files,
            depth: max_depth,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_entity_and_impact() {
        let db = Database::open_in_memory().expect("in-memory db open failed");
        let builder = GraphBuilder::new(&db);

        let e1 = GraphEntity {
            id: "file-main".to_string(),
            project_id: Some("proj-1".to_string()),
            entity_type: "file".to_string(),
            name: "main.rs".to_string(),
            path: "src/main.rs".to_string(),
            line_start: None,
            line_end: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let e2 = GraphEntity {
            id: "file-auth".to_string(),
            project_id: Some("proj-1".to_string()),
            entity_type: "file".to_string(),
            name: "auth.rs".to_string(),
            path: "src/auth.rs".to_string(),
            line_start: None,
            line_end: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let edge = GraphEdge {
            source_id: "file-main".to_string(),
            target_id: "file-auth".to_string(),
            relation_type: "imports".to_string(),
            confidence: 1.0,
            created_at: Utc::now(),
        };

        db.batch_insert_graph_entities(&[e1, e2]).expect("insert entities failed");
        db.batch_insert_graph_edges(&[edge]).expect("insert edge failed");

        let impact = builder.trace_impact(Some("proj-1"), "auth.rs", 2).expect("trace impact failed");
        assert!(impact.is_some());
        let rep = impact.unwrap();
        assert_eq!(rep.target_entity.id, "file-auth");
        assert_eq!(rep.affected_files, vec!["src/main.rs".to_string()]);
    }
}

