//! Transcript readers: one file per agent, one line in `registry()`. A reader only finds and parses that agent's chat files;
//! it never writes to them, never redacts (the importer does), and never talks to the database.
//! Adding an agent: see plan/continuity-plan.md §8 (sample fixture, a reader file, a registry line, pass `testkit`).
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

pub mod antigravity;
pub mod claude_code;
#[cfg(test)]
pub mod testkit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// A one-line note of a tool the agent ran ("→ Edit src/a.rs"); never the tool's output.
    Tool,
    /// A plan or notes file the agent wrote about the conversation (not chat).
    Artifact,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
            Self::Artifact => "artifact",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawMessage {
    pub role: Role,
    pub ts: Option<DateTime<Utc>>,
    pub text: String,
}

/// One chat the agent keeps for a project.
#[derive(Debug, Clone)]
pub struct SessionRef {
    pub native_id: String,
    pub path: PathBuf,
    pub modified: Option<DateTime<Utc>>,
}

/// Opaque position in a session; each reader defines what it holds (Claude: a byte offset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor(pub String);

#[derive(Debug, Default)]
pub struct ReadResult {
    pub messages: Vec<RawMessage>,
    pub cursor: Option<Cursor>,
    pub title: Option<String>,
}

pub trait TranscriptReader: Send + Sync {
    /// The agent id the hooks use (`claude`, `agy`, `codex`).
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// The folders that would be read, shown to the person before they opt in.
    fn locations(&self, home: &Path) -> Vec<PathBuf>;
    /// Chats of the project at `project_root`.
    fn sessions(&self, home: &Path, project_root: &Path) -> Vec<SessionRef>;
    /// Messages after `since` (everything when `None`), skipping anything it cannot parse.
    fn read(&self, session: &SessionRef, since: Option<&Cursor>) -> Result<ReadResult>;
}

/// Every reader AutoMemory knows. Add a line here for a new agent.
pub fn registry() -> Vec<Box<dyn TranscriptReader>> {
    vec![Box::new(claude_code::ClaudeCode), Box::new(antigravity::Antigravity)]
}

pub fn reader(agent: &str) -> Option<Box<dyn TranscriptReader>> {
    registry().into_iter().find(|r| r.id() == agent)
}
