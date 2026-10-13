use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntity {
    pub id: String,
    pub project_id: Option<String>,
    pub entity_type: String, // "file", "function", "struct", "module", "trait", "class", "doc"
    pub name: String,
    pub path: String,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String, // "imports", "calls", "defines", "implements", "references"
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub target_entity: GraphEntity,
    pub affected_entities: Vec<GraphEntity>,
    pub affected_files: Vec<String>,
    pub depth: usize,
}
