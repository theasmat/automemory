use super::sql_str;
use crate::db::Database;
use crate::guardrails::rules::CustomGuardrailRule;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn save_custom_guardrail_rule(&self, rule: &CustomGuardrailRule) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO custom_guardrail_rules (id, project_id, name, category, pattern, is_critical, action, description, mitigation_hint, enabled, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                category = excluded.category,
                pattern = excluded.pattern,
                is_critical = excluded.is_critical,
                action = excluded.action,
                description = excluded.description,
                mitigation_hint = excluded.mitigation_hint,
                enabled = excluded.enabled
            "#,
            params![
                rule.id,
                rule.project_id,
                rule.name,
                rule.category,
                rule.pattern,
                if rule.is_critical { 1 } else { 0 },
                rule.action,
                rule.description,
                rule.mitigation_hint,
                if rule.enabled { 1 } else { 0 },
                rule.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_custom_guardrail_rules(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<CustomGuardrailRule>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, name, category, pattern, is_critical, action, description, mitigation_hint, enabled, created_at FROM custom_guardrail_rules WHERE 1=1".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                sql_str(pid)
            ));
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_custom_rule)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_custom_guardrail_rule(&self, id: &str) -> Result<Option<CustomGuardrailRule>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, name, category, pattern, is_critical, action, description, mitigation_hint, enabled, created_at FROM custom_guardrail_rules WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_custom_rule(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_custom_guardrail_rule(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "DELETE FROM custom_guardrail_rules WHERE id = ?1",
            params![id],
        )?;
        Ok(rows > 0)
    }

    pub fn toggle_custom_guardrail_rule(&self, id: &str, enabled: bool) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE custom_guardrail_rules SET enabled = ?1 WHERE id = ?2",
            params![if enabled { 1 } else { 0 }, id],
        )?;
        Ok(rows > 0)
    }

    fn row_to_custom_rule(row: &rusqlite::Row) -> rusqlite::Result<CustomGuardrailRule> {
        let id: String = row.get(0)?;
        let project_id: Option<String> = row.get(1)?;
        let name: String = row.get(2)?;
        let category: String = row.get(3)?;
        let pattern: String = row.get(4)?;
        let is_critical_int: i32 = row.get(5)?;
        let action: String = row.get(6)?;
        let description: String = row.get(7)?;
        let mitigation_hint: String = row.get(8)?;
        let enabled_int: i32 = row.get(9)?;
        let created_at_str: String = row.get(10)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(CustomGuardrailRule {
            id,
            project_id,
            name,
            category,
            pattern,
            is_critical: is_critical_int != 0,
            action,
            description,
            mitigation_hint,
            enabled: enabled_int != 0,
            created_at,
        })
    }
}
