use crate::db::Database;
use crate::models::{DocumentRevision, TrackedDocument};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn track_document(
        &self,
        doc: &TrackedDocument,
        initial_content: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn();
        let actual_id: String = conn.query_row(
            r#"
            INSERT INTO tracked_documents (
                id, project_id, relative_path, category, title, content_hash, remote_synced_hash,
                summary, l0_abstract, l1_overview, last_scanned_at, is_deleted
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(project_id, relative_path) DO UPDATE SET
                category = excluded.category,
                title = excluded.title,
                content_hash = excluded.content_hash,
                remote_synced_hash = excluded.remote_synced_hash,
                summary = excluded.summary,
                l0_abstract = excluded.l0_abstract,
                l1_overview = excluded.l1_overview,
                last_scanned_at = excluded.last_scanned_at,
                is_deleted = excluded.is_deleted
            RETURNING id
            "#,
            params![
                doc.id,
                doc.project_id,
                doc.relative_path,
                doc.category,
                doc.title,
                doc.content_hash,
                doc.remote_synced_hash,
                doc.summary,
                doc.l0_abstract,
                doc.l1_overview,
                doc.last_scanned_at.to_rfc3339(),
                if doc.is_deleted { 1 } else { 0 },
            ],
            |row| row.get(0),
        )?;

        if let Some(content) = initial_content {
            let already_stored: bool = conn.query_row(
                "SELECT 1 FROM document_revisions WHERE document_id = ?1 AND content_hash = ?2 LIMIT 1",
                params![actual_id, doc.content_hash],
                |_| Ok(true),
            ).unwrap_or(false);

            if !already_stored {
                let next_rev: i64 = conn.query_row(
                    "SELECT COALESCE(MAX(revision_num), 0) + 1 FROM document_revisions WHERE document_id = ?1",
                    params![actual_id],
                    |r| r.get(0),
                ).unwrap_or(1);

                conn.execute(
                    r#"
                    INSERT INTO document_revisions (id, document_id, revision_num, content, content_hash, created_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![
                        uuid::Uuid::new_v4().to_string(),
                        actual_id,
                        next_rev,
                        content,
                        doc.content_hash,
                        Utc::now().to_rfc3339(),
                    ],
                )?;
            }
        }

        Ok(())
    }

    pub fn list_documents(&self, project_id: &str) -> Result<Vec<TrackedDocument>> {
        self.list_all_tracked_documents(Some(project_id))
    }

    pub fn list_all_tracked_documents(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<TrackedDocument>> {
        let conn = self.conn();
        let query = match project_id {
            Some(_) => "SELECT id, project_id, relative_path, category, title, content_hash, remote_synced_hash, summary, l0_abstract, l1_overview, last_scanned_at, is_deleted FROM tracked_documents WHERE (project_id = ?1 OR project_id IN (SELECT slug FROM projects WHERE id = ?1) OR project_id IN (SELECT id FROM projects WHERE slug = ?1)) AND is_deleted = 0 ORDER BY relative_path ASC",
            None => "SELECT id, project_id, relative_path, category, title, content_hash, remote_synced_hash, summary, l0_abstract, l1_overview, last_scanned_at, is_deleted FROM tracked_documents WHERE is_deleted = 0 ORDER BY project_id, relative_path ASC",
        };

        let mut stmt = conn.prepare(query)?;
        let row_mapper = |row: &rusqlite::Row| {
            let created_at_str: String = row.get(10)?;
            let last_scanned_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_del: i32 = row.get(11)?;

            Ok(TrackedDocument {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                category: row.get(3)?,
                title: row.get(4)?,
                content_hash: row.get(5)?,
                remote_synced_hash: row.get(6)?,
                summary: row.get(7)?,
                l0_abstract: row.get(8)?,
                l1_overview: row.get(9)?,
                last_scanned_at,
                is_deleted: is_del != 0,
            })
        };

        let rows: rusqlite::MappedRows<_> = match project_id {
            Some(pid) => stmt.query_map(params![pid], row_mapper)?,
            None => stmt.query_map([], row_mapper)?,
        };

        let mut list = Vec::new();
        for d in rows {
            list.push(d?);
        }
        Ok(list)
    }

    pub fn get_document_by_path(
        &self,
        project_id: &str,
        rel_path: &str,
    ) -> Result<Option<TrackedDocument>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, relative_path, category, title, content_hash, remote_synced_hash, summary, l0_abstract, l1_overview, last_scanned_at, is_deleted FROM tracked_documents WHERE project_id = ?1 AND relative_path = ?2",
        )?;
        let mut rows = stmt.query(params![project_id, rel_path])?;
        if let Some(row) = rows.next()? {
            let scanned_str: String = row.get(10)?;
            let last_scanned_at = DateTime::parse_from_rfc3339(&scanned_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_del: i32 = row.get(11)?;

            Ok(Some(TrackedDocument {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                category: row.get(3)?,
                title: row.get(4)?,
                content_hash: row.get(5)?,
                remote_synced_hash: row.get(6)?,
                summary: row.get(7)?,
                l0_abstract: row.get(8)?,
                l1_overview: row.get(9)?,
                last_scanned_at,
                is_deleted: is_del != 0,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_document_l0_l1(
        &self,
        doc_id: &str,
        l0: Option<&str>,
        l1: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE tracked_documents SET l0_abstract = ?1, l1_overview = ?2, last_scanned_at = ?3 WHERE id = ?4",
            params![l0, l1, Utc::now().to_rfc3339(), doc_id],
        )?;
        Ok(())
    }

    pub fn update_document_synced_hash(&self, doc_id: &str, synced_hash: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE tracked_documents SET remote_synced_hash = ?1, last_scanned_at = ?2 WHERE id = ?3",
            params![synced_hash, Utc::now().to_rfc3339(), doc_id],
        )?;
        Ok(())
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

    pub fn record_document_revision(
        &self,
        document_id: &str,
        content: &str,
        hash: &str,
    ) -> Result<DocumentRevision> {
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
