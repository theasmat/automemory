//! The project's own documents (PRD.md, docs/**, ADRs...) that AutoMemory finds in the repository, shown next to the library files so there is one place to
//! look. They stay in the repository (git is their home); the library only lists and reads them.
use crate::db::Database;
use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct RepoDoc {
    pub path: String,
    pub category: String,
    pub title: String,
    pub summary: String,
}

pub fn list(db: &Database, project_id: &str) -> Result<Vec<RepoDoc>> {
    let mut docs: Vec<RepoDoc> = db
        .list_documents(project_id)?
        .into_iter()
        .filter(|d| !d.is_deleted)
        .map(|d| RepoDoc { path: d.relative_path, category: d.category, title: d.title, summary: d.l0_abstract.or(d.summary).unwrap_or_default() })
        .collect();
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(docs)
}

pub fn project_root(db: &Database, project_id: &str) -> Option<PathBuf> {
    db.conn()
        .query_row("SELECT local_path FROM project_paths WHERE project_id = ?1 ORDER BY is_primary DESC LIMIT 1", [project_id], |r| r.get::<_, String>(0))
        .ok()
        .map(PathBuf::from)
}

/// One document at a level: 0 = the one-line abstract, 1 = the overview, 2 = the file itself (read from the repository, never outside the project folder).
pub fn get(db: &Database, project_id: &str, path: &str, tier: u8) -> Result<Option<String>> {
    let Some(doc) = db.get_document_by_path(project_id, path)?.filter(|d| !d.is_deleted) else { return Ok(None) };
    Ok(match tier {
        0 => Some(doc.l0_abstract.or(doc.summary).unwrap_or_else(|| doc.title.clone())),
        1 => Some(doc.l1_overview.or(doc.l0_abstract).unwrap_or_default()),
        _ => {
            let root = project_root(db, project_id).unwrap_or_default();
            let full = root.join(&doc.relative_path);
            // The path comes from our own table, but never follow it out of the project.
            let inside = std::fs::canonicalize(&full).ok().zip(std::fs::canonicalize(&root).ok()).is_some_and(|(f, r)| f.starts_with(&r));
            if inside { std::fs::read_to_string(&full).ok() } else { None }
        }
    })
}

