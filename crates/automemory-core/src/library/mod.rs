//! Project library: documents for agents (PRDs, plans, notes) kept outside git. L1: the store, deterministic tiers, search and the
//! index shown to agents. Files appear in the project folder (L2) and sync through the vault (L3) in later phases (plan/library-plan.md).
pub mod fs;
pub mod model;
pub mod propose;
pub mod store;
pub mod tiers;

pub use model::{LibraryFile, NewFile, Origin, PutOutcome};
pub use store::LibraryRecord;

/// Tokens (4 chars each) the session-start index may use.
pub const INDEX_TOKENS: usize = 600;

/// The short "Project library" block for the session-start pack: one line per file (path and its L0 line), within the budget.
pub fn index_block(db: &crate::db::Database, project_id: Option<&str>, max_tokens: usize) -> Option<String> {
    let files = db.library_list(project_id, None).ok()?;
    if files.is_empty() {
        return None;
    }
    let budget = max_tokens.max(100) * 4;
    let mut out = String::from("Project library (documents kept outside git; read one with library.get {path}, find with library.search, or open .automemory/library/INDEX.md):\n");
    let mut omitted = 0;
    for f in &files {
        let line = format!("- {} - {}{}\n", f.path, f.l0, if f.origin == Origin::Agent { format!(" (agent-written by {})", f.author) } else if f.linked { " (linked, in the project)".to_string() } else { String::new() });
        if out.len() + line.len() > budget {
            omitted += 1;
        } else {
            out.push_str(&line);
        }
    }
    if omitted > 0 {
        out.push_str(&format!("...{omitted} more: call library.list.\n"));
    }
    Some(out.trim_end().to_string())
}
