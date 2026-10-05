use crate::db::Database;
use crate::graph::{GraphEdge, GraphEntity};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn batch_insert_graph_entities(&self, entities: &[GraphEntity]) -> Result<()> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"
            INSERT INTO graph_entities (
                id, project_id, entity_type, name, path, line_start, line_end, metadata, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                entity_type = excluded.entity_type,
                name = excluded.name,
                path = excluded.path,
                line_start = excluded.line_start,
                line_end = excluded.line_end,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at
            "#
        )?;

        for e in entities {
            stmt.execute(params![
                e.id,
                e.project_id,
                e.entity_type,
                e.name,
                e.path,
                e.line_start,
                e.line_end,
                serde_json::to_string(&e.metadata)?,
                e.created_at.to_rfc3339(),
                e.updated_at.to_rfc3339(),
            ])?;
        }
        Ok(())
    }

    pub fn batch_insert_graph_edges(&self, edges: &[GraphEdge]) -> Result<()> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"
            INSERT INTO graph_edges (source_id, target_id, relation_type, confidence, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(source_id, target_id, relation_type) DO UPDATE SET
                confidence = excluded.confidence
            "#,
        )?;

        for edge in edges {
            let _ = stmt.execute(params![
                edge.source_id,
                edge.target_id,
                edge.relation_type,
                edge.confidence,
                edge.created_at.to_rfc3339(),
            ]);
        }
        Ok(())
    }

    pub fn get_graph_entity_by_id(&self, id: &str) -> Result<Option<GraphEntity>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, entity_type, name, path, line_start, line_end, metadata, created_at, updated_at FROM graph_entities WHERE id = ?1"
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_graph_entity(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn query_graph_entities(
        &self,
        project_id: Option<&str>,
        entity_type: Option<&str>,
        name_pattern: Option<&str>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GraphEntity>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, entity_type, name, path, line_start, line_end, metadata, created_at, updated_at FROM graph_entities WHERE 1=1".to_string();

        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND project_id = '{}'", pid));
        }
        if let Some(et) = entity_type {
            sql.push_str(&format!(" AND entity_type = '{}'", et));
        }
        if let Some(np) = name_pattern {
            sql.push_str(&format!(" AND name LIKE '%{}%'", np));
        }
        if let Some(pp) = path_prefix {
            sql.push_str(&format!(" AND path LIKE '{}%'", pp));
        }

        sql.push_str(&format!(" ORDER BY path ASC LIMIT {}", limit));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_graph_entity)?;

        let mut entities = Vec::new();
        for r in rows {
            entities.push(r?);
        }
        Ok(entities)
    }

    pub fn get_outgoing_graph_edges(&self, source_id: &str) -> Result<Vec<GraphEdge>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, relation_type, confidence, created_at FROM graph_edges WHERE source_id = ?1"
        )?;
        let rows = stmt.query_map(params![source_id], Self::row_to_graph_edge)?;
        let mut edges = Vec::new();
        for r in rows {
            edges.push(r?);
        }
        Ok(edges)
    }

    pub fn get_incoming_graph_edges(&self, target_id: &str) -> Result<Vec<GraphEdge>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, relation_type, confidence, created_at FROM graph_edges WHERE target_id = ?1"
        )?;
        let rows = stmt.query_map(params![target_id], Self::row_to_graph_edge)?;
        let mut edges = Vec::new();
        for r in rows {
            edges.push(r?);
        }
        Ok(edges)
    }

    pub fn list_all_graph_entities(
        &self,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GraphEntity>> {
        self.query_graph_entities(project_id, None, None, None, limit)
    }

    pub fn list_all_graph_edges(&self, limit: usize) -> Result<Vec<GraphEdge>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            &format!("SELECT source_id, target_id, relation_type, confidence, created_at FROM graph_edges LIMIT {}", limit)
        )?;
        let rows = stmt.query_map([], Self::row_to_graph_edge)?;
        let mut edges = Vec::new();
        for r in rows {
            edges.push(r?);
        }
        Ok(edges)
    }

    fn row_to_graph_entity(row: &rusqlite::Row) -> rusqlite::Result<GraphEntity> {
        let meta_str: String = row.get(7)?;
        let created_at_str: String = row.get(8)?;
        let updated_at_str: String = row.get(9)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let metadata = serde_json::from_str(&meta_str).unwrap_or_default();

        Ok(GraphEntity {
            id: row.get(0)?,
            project_id: row.get(1)?,
            entity_type: row.get(2)?,
            name: row.get(3)?,
            path: row.get(4)?,
            line_start: row.get(5)?,
            line_end: row.get(6)?,
            metadata,
            created_at,
            updated_at,
        })
    }

    fn row_to_graph_edge(row: &rusqlite::Row) -> rusqlite::Result<GraphEdge> {
        let created_at_str: String = row.get(4)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(GraphEdge {
            source_id: row.get(0)?,
            target_id: row.get(1)?,
            relation_type: row.get(2)?,
            confidence: row.get(3)?,
            created_at,
        })
    }
}
