/// Escapes a value for use inside a single-quoted SQLite string literal.
/// Dynamic filters are assembled as text, so every interpolated value goes through this.
pub(crate) fn sql_str(v: impl std::fmt::Display) -> String {
    v.to_string().replace('\'', "''")
}

/// Builds an FTS5 prefix query where every alphanumeric word is a quoted prefix term.
pub(crate) fn fts_query(q: &str) -> String {
    q.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub mod batons;
pub mod decisions;
pub mod documents;
pub mod events;
pub mod failures;
pub mod guardrails;
pub mod leases;
pub mod memories;
pub mod preferences;
pub mod projects;
pub mod proposals;
pub mod prune;
pub mod rooms;
pub mod skills;
pub mod stats;
pub mod tasks;
pub mod traces;
