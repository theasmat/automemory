use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Room lifecycle: exactly one propose → critique → revise pass, then decided.
/// The machine terminates structurally; no round caps needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoomState {
    #[default]
    Open,
    Critique,
    Revise,
    Decided,
}

impl std::fmt::Display for RoomState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Critique => write!(f, "critique"),
            Self::Revise => write!(f, "revise"),
            Self::Decided => write!(f, "decided"),
        }
    }
}

impl std::str::FromStr for RoomState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "critique" => Ok(Self::Critique),
            "revise" => Ok(Self::Revise),
            "decided" => Ok(Self::Decided),
            other => Err(format!("Unknown room state: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostKind {
    Propose,
    Critique,
    Revise,
    /// A human remark: no turn, no state change, never evidence.
    Note,
}

impl std::fmt::Display for PostKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Propose => write!(f, "propose"),
            Self::Critique => write!(f, "critique"),
            Self::Revise => write!(f, "revise"),
            Self::Note => write!(f, "note"),
        }
    }
}

impl std::str::FromStr for PostKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "propose" => Ok(Self::Propose),
            "critique" => Ok(Self::Critique),
            "revise" => Ok(Self::Revise),
            "note" => Ok(Self::Note),
            other => Err(format!("Unknown post kind: {other}")),
        }
    }
}

impl PostKind {
    /// The only kind each state accepts.
    pub fn expected_in(state: RoomState) -> Option<Self> {
        match state {
            RoomState::Open => Some(Self::Propose),
            RoomState::Critique => Some(Self::Critique),
            RoomState::Revise => Some(Self::Revise),
            RoomState::Decided => None,
        }
    }
}

fn default_rounds() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub project_id: Option<String>,
    pub topic: String,
    pub state: RoomState,
    #[serde(default)]
    pub members: Vec<String>,
    pub turn_holder: Option<String>,
    pub round: i64,
    #[serde(default = "default_rounds")]
    pub rounds: i64,
    #[serde(default)]
    pub parent_room: Option<String>,
    #[serde(default)]
    pub turn_started_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One turn-gated contribution. Rationale stays short; evidence IDs must
/// resolve to stored records (memory, trace, or failure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomPost {
    pub id: String,
    pub room_id: String,
    pub author_session: String,
    pub kind: PostKind,
    pub body: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub created_at: DateTime<Utc>,
}
