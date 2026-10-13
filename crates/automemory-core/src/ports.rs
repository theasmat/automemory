use crate::db::Database;
use anyhow::{Result, anyhow};

/// Versioned port contract (PRD §7.4). Every swappable module implements
/// [`Port`]; the host refuses `api_version` mismatches instead of
/// misbehaving (see [`check_api_version`]).
pub trait Port {
    fn port_name(&self) -> &'static str;
    fn api_version(&self) -> &'static str;
}

pub fn check_api_version(port_name: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(anyhow!(
            "port {port_name}: api_version mismatch (plugin {actual} != host {expected})"
        ))
    }
}

pub const CODE_INTEL_API_VERSION: &str = "1";

/// Code intelligence (PRD §7.1 CodeIntel): symbol existence, content hash,
/// callers. Powers the verifier's future `changed` status.
pub trait CodeIntel: Port {
    fn symbol_exists(&self, project_id: Option<&str>, symbol: &str) -> bool;
    fn symbol_hash(&self, project_id: Option<&str>, symbol: &str) -> Option<String>;
    fn callers(&self, project_id: Option<&str>, symbol: &str) -> Vec<String>;
}

/// Default implementation over the local graph index. An empty (never
/// indexed) graph answers negative — fail-open, never a false claim.
pub struct GraphCodeIntel<'a> {
    db: &'a Database,
}

impl<'a> GraphCodeIntel<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl Port for GraphCodeIntel<'_> {
    fn port_name(&self) -> &'static str {
        "CodeIntel"
    }
    fn api_version(&self) -> &'static str {
        CODE_INTEL_API_VERSION
    }
}

impl CodeIntel for GraphCodeIntel<'_> {
    fn symbol_exists(&self, project_id: Option<&str>, symbol: &str) -> bool {
        self.db
            .query_graph_entities(project_id, None, Some(symbol), None, 1)
            .is_ok_and(|hits| !hits.is_empty())
    }

    // ponytail: needs per-symbol content hashes recorded at index time.
    fn symbol_hash(&self, _project_id: Option<&str>, _symbol: &str) -> Option<String> {
        None
    }

    fn callers(&self, project_id: Option<&str>, symbol: &str) -> Vec<String> {
        let targets = self
            .db
            .query_graph_entities(project_id, None, Some(symbol), None, 5)
            .unwrap_or_default();
        let mut out = Vec::new();
        for t in targets {
            for edge in self.db.get_incoming_graph_edges(&t.id).unwrap_or_default() {
                // Current indexer records imports; call-level edges arrive
                // with a richer indexer. Importers are still dependents.
                if !matches!(edge.relation_type.as_str(), "calls" | "references" | "imports") {
                    continue;
                }
                let name = self
                    .db
                    .get_graph_entity_by_id(&edge.source_id)
                    .ok()
                    .flatten()
                    .map(|e| e.name)
                    .unwrap_or(edge.source_id);
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    }
}

/// Fallback implementation: no index configured. Answers negative everywhere.
pub struct NoneCodeIntel;

impl Port for NoneCodeIntel {
    fn port_name(&self) -> &'static str {
        "CodeIntel"
    }
    fn api_version(&self) -> &'static str {
        CODE_INTEL_API_VERSION
    }
}

impl CodeIntel for NoneCodeIntel {
    fn symbol_exists(&self, _project_id: Option<&str>, _symbol: &str) -> bool {
        false
    }
    fn symbol_hash(&self, _project_id: Option<&str>, _symbol: &str) -> Option<String> {
        None
    }
    fn callers(&self, _project_id: Option<&str>, _symbol: &str) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphEdge, GraphEntity};
    use chrono::Utc;

    #[test]
    fn api_version_gate_refuses_mismatch() {
        assert!(check_api_version("CodeIntel", "1", CODE_INTEL_API_VERSION).is_ok());
        assert!(check_api_version("CodeIntel", "2", CODE_INTEL_API_VERSION).is_err());
    }

    #[test]
    fn none_backend_answers_negative() {
        let ci = NoneCodeIntel;
        assert_eq!(ci.port_name(), "CodeIntel");
        assert!(!ci.symbol_exists(None, "anything"));
        assert_eq!(ci.symbol_hash(None, "anything"), None);
        assert!(ci.callers(None, "anything").is_empty());
    }

    #[test]
    fn graph_backend_resolves_symbols_and_importers() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();
        db.batch_insert_graph_entities(&[
            GraphEntity {
                id: "e-auth".to_string(),
                project_id: None,
                entity_type: "function".to_string(),
                name: "auth.session.createSession".to_string(),
                path: "src/auth/session.ts".to_string(),
                line_start: Some(1),
                line_end: Some(10),
                metadata: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            },
            GraphEntity {
                id: "e-login".to_string(),
                project_id: None,
                entity_type: "function".to_string(),
                name: "auth.login".to_string(),
                path: "src/auth/login.ts".to_string(),
                line_start: Some(1),
                line_end: Some(5),
                metadata: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            },
        ])
        .unwrap();
        db.batch_insert_graph_edges(&[GraphEdge {
            source_id: "e-login".to_string(),
            target_id: "e-auth".to_string(),
            relation_type: "imports".to_string(),
            confidence: 1.0,
            created_at: now,
        }])
        .unwrap();

        let ci = GraphCodeIntel::new(&db);
        assert!(check_api_version(ci.port_name(), ci.api_version(), CODE_INTEL_API_VERSION).is_ok());
        assert!(ci.symbol_exists(None, "createSession"));
        assert!(!ci.symbol_exists(None, "no.such.symbol"));
        assert_eq!(ci.callers(None, "createSession"), vec!["auth.login".to_string()]);
        // Hashes need indexer support (see ponytail note above).
        assert_eq!(ci.symbol_hash(None, "createSession"), None);
    }
}
