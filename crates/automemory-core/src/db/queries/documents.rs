use crate::db::Database;
use crate::models::{DocumentRevision, TrackedDocument};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn track_document(&self, doc: &TrackedDocument, initial_content: Option<&str>) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO tracked_documents (id, project_id, relative_path, content_hash, summary, last_scanned_at, is_deleted)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(project_id, relative_path) DO UPDATE SET
                content_hash = excluded.content_hash,
                summary = excluded.summary,
                last_scanned_at = excluded.last_scanned_at,
                is_deleted = excluded.is_deleted
            "#,
            params![
                doc.id,
                doc.project_id,
                doc.relative_path,
                doc.content_hash,
                doc.summary,
                doc.last_scanned_at.to_rfc3339(),
                if doc.is_deleted { 1 } else { 0 },
            ],
        )?;

        if let Some(content) = initial_content {
            conn.execute(
                r#"
                INSERT INTO document_revisions (id, document_id, revision_num, content, content_hash, created_at)
                VALUES (?1, ?2, 1, ?3, ?4, ?5)
                "#,
                params![
                    uuid::Uuid::new_v4().to_string(),
                    doc.id,
                    content,
                    doc.content_hash,
                    Utc::now().to_rfc3339(),
                ],
            )?;
        }

        Ok(())
    }

    pub fn list_documents(&self, project_id: &str) -> Result<Vec<TrackedDocument>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, relative_path, content_hash, summary, last_scanned_at, is_deleted FROM tracked_documents WHERE project_id = ?1 AND is_deleted = 0 ORDER BY relative_path ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let created_at_str: String = row.get(5)?;
            let last_scanned_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_del: i32 = row.get(6)?;

            Ok(TrackedDocument {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                content_hash: row.get(3)?,
                summary: row.get(4)?,
                last_scanned_at,
                is_deleted: is_del != 0,
            })
        })?;

        let mut list = Vec::new();
        for d in rows {
            list.push(d?);
        }
        Ok(list)
    }

    pub fn get_document_by_path(&self, project_id: &str, rel_path: &str) -> Result<Option<TrackedDocument>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, relative_path, content_hash, summary, last_scanned_at, is_deleted FROM tracked_documents WHERE project_id = ?1 AND relative_path = ?2",
        )?;
        let mut rows = stmt.query(params![project_id, rel_path])?;
        if let Some(row) = rows.next()? {
            let scanned_str: String = row.get(5)?;
            let last_scanned_at = DateTime::parse_from_rfc3339(&scanned_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_del: i32 = row.get(6)?;

            Ok(Some(TrackedDocument {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                content_hash: row.get(3)?,
                summary: row.get(4)?,
                last_scanned_at,
                is_deleted: is_del != 0,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_latest_revision(&self, document_id: &str) -> Result<Option<DocumentRevision>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, document_id, revision_num, content, content_hash, created_at FROM document_revisions WHERE document_id = ?1 ORDER BY revision_num DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![document_id])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(5)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Some(DocumentRevision {
                id: row.get(0)?,
                document_id: row.get(1)?,
                revision_num: row.get(2)?,
                content: row.get(3)?,
                content_hash: row.get(4)?,
                created_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn record_document_revision(&self, document_id: &str, content: &str, hash: &str) -> Result<DocumentRevision> {
        let conn = self.conn();
        let next_rev: i32 = conn.query_row(
            "SELECT COALESCE(MAX(revision_num), 0) + 1 FROM document_revisions WHERE document_id = ?1",
            params![document_id],
            |r| r.get(0),
        )?;

        let rev_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        conn.execute(
            r#"
            INSERT INTO document_revisions (id, document_id, revision_num, content, content_hash, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![rev_id, document_id, next_rev, content, hash, now.to_rfc3339()],
        )?;

        conn.execute(
            r#"
            UPDATE tracked_documents SET content_hash = ?1, last_scanned_at = ?2, is_deleted = 0 WHERE id = ?3
            "#,
            params![hash, now.to_rfc3339(), document_id],
        )?;

        Ok(DocumentRevision {
            id: rev_id,
            document_id: document_id.to_string(),
            revision_num: next_rev,
            content: content.to_string(),
            content_hash: hash.to_string(),
            created_at: now,
        })
    }

    pub fn untrack_document(&self, project_id: &str, rel_path: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE tracked_documents SET is_deleted = 1 WHERE project_id = ?1 AND relative_path = ?2",
            params![project_id, rel_path],
        )?;
        Ok(rows > 0)
    }
}
