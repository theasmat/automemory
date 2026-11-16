//! What not to store. Notes the repository already shows (layout, overviews, standard commands) cost tokens at every session and made agents
//! slightly worse in the 2026 context-file studies; what helps is what the code cannot say: decisions, constraints, gotchas, failures with fixes.
//! The check is deliberately narrow and only looks at facts, observations and patterns an agent saved: decisions, lessons, warnings and
//! preferences, and anything a person wrote, are never judged.

use crate::models::MemoryType;

const LAYOUT_CUES: &[&str] = &[
    "directory structure", "folder structure", "file structure", "project structure", "project layout", "directory layout",
    "repo layout", "repository structure", "codebase structure", "repository layout", "file layout",
];
const OVERVIEW_STARTS: &[&str] = &["overview of", "this project is a", "this repo is a", "this repository is a", "the project is a", "the repo is a", "the repository is a"];
const STANDARD_COMMANDS: &[&str] =
    &["run cargo build", "run cargo test", "run npm install", "run pnpm install", "run yarn install", "run pip install", "use cargo build", "use npm install"];

/// Why a note an agent wants to save should not be trusted as knowledge, or None when it is fine.
pub fn derivable(kind: MemoryType, summary: &str, content: &str) -> Option<&'static str> {
    if !matches!(kind, MemoryType::Fact | MemoryType::Observation | MemoryType::Pattern) {
        return None;
    }
    let s = summary.trim().to_lowercase();
    let head: String = content.trim().to_lowercase().chars().take(160).collect();
    if LAYOUT_CUES.iter().any(|c| s.contains(c) || head.starts_with(c)) {
        return Some("describes the repository's layout, which the repository already shows");
    }
    if OVERVIEW_STARTS.iter().any(|c| s.starts_with(c) || head.starts_with(c)) {
        return Some("is a project overview, which the README and code already give");
    }
    if STANDARD_COMMANDS.iter().any(|c| s.starts_with(c)) {
        return Some("is a standard tooling command anyone can find in the repository");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_overviews_and_standard_commands_are_derivable() {
        assert!(derivable(MemoryType::Fact, "Project structure", "src/ has the code").is_some());
        assert!(derivable(MemoryType::Observation, "Notes", "This repository is a Rust workspace with nine crates").is_some());
        assert!(derivable(MemoryType::Fact, "Run cargo test to test", "").is_some());
    }

    #[test]
    fn decisions_gotchas_and_failures_are_never_judged() {
        assert!(derivable(MemoryType::Decision, "Project structure: one crate per bounded context", "").is_none());
        assert!(derivable(MemoryType::Lesson, "This project is a trap for floats", "").is_none());
        assert!(derivable(MemoryType::Warning, "Do not run cargo test without TEST_DATABASE_URL", "").is_none());
        assert!(derivable(MemoryType::Fact, "Money is stored as integer cents", "never floats").is_none());
        assert!(derivable(MemoryType::Fact, "Staging deploys from release/* branches", "").is_none());
    }
}
