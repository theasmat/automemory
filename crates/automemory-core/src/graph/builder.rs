use super::model::{GraphEdge, GraphEntity, ImpactReport};
use crate::db::Database;
use anyhow::Result;
use chrono::Utc;
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

    /// Scans a workspace directory and indexes file structure and markdown documents into the graph.
    /// Deep AST symbol and call-graph analysis is delegated to companion engines (e.g. codebase-memory-mcp).
    pub fn scan_and_index_project(&self, project_id: &str, root_path: &str) -> Result<(usize, usize)> {
        let mut entities = Vec::new();
        let edges = Vec::new();

        let root = Path::new(root_path);
        if !root.exists() {
            return Ok((0, 0));
        }

        // Map relative path -> file entity id
        let mut file_entity_map: HashMap<String, String> = HashMap::new();
        self.walk_dir(root, root, project_id, &mut entities, &mut file_entity_map)?;

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

    /// Ingest external entities and edges (e.g. from companion codebase-memory tools) into AutoMemory's graph store.
    pub fn ingest_graph_data(&self, entities: &[GraphEntity], edges: &[GraphEdge]) -> Result<(usize, usize)> {
        self.db.batch_insert_graph_entities(entities)?;
        self.db.batch_insert_graph_edges(edges)?;
        Ok((entities.len(), edges.len()))
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
