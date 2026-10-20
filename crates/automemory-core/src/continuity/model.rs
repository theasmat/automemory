//! What a hand-off carries beyond tool calls: the goal, what was ruled out and why, what is open and next.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const TEXT_MAX: usize = 400;
pub const REASON_MAX: usize = 300;
/// Active items per project; resolve or remove some before adding more.
pub const ACTIVE_MAX: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Goal,
    Status,
    Constraint,
    Decision,
    Rejected,
    Open,
    Next,
    File,
    Glossary,
    Preference,
}

impl ItemKind {
    /// Delivery order and heading.
    pub const ALL: [ItemKind; 10] = [
        Self::Goal, Self::Status, Self::Constraint, Self::Decision, Self::Next, Self::Open, Self::Rejected, Self::File, Self::Preference, Self::Glossary,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Goal => "goal",
            Self::Status => "status",
            Self::Constraint => "constraint",
            Self::Decision => "decision",
            Self::Rejected => "rejected",
            Self::Open => "open",
            Self::Next => "next",
            Self::File => "file",
            Self::Glossary => "glossary",
            Self::Preference => "preference",
        }
    }

    pub fn heading(&self) -> &'static str {
        match self {
            Self::Goal => "Goal",
            Self::Status => "Where it stands",
            Self::Constraint => "Constraints",
            Self::Decision => "Decided",
            Self::Rejected => "Ruled out (do not redo)",
            Self::Open => "Open questions",
            Self::Next => "Next steps",
            Self::File => "Key files",
            Self::Glossary => "Terms",
            Self::Preference => "Preferences seen",
        }
    }

    /// One current goal and one current status; adding a new one retires the old.
    pub fn is_singleton(&self) -> bool {
        matches!(self, Self::Goal | Self::Status)
    }
}

impl FromStr for ItemKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        Self::ALL.iter().copied().find(|k| k.as_str() == s.trim().to_ascii_lowercase()).ok_or_else(|| {
            format!("unknown kind '{s}': use {}", Self::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", "))
        })
    }
}

/// Where an item came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Human,
    Agent,
    /// Drafted from a chat transcript (later phases).
    Transcript,
}

/// How far to trust it: a draft nobody confirmed, something an agent or a person stands behind, or a person's edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemState {
    Draft,
    Confirmed,
    Edited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Active,
    /// An open question answered, a next step done, a goal replaced: kept as history, not delivered.
    Resolved,
    Removed,
}

macro_rules! text_enum {
    ($t:ty, $($v:ident => $s:literal),+) => {
        impl $t {
            pub fn as_str(&self) -> &'static str { match self { $(Self::$v => $s),+ } }
        }
        impl FromStr for $t {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, String> {
                match s { $($s => Ok(Self::$v),)+ other => Err(format!("unknown value '{other}'")) }
            }
        }
    };
}
text_enum!(Origin, Human => "human", Agent => "agent", Transcript => "transcript");
text_enum!(ItemState, Draft => "draft", Confirmed => "confirmed", Edited => "edited");
text_enum!(ItemStatus, Active => "active", Resolved => "resolved", Removed => "removed");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefItem {
    pub id: String,
    pub project_id: Option<String>,
    pub kind: ItemKind,
    pub text: String,
    /// Why (for ruled-out options and decisions).
    pub reason: Option<String>,
    /// A decision, memory or file this item points at.
    pub ref_id: Option<String>,
    pub origin: Origin,
    /// Agent name, or `human`.
    pub author: String,
    pub session: Option<String>,
    pub state: ItemState,
    pub status: ItemStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// What a writer supplies; the store fills in the rest.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub project_id: Option<String>,
    pub kind: ItemKind,
    pub text: String,
    pub reason: Option<String>,
    pub ref_id: Option<String>,
    pub origin: Origin,
    pub author: String,
    pub session: Option<String>,
}
