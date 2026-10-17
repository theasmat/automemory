use super::sql_str;
use crate::db::Database;
use crate::models::{MemoryScope, Preference};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;

impl Database {
    pub fn set_preference(&self, pref: &Preference) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO preferences (id, key, value, scope, project_id, locked, source, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(key, scope, COALESCE(project_id, '')) DO UPDATE SET
                value = excluded.value,
                locked = excluded.locked,
                source = excluded.source,
                updated_at = excluded.updated_at
            "#,
            params![
                pref.id,
                pref.key,
                pref.value.to_string(),
                pref.scope.to_string(),
                pref.project_id,
                if pref.locked { 1 } else { 0 },
                pref.source,
                pref.created_at.to_rfc3339(),
                pref.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_preference(
        &self,
        key: &str,
        project_id: Option<&str>,
    ) -> Result<Option<Preference>> {
        let conn = self.conn();
        // Project preference overrides global preference
        let mut stmt = conn.prepare(
            r#"
            SELECT id, key, value, scope, project_id, locked, source, created_at, updated_at
            FROM preferences
            WHERE key = ?1 AND (
                (scope = 'project' AND project_id = ?2) OR
                (scope = 'global')
            )
            ORDER BY CASE WHEN scope = 'project' THEN 0 ELSE 1 END
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![key, project_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_preference(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_preferences(&self, project_id: Option<&str>) -> Result<Vec<Preference>> {
        let conn = self.conn();
        let mut sql = "SELECT id, key, value, scope, project_id, locked, source, created_at, updated_at FROM preferences WHERE 1=1".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (scope = 'global' OR project_id = '{}')",
                sql_str(pid)
            ));
        }
        sql.push_str(" ORDER BY scope ASC, key ASC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_preference)?;
        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn lock_preference(
        &self,
        key: &str,
        project_id: Option<&str>,
        locked: bool,
    ) -> Result<bool> {
        let conn = self.conn();
        let rows = if let Some(pid) = project_id {
            conn.execute(
                "UPDATE preferences SET locked = ?1, updated_at = ?2 WHERE key = ?3 AND project_id = ?4",
                params![if locked { 1 } else { 0 }, Utc::now().to_rfc3339(), key, pid],
            )?
        } else {
            conn.execute(
                "UPDATE preferences SET locked = ?1, updated_at = ?2 WHERE key = ?3 AND scope = 'global'",
                params![if locked { 1 } else { 0 }, Utc::now().to_rfc3339(), key],
            )?
        };
        Ok(rows > 0)
    }

    fn row_to_preference(row: &rusqlite::Row) -> rusqlite::Result<Preference> {
        let id: String = row.get(0)?;
        let key: String = row.get(1)?;
        let val_str: String = row.get(2)?;
        let scope_str: String = row.get(3)?;
        let project_id: Option<String> = row.get(4)?;
        let locked_int: i32 = row.get(5)?;
        let source: String = row.get(6)?;
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;

        let value: serde_json::Value = serde_json::from_str(&val_str).unwrap_or_default();
        let scope = MemoryScope::from_str(&scope_str).unwrap_or(MemoryScope::Global);
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Preference {
            id,
            key,
            value,
            scope,
            project_id,
            locked: locked_int != 0,
            source,
            created_at,
            updated_at,
        })
    }
}
