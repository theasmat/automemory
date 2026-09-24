use crate::db::Database;
use crate::models::{Project, ProjectPath};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn save_project(&self, project: &Project) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO projects (
                id, slug, display_name, created_at, updated_at,
                capture_policy, context_budget, document_config, sync_config
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                slug = excluded.slug,
                display_name = excluded.display_name,
                updated_at = excluded.updated_at,
                capture_policy = excluded.capture_policy,
                context_budget = excluded.context_budget,
                document_config = excluded.document_config,
                sync_config = excluded.sync_config
            "#,
            params![
                project.id,
                project.slug,
                project.display_name,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
                serde_json::to_string(&project.capture_policy)?,
                serde_json::to_string(&project.context_budget)?,
                serde_json::to_string(&project.document_config)?,
                serde_json::to_string(&project.sync_config)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_project_by_id(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, created_at, updated_at, capture_policy, context_budget, document_config, sync_config FROM projects WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            Ok(Some(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
                capture_policy: serde_json::from_str(&cp_str)?,
                context_budget: serde_json::from_str(&cb_str)?,
                document_config: serde_json::from_str(&dc_str)?,
                sync_config: serde_json::from_str(&sc_str)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_project_by_slug(&self, slug: &str) -> Result<Option<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, created_at, updated_at, capture_policy, context_budget, document_config, sync_config FROM projects WHERE slug = ?1",
        )?;
        let mut rows = stmt.query(params![slug])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            Ok(Some(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
                capture_policy: serde_json::from_str(&cp_str)?,
                context_budget: serde_json::from_str(&cb_str)?,
                document_config: serde_json::from_str(&dc_str)?,
                sync_config: serde_json::from_str(&sc_str)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, created_at, updated_at, capture_policy, context_budget, document_config, sync_config FROM projects ORDER BY display_name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let capture_policy = serde_json::from_str(&cp_str).unwrap_or_default();
            let context_budget = serde_json::from_str(&cb_str).unwrap_or_default();
            let document_config = serde_json::from_str(&dc_str).unwrap_or_default();
            let sync_config = serde_json::from_str(&sc_str).unwrap_or_default();

            Ok(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at,
                updated_at,
                capture_policy,
                context_budget,
                document_config,
                sync_config,
            })
        })?;

        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn add_project_path(&self, project_id: &str, local_path: &str, is_primary: bool) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO project_paths (project_id, local_path, is_primary, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(project_id, local_path) DO UPDATE SET is_primary = excluded.is_primary
            "#,
            params![project_id, local_path, if is_primary { 1 } else { 0 }, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_project_paths(&self, project_id: &str) -> Result<Vec<ProjectPath>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT project_id, local_path, is_primary, created_at FROM project_paths WHERE project_id = ?1 ORDER BY is_primary DESC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_primary_int: i32 = row.get(2)?;
            Ok(ProjectPath {
                project_id: row.get(0)?,
                local_path: row.get(1)?,
                is_primary: is_primary_int != 0,
                created_at,
            })
        })?;

        let mut paths = Vec::new();
        for p in rows {
            paths.push(p?);
        }
        Ok(paths)
    }

    pub fn find_project_by_path(&self, path: &str) -> Result<Option<Project>> {
        let conn = self.conn();
        // Exact match first, or path starts with registered root
        let mut stmt = conn.prepare(
            r#"
            SELECT p.id, p.slug, p.display_name, p.created_at, p.updated_at,
                   p.capture_policy, p.context_budget, p.document_config, p.sync_config
            FROM projects p
            JOIN project_paths pp ON p.id = pp.project_id
            WHERE ?1 = pp.local_path OR ?1 LIKE (pp.local_path || '/%')
            ORDER BY LENGTH(pp.local_path) DESC
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            Ok(Some(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
                capture_policy: serde_json::from_str(&cp_str)?,
                context_budget: serde_json::from_str(&cb_str)?,
                document_config: serde_json::from_str(&dc_str)?,
                sync_config: serde_json::from_str(&sc_str)?,
            }))
        } else {
            Ok(None)
        }
    }
}
