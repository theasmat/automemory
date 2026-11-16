use crate::db::Database;
use crate::models::{PostKind, Room, RoomPost, RoomState};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

/// Rationale stays short: stored posts are capped, never raw reasoning.
pub const ROOM_BODY_BUDGET: usize = 2000;
/// A turn is spent once (one round per room), so a post must say something: "test" or "lgtm" is refused.
pub const ROOM_BODY_MIN: usize = 25;

const ROOM_COLS: &str = "id, project_id, topic, state, members, turn_holder, round, created_at, updated_at, rounds, parent_room, turn_started_at";

impl Database {
    pub fn create_room(
        &self,
        project_id: Option<&str>,
        topic: &str,
        creator_session: &str,
    ) -> Result<Room> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let room = Room {
            id: format!("room-{}", Uuid::now_v7().simple()),
            project_id: project_id.map(str::to_string),
            topic: topic.to_string(),
            state: RoomState::Open,
            members: vec![creator_session.to_string()],
            turn_holder: Some(creator_session.to_string()),
            round: 0,
            rounds: 1,
            parent_room: None,
            turn_started_at: Some(now),
            created_at: now,
            updated_at: now,
        };
        self.conn().execute(
            "INSERT INTO rooms (id, project_id, topic, state, members, turn_holder, round, rounds, parent_room, turn_started_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'open', ?4, ?5, 0, 1, NULL, ?6, ?6, ?6)",
            params![
                room.id,
                room.project_id,
                room.topic,
                serde_json::to_string(&room.members)?,
                room.turn_holder,
                now_str,
            ],
        )?;
        Ok(room)
    }

    pub fn get_room(&self, id: &str) -> Result<Option<Room>> {
        let conn = self.conn();
        let sql = format!("SELECT {ROOM_COLS} FROM rooms WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_room(row)?))
        } else {
            Ok(None)
        }
    }

    /// Adds a session to the room. Joining a decided room is refused.
    pub fn join_room(&self, id: &str, session_id: &str) -> Result<Option<Room>> {
        let mut room = match self.get_room(id)? {
            Some(r) => r,
            None => return Ok(None),
        };
        if room.state == RoomState::Decided {
            return Err(anyhow!("room is decided"));
        }
        if !room.members.contains(&session_id.to_string()) {
            room.members.push(session_id.to_string());
            let now = Utc::now();
            // An unheld turn, or one an agent holds only because it was alone after its own post, goes to the newcomer.
            let last_author = self.room_posts(id)?.last().map(|p| p.author_session.clone());
            if room.turn_holder.is_none() || (room.turn_holder.is_some() && room.turn_holder == last_author) {
                room.turn_holder = Some(session_id.to_string());
                room.turn_started_at = Some(now);
            }
            room.updated_at = now;
            self.conn().execute(
                "UPDATE rooms SET members = ?1, turn_holder = ?2, turn_started_at = ?3, updated_at = ?4 WHERE id = ?5",
                params![
                    serde_json::to_string(&room.members)?,
                    room.turn_holder,
                    room.turn_started_at.map(|t| t.to_rfc3339()),
                    room.updated_at.to_rfc3339(),
                    room.id,
                ],
            )?;
        }
        Ok(Some(room))
    }

    /// Rooms awaiting action: undecided rooms where the turn is unheld or
    /// belongs to `session_id` (None lists every open room).
    pub fn pending_rooms(
        &self,
        project_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Vec<Room>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {ROOM_COLS}
             FROM rooms
             WHERE state != 'decided'
               AND (?1 IS NULL OR project_id = ?1 OR project_id IS NULL)
               AND (?2 IS NULL OR turn_holder IS NULL OR turn_holder = ?2)
             ORDER BY updated_at DESC
             LIMIT 20"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rooms = stmt
            .query_map(params![project_id, session_id], Self::row_to_room)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rooms)
    }

    /// Posts one turn: enforces turn holder, kind-per-state, and resolvable
    /// evidence, then advances the machine and rotates the turn.
    pub fn post_to_room(
        &self,
        room_id: &str,
        author_session: &str,
        kind: PostKind,
        body: &str,
        evidence: &[String],
    ) -> Result<RoomPost> {
        self.post_to_room_with(
            &crate::ports::PcrProtocol,
            room_id,
            author_session,
            kind,
            body,
            evidence,
        )
    }

    /// Same as above, but the turn rules come from the injected RoomProtocol:
    /// alternative room formats (review-only, vote) plug in here.
    pub fn post_to_room_with(
        &self,
        protocol: &dyn crate::ports::RoomProtocol,
        room_id: &str,
        author_session: &str,
        kind: PostKind,
        body: &str,
        evidence: &[String],
    ) -> Result<RoomPost> {
        let mut room = self
            .get_room(room_id)?
            .ok_or_else(|| anyhow!("room not found"))?;
        if kind == PostKind::Note {
            return Err(anyhow!("notes come from the human, through the dashboard"));
        }
        protocol.validate_turn(
            room.state,
            kind,
            room.turn_holder.as_deref(),
            author_session,
            evidence.is_empty(),
        )?;
        if body.trim().chars().count() < ROOM_BODY_MIN {
            return Err(anyhow!("a post needs at least {ROOM_BODY_MIN} characters: state your {kind} and the reason, not a placeholder"));
        }
        let prior_posts = self.room_posts(room_id)?;
        let previous = prior_posts.iter().rev().find(|p| p.kind != PostKind::Note);
        if kind == PostKind::Critique && previous.is_some_and(|p| p.author_session == author_session) {
            return Err(anyhow!("a critique must come from another agent: wait for one to join"));
        }
        let proposer = prior_posts
            .iter()
            .rev()
            .find(|p| matches!(p.kind, PostKind::Propose | PostKind::Revise))
            .map(|p| p.author_session.clone())
            .or_else(|| (kind == PostKind::Propose).then(|| author_session.to_string()));
        if kind == PostKind::Critique && proposer.as_deref() == Some(author_session) {
            return Err(anyhow!("a critique must come from another agent, not the proposer"));
        }
        for id in evidence {
            if !self.evidence_resolves(id)? {
                return Err(anyhow!("evidence does not resolve: {id}"));
            }
        }

        if !room.members.contains(&author_session.to_string()) {
            room.members.push(author_session.to_string());
        }

        let now = Utc::now();
        let mut capped = body.trim().to_string();
        if capped.len() > ROOM_BODY_BUDGET {
            let mut idx = ROOM_BODY_BUDGET;
            while !capped.is_char_boundary(idx) {
                idx = idx.saturating_sub(1);
            }
            capped.truncate(idx);
            capped.push_str("\n…[truncated]");
        }
        let post = RoomPost {
            id: format!("post-{}", Uuid::now_v7().simple()),
            room_id: room_id.to_string(),
            author_session: author_session.to_string(),
            kind,
            body: capped,
            evidence: evidence.to_vec(),
            created_at: now,
        };
        let (next_state, next_turn) = match room.state {
            RoomState::Open => {
                let t = Some(next_member(&room.members, author_session)).filter(|n| n != author_session);
                (RoomState::Critique, t)
            }
            RoomState::Critique => {
                // Multi-critic: every non-proposer member in the room critiques once per round before Revise.
                let mut critiqued: Vec<&str> = prior_posts
                    .iter()
                    .rev()
                    .take_while(|p| !matches!(p.kind, PostKind::Propose | PostKind::Revise))
                    .filter(|p| p.kind == PostKind::Critique)
                    .map(|p| p.author_session.as_str())
                    .collect();
                critiqued.push(author_session);
                let prop_str = proposer.as_deref().unwrap_or("");
                let next_critic = next_unseen_critic(&room.members, author_session, prop_str, &critiqued);
                // An invited agent that has not joined yet is still owed its critique: leave the turn open for it
                // (the human can Skip turn if it never comes). Only when the sessions can be matched to agent names.
                let waiting_on = {
                    let agent_of = |s: &str| self.session_agent_id(s).map(|a| canon_agent(&a));
                    let proposer_agent = agent_of(prop_str);
                    let done: Vec<String> = critiqued.iter().filter_map(|s| agent_of(s)).collect();
                    if proposer_agent.is_none() && done.is_empty() {
                        Vec::new()
                    } else {
                        self.room_meta(room_id)
                            .unwrap_or_default()
                            .invited
                            .iter()
                            .map(|a| canon_agent(a))
                            .filter(|a| Some(a) != proposer_agent.as_ref() && !done.contains(a))
                            .collect::<Vec<_>>()
                    }
                };
                if let Some(c) = next_critic {
                    (RoomState::Critique, Some(c))
                } else if !waiting_on.is_empty() {
                    (RoomState::Critique, None)
                } else {
                    let rev_holder = if !prop_str.is_empty() {
                        Some(prop_str.to_string())
                    } else {
                        Some(next_member(&room.members, author_session)).filter(|n| n != author_session)
                    };
                    (RoomState::Revise, rev_holder)
                }
            }
            RoomState::Revise => {
                let completed_revisions = prior_posts.iter().filter(|p| p.kind == PostKind::Revise).count() as i64 + 1;
                if completed_revisions < room.rounds.max(1) {
                    let t = Some(next_member(&room.members, author_session)).filter(|n| n != author_session);
                    (RoomState::Critique, t)
                } else {
                    (RoomState::Decided, None)
                }
            }
            RoomState::Decided => (RoomState::Decided, None),
        };
        let now_str = now.to_rfc3339();
        let turn_started = (next_state != RoomState::Decided).then(|| now_str.clone());
        // Another process may have posted since the turn was checked: the room row is the compare-and-swap (its round must be unchanged), and the post is stored in the same transaction.
        let conn = self.conn();
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let stored = (|| -> Result<()> {
            let moved = conn.execute(
                "UPDATE rooms SET members = ?1, state = ?2, turn_holder = ?3, round = round + 1, turn_started_at = ?4, updated_at = ?5 WHERE id = ?6 AND round = ?7",
                params![
                    serde_json::to_string(&room.members)?,
                    next_state.to_string(),
                    next_turn,
                    turn_started,
                    now_str,
                    room_id,
                    room.round
                ],
            )?;
            if moved == 0 {
                return Err(anyhow!("another agent posted first: read the room again before you answer"));
            }
            conn.execute(
                "INSERT INTO room_posts (id, room_id, author_session, kind, body, evidence, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    post.id,
                    post.room_id,
                    post.author_session,
                    kind.to_string(),
                    post.body,
                    serde_json::to_string(&post.evidence)?,
                    now.to_rfc3339(),
                ],
            )?;

            Ok(())
        })();
        conn.execute_batch(if stored.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        stored?;
        Ok(post)
    }

    pub fn room_posts(&self, room_id: &str) -> Result<Vec<RoomPost>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, room_id, author_session, kind, body, evidence, created_at
             FROM room_posts WHERE room_id = ?1 ORDER BY created_at ASC",
        )?;
        let posts = stmt
            .query_map(params![room_id], |row| {
                let kind_str: String = row.get(3)?;
                let ev_str: String = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(RoomPost {
                    id: row.get(0)?,
                    room_id: row.get(1)?,
                    author_session: row.get(2)?,
                    kind: PostKind::from_str(&kind_str).unwrap_or(PostKind::Propose),
                    body: row.get(4)?,
                    evidence: serde_json::from_str(&ev_str).unwrap_or_default(),
                    created_at: DateTime::parse_from_rfc3339(&created_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(posts)
    }

    fn evidence_resolves(&self, id: &str) -> Result<bool> {
        // A document in the project library: `library:plans/prd.md`.
        if let Some(path) = id.strip_prefix("library:") {
            return Ok(self.conn().query_row("SELECT COUNT(*) FROM library_files WHERE path = ?1 AND status = 'active'", [path.trim()], |r| r.get::<_, i64>(0))? > 0);
        }
        if self.get_memory_by_id(id)?.is_some() {
            return Ok(true);
        }
        if self.get_trace_by_id(id)?.is_some() {
            return Ok(true);
        }
        if self.get_failure_by_id(id)?.is_some() {
            return Ok(true);
        }
        if self.conn().query_row("SELECT COUNT(*) FROM decisions WHERE id = ?1", [id.trim()], |r| r.get::<_, i64>(0))? > 0 {
            return Ok(true);
        }
        Ok(false)
    }

    fn row_to_room(row: &rusqlite::Row) -> rusqlite::Result<Room> {
        let members_str: String = row.get(4)?;
        let state_str: String = row.get(3)?;
        let created_str: String = row.get(7)?;
        let updated_str: String = row.get(8)?;
        let rounds: i64 = row.get::<_, Option<i64>>(9)?.unwrap_or(1).max(1);
        let parent_room: Option<String> = row.get(10)?;
        let turn_started_at: Option<DateTime<Utc>> = row
            .get::<_, Option<String>>(11)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc)));
        Ok(Room {
            id: row.get(0)?,
            project_id: row.get(1)?,
            topic: row.get(2)?,
            state: RoomState::from_str(&state_str).unwrap_or(RoomState::Open),
            members: serde_json::from_str(&members_str).unwrap_or_default(),
            turn_holder: row.get(5)?,
            round: row.get(6)?,
            rounds,
            parent_room,
            turn_started_at,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

/// One structured step in a room's plan or human draft.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanStep {
    pub title: String,
    #[serde(default, alias = "assigned_agent")]
    pub owner: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default, alias = "check_cmd")]
    pub check: Option<String>,
}

/// Editable human draft for a room (`room_draft:<id>` in `sync_kv`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RoomDraft {
    pub decision: String,
    #[serde(default)]
    pub steps: Vec<PlanStep>,
    pub updated_at: String,
}

/// What approving a decided room would store: the decision and the tasks.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoomPlan {
    pub topic: String,
    pub decision: String,
    pub reason: String,
    pub dissent: Option<String>,
    pub tasks: Vec<String>,
    pub steps: Vec<PlanStep>,
    pub evidence: Vec<String>,
}

/// What a person asked for when starting a room from the dashboard. Rooms are per machine (they do not sync), so this
/// lives in `sync_kv` as `room_meta:<id>`.
// shortcut: kept in sync_kv, not columns; move to a migration if rooms ever sync between machines.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RoomMeta {
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Agent names as the hooks know them (`claude`, `agy`, `codex`).
    #[serde(default)]
    pub invited: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub rounds: Option<i64>,
    #[serde(default)]
    pub parent_room: Option<String>,
}

/// The human's verdict on a decided room, kept so approving twice is impossible.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomOutcome {
    pub outcome: String, // "approved" | "rejected" | "ended"
    pub decision_id: Option<String>,
    pub task_ids: Vec<String>,
    pub at: String,
    #[serde(default)]
    pub edited_by_human: bool,
    #[serde(default)]
    pub superseded_room_id: Option<String>,
    #[serde(default)]
    pub cancelled_tasks: usize,
    #[serde(default)]
    pub flagged_claimed_tasks: Vec<String>,
}

use crate::text::clip;

/// Steps of a plan: bullet or numbered lines, at most 10.
/// The agent a step names as its doer: "claude-code builds …", "codex adds …", "agy builds …". A step that starts with
/// anything else has no owner, and the first agent to ask claims it.
pub fn step_owner(title: &str) -> Option<String> {
    let first = title.split_whitespace().next()?.trim_matches(|c: char| !c.is_alphanumeric() && c != '-').to_lowercase();
    ["claude", "codex", "cursor"]
        .iter()
        .find(|a| first.starts_with(**a))
        .map(|a| a.to_string())
        .or_else(|| ["agy", "antigravity", "jetski"].iter().any(|a| first.starts_with(*a)).then(|| "agy".to_string()))
}

pub fn extract_steps(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|l| {
            let t = l.trim_start();
            let rest = t.strip_prefix(['-', '*', '•']).or_else(|| {
                let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
                (digits > 0).then(|| t[digits..].strip_prefix(['.', ')'])).flatten()
            })?;
            let step = rest.trim();
            (!step.is_empty()).then(|| clip(step, 120))
        })
        .take(10)
        .collect()
}

fn clean_plan_steps(steps: &[PlanStep]) -> Vec<PlanStep> {
    steps
        .iter()
        .filter_map(|s| {
            let title = clip(s.title.trim(), 120);
            if title.is_empty() {
                return None;
            }
            let owner = s
                .owner
                .as_deref()
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(|o| clip(o, 40));
            let files: Vec<String> = s
                .files
                .iter()
                .map(|f| clip(f.trim(), 120))
                .filter(|f| !f.is_empty())
                .take(10)
                .collect();
            let check = s
                .check
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(|c| clip(c, 200));
            Some(PlanStep {
                title,
                owner,
                files,
                check,
            })
        })
        .take(20)
        .collect()
}

impl Database {
    /// Every room, newest first, with how many posts it holds.
    pub fn list_rooms(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<(Room, i64)>> {
        let rooms: Vec<Room> = {
            let conn = self.conn();
            let sql = format!(
                "SELECT {ROOM_COLS} FROM rooms
                 WHERE (?1 IS NULL OR project_id = ?1 OR project_id IS NULL) ORDER BY updated_at DESC LIMIT ?2"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![project_id, limit as i64], Self::row_to_room)?.collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        Ok(rooms.into_iter().map(|r| { let n = self.room_posts(&r.id).map(|p| p.len() as i64).unwrap_or(0); (r, n) }).collect())
    }

    pub fn room_meta(&self, room_id: &str) -> Option<RoomMeta> {
        self.sync_kv_get(&format!("room_meta:{room_id}")).and_then(|j| serde_json::from_str(&j).ok())
    }

    pub fn room_draft(&self, room_id: &str) -> Option<RoomDraft> {
        self.sync_kv_get(&format!("room_draft:{room_id}"))
            .and_then(|j| serde_json::from_str(&j).ok())
    }

    pub fn save_room_draft(
        &self,
        room_id: &str,
        decision: &str,
        steps: &[PlanStep],
    ) -> Result<RoomDraft> {
        if self.get_room(room_id)?.is_none() {
            return Err(anyhow!("room not found"));
        }
        if self.room_outcome(room_id).is_some() {
            return Err(anyhow!("this room already has an outcome"));
        }
        let draft = RoomDraft {
            decision: clip(decision.trim(), 200),
            steps: clean_plan_steps(steps),
            updated_at: Utc::now().to_rfc3339(),
        };
        self.sync_kv_set(
            &format!("room_draft:{room_id}"),
            &serde_json::to_string(&draft)?,
        );
        Ok(draft)
    }

    pub fn superseded_by_room(&self, room_id: &str) -> Option<String> {
        self.sync_kv_get(&format!("superseded_by_room:{room_id}"))
    }

    /// True when an undecided room's current turn has waited 30 minutes or more.
    pub fn is_room_turn_stalled(&self, room: &Room) -> bool {
        if room.state == RoomState::Decided || self.room_outcome(&room.id).is_some() {
            return false;
        }
        let since = room.turn_started_at.unwrap_or(room.updated_at);
        (Utc::now() - since).num_minutes() >= 30
    }

    /// A room you start: nobody holds the turn, so the first invited agent to join proposes.
    pub fn create_room_by_human(&self, project_id: Option<&str>, topic: &str, mut meta: RoomMeta) -> Result<Room> {
        let topic = clip(topic, 120);
        if topic.is_empty() {
            return Err(anyhow!("the topic is empty"));
        }
        if meta.question.trim().is_empty() {
            return Err(anyhow!("write the question the agents should answer"));
        }
        if meta.invited.is_empty() {
            return Err(anyhow!("invite at least one agent"));
        }
        for id in &meta.evidence {
            if !self.evidence_resolves(id)? {
                return Err(anyhow!("evidence does not resolve: {id}"));
            }
        }
        let rounds = meta.rounds.unwrap_or(1).clamp(1, 3);
        meta.rounds = Some(rounds);
        meta.question = clip(&meta.question, 400);
        meta.created_by = "human".into();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let id = format!("room-{}", Uuid::now_v7().simple());
        self.conn().execute(
            "INSERT INTO rooms (id, project_id, topic, state, members, turn_holder, round, rounds, parent_room, turn_started_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'open', '[]', NULL, 0, ?4, ?5, ?6, ?6, ?6)",
            params![id, project_id, topic, rounds, meta.parent_room, now_str],
        )?;
        self.sync_kv_set(&format!("room_meta:{id}"), &serde_json::to_string(&meta)?);
        self.get_room(&id)?.ok_or_else(|| anyhow!("room vanished"))
    }

    /// Opens a follow-up room seeded with the approved room's decision and metadata.
    pub fn revise_room(&self, room_id: &str) -> Result<Room> {
        let old_room = self
            .get_room(room_id)?
            .ok_or_else(|| anyhow!("room not found"))?;
        let outcome = self
            .room_outcome(room_id)
            .ok_or_else(|| anyhow!("only an approved room can be revised"))?;
        if outcome.outcome != "approved" {
            return Err(anyhow!("only an approved room can be revised"));
        }
        let mut meta = self.room_meta(room_id).unwrap_or_default();
        if meta.question.trim().is_empty() {
            meta.question = format!("How should we revise the plan for {}?", old_room.topic);
        }
        if meta.invited.is_empty() {
            let mut inferred: Vec<String> = old_room
                .members
                .iter()
                .filter_map(|s| self.sync_kv_get(&format!("session_agent:{s}")))
                .filter(|a| !a.trim().is_empty())
                .collect();
            inferred.sort();
            inferred.dedup();
            if inferred.is_empty() {
                inferred = vec!["claude".to_string(), "agy".to_string()];
            }
            meta.invited = inferred;
        }
        if let Some(ref dec_id) = outcome.decision_id {
            if !meta.evidence.contains(dec_id) {
                meta.evidence.push(dec_id.clone());
            }
        }
        meta.parent_room = Some(room_id.to_string());
        self.create_room_by_human(old_room.project_id.as_deref(), &old_room.topic, meta)
    }

    /// Skips a stalled turn, handing the turn to the next member (or advancing the phase when the last critic/reviser is skipped).
    pub fn skip_room_turn(&self, room_id: &str) -> Result<Room> {
        let room = self
            .get_room(room_id)?
            .ok_or_else(|| anyhow!("room not found"))?;
        if room.state == RoomState::Decided || self.room_outcome(room_id).is_some() {
            return Err(anyhow!("the room is already decided"));
        }
        let current = room.turn_holder.as_deref().unwrap_or("");
        let prior_posts = self.room_posts(room_id)?;
        let proposer = prior_posts
            .iter()
            .rev()
            .find(|p| matches!(p.kind, PostKind::Propose | PostKind::Revise))
            .map(|p| p.author_session.clone());
        let (next_state, next_turn) = match room.state {
            RoomState::Critique => {
                let mut critiqued: Vec<&str> = prior_posts
                    .iter()
                    .rev()
                    .take_while(|p| !matches!(p.kind, PostKind::Propose | PostKind::Revise))
                    .filter(|p| p.kind == PostKind::Critique)
                    .map(|p| p.author_session.as_str())
                    .collect();
                if !current.is_empty() {
                    critiqued.push(current);
                }
                let prop = proposer.as_deref().unwrap_or(current);
                if let Some(next_critic) =
                    next_unseen_critic(&room.members, current, prop, &critiqued)
                {
                    (RoomState::Critique, Some(next_critic))
                } else {
                    (
                        RoomState::Revise,
                        proposer.or_else(|| Some(next_member(&room.members, current))),
                    )
                }
            }
            RoomState::Revise => (RoomState::Decided, None),
            RoomState::Open => {
                let next = if room.members.len() >= 2 && !current.is_empty() {
                    Some(next_member(&room.members, current)).filter(|n| n != current)
                } else {
                    None
                };
                (RoomState::Open, next)
            }
            RoomState::Decided => (RoomState::Decided, None),
        };
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "UPDATE rooms SET state = ?1, turn_holder = ?2, round = round + 1, turn_started_at = ?3, updated_at = ?3 WHERE id = ?4",
            params![next_state.to_string(), next_turn, now, room_id],
        )?;
        self.get_room(room_id)?
            .ok_or_else(|| anyhow!("room vanished"))
    }

    /// Closes an undecided room without storing a decision or tasks.
    pub fn end_room(&self, room_id: &str) -> Result<RoomOutcome> {
        if self.get_room(room_id)?.is_none() {
            return Err(anyhow!("room not found"));
        }
        if self.room_outcome(room_id).is_some() {
            return Err(anyhow!("this room already has an outcome"));
        }
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "UPDATE rooms SET state = 'decided', turn_holder = NULL, turn_started_at = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, room_id],
        )?;
        let out = RoomOutcome {
            outcome: "ended".into(),
            decision_id: None,
            task_ids: vec![],
            at: now,
            edited_by_human: false,
            superseded_room_id: None,
            cancelled_tasks: 0,
            flagged_claimed_tasks: vec![],
        };
        self.sync_kv_set(
            &format!("room_outcome:{room_id}"),
            &serde_json::to_string(&out)?,
        );
        Ok(out)
    }

    /// One line per room that waits for this agent: its turn, or an open turn in a room it was invited to. Each line is
    /// given once per turn (the hooks fire on every prompt), at most two rooms.
    pub fn room_lines_for(&self, project_id: Option<&str>, session_id: &str, agent: &str) -> Vec<String> {
        let Ok(rooms) = self.list_rooms(project_id, 20) else { return vec![] };
        let mut lines = Vec::new();
        for (room, _) in rooms {
            if room.state == RoomState::Decided || self.room_outcome(&room.id).is_some() {
                continue;
            }
            let stored = self.room_meta(&room.id);
            let meta = stored.clone().unwrap_or_default();
            let invited = meta.invited.iter().any(|a| a == agent) || room.members.iter().any(|m| m == session_id);
            let last = self.room_posts(&room.id).ok().and_then(|p| p.into_iter().rev().find(|p| p.kind != PostKind::Note)).map(|p| p.author_session);
            let mine = room.turn_holder.as_deref() == Some(session_id)
                || (room.turn_holder.is_none() && invited && last.as_deref() != Some(session_id));
            if !mine {
                continue;
            }
            let told = format!("room_told:{session_id}:{}:{}:{}", room.id, room.state, room.round);
            if self.sync_kv_get(&told).is_some() {
                continue;
            }
            self.sync_kv_set(&told, "1");
            let what = match room.state { RoomState::Open => "proposal", RoomState::Critique => "critique", _ => "revision" };
            // A room a person started without a question (older ones): say so, so the agent states its assumptions
            // instead of guessing the goal (trial T11b: "cart" produced a proposal for an API the shop does not have).
            let question = match (&stored, meta.question.is_empty()) {
                (_, false) => format!(" Question: {}", meta.question),
                (Some(_), true) => " No question was given: read room.status for notes from the human, and state your assumptions at the top of your post instead of guessing the goal.".to_string(),
                (None, true) => String::new(),
            };
            lines.push(format!(
                "[AutoMemory] Planning room \"{}\" ({}) waits for your {what}.{question} Join with room.join (room_id, session_id \"{session_id}\", agent), read room.status, then room.post.",
                room.topic, room.id
            ));
            if lines.len() == 2 {
                break;
            }
        }
        lines
    }

    pub fn room_outcome(&self, room_id: &str) -> Option<RoomOutcome> {
        self.sync_kv_get(&format!("room_outcome:{room_id}")).and_then(|j| serde_json::from_str(&j).ok())
    }

    /// The plan a decided room produced: the last revision, the objection it answered, the steps it lists.
    pub fn room_plan(&self, room_id: &str) -> Result<RoomPlan> {
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        if room.state != RoomState::Decided {
            return Err(anyhow!("the room is not decided yet"));
        }
        let posts = self.room_posts(room_id)?;
        let revise = posts.iter().rev().find(|p| p.kind == PostKind::Revise).ok_or_else(|| anyhow!("no revision in the room"))?;
        let critique = posts.iter().rev().find(|p| p.kind == PostKind::Critique);
        let decision = revise.body.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| clip(l.trim_start_matches(['#', ' ']), 200)).unwrap_or_default();
        let mut evidence: Vec<String> = posts.iter().flat_map(|p| p.evidence.clone()).chain(self.room_meta(room_id).unwrap_or_default().evidence).collect();
        evidence.sort();
        evidence.dedup();
        let tasks = extract_steps(&revise.body);
        let steps = tasks
            .iter()
            .map(|t| PlanStep {
                title: t.clone(),
                owner: step_owner(t),
                ..Default::default()
            })
            .collect();
        Ok(RoomPlan {
            topic: room.topic,
            decision,
            reason: format!("Agreed in planning room {room_id}: {}", clip(&revise.body, 600)),
            dissent: critique.map(|c| clip(&c.body, 400)),
            tasks,
            steps,
            evidence,
        })
    }

    /// Human approval of the room's plan: uses the saved draft if one exists, else the agents' plan as written.
    pub fn approve_room(&self, room_id: &str) -> Result<RoomOutcome> {
        if let Some(draft) = self.room_draft(room_id) {
            if !draft.decision.trim().is_empty() {
                return self.decide_room_steps(room_id, &draft.decision, &draft.steps);
            }
        }
        let plan = self.room_plan(room_id)?;
        self.store_plan(room_id, plan, "room", false)
    }

    /// The human decides, at any stage: the decision text and steps are theirs (prefilled from the agents' plan
    /// when there is one). Ends the discussion.
    pub fn decide_room(&self, room_id: &str, decision: &str, steps: &[String]) -> Result<RoomOutcome> {
        let structured: Vec<PlanStep> = steps
            .iter()
            .map(|t| PlanStep {
                title: t.clone(),
                ..Default::default()
            })
            .collect();
        self.decide_room_steps(room_id, decision, &structured)
    }

    pub fn decide_room_steps(
        &self,
        room_id: &str,
        decision: &str,
        steps: &[PlanStep],
    ) -> Result<RoomOutcome> {
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        if decision.trim().is_empty() {
            return Err(anyhow!("the decision is empty"));
        }
        let agents = self.room_plan(room_id).ok();
        let cleaned_steps = clean_plan_steps(steps);
        let step_titles: Vec<String> = cleaned_steps.iter().map(|s| s.title.clone()).collect();
        let has_step_metadata = cleaned_steps
            .iter()
            .any(|s| s.owner.is_some() || !s.files.is_empty() || s.check.is_some());
        let unchanged = !has_step_metadata
            && agents
                .as_ref()
                .is_some_and(|p| p.decision == decision.trim() && p.tasks == step_titles);
        let posts = self.room_posts(room_id)?;
        let mut evidence: Vec<String> = posts
            .iter()
            .flat_map(|p| p.evidence.clone())
            .chain(self.room_meta(room_id).unwrap_or_default().evidence)
            .collect();
        evidence.sort();
        evidence.dedup();
        let plan = RoomPlan {
            topic: room.topic.clone(),
            decision: clip(decision, 200),
            reason: match &agents {
                Some(p) if unchanged => p.reason.clone(),
                Some(p) => format!(
                    "Decided by the human in planning room {room_id} (edited by human), starting from the agents' plan: {}",
                    clip(&p.decision, 300)
                ),
                None => format!(
                    "Decided by the human in planning room {room_id} after {} post(s).",
                    posts.len()
                ),
            },
            dissent: posts
                .iter()
                .rev()
                .find(|p| p.kind == PostKind::Critique)
                .map(|c| clip(&c.body, 400)),
            tasks: step_titles,
            steps: cleaned_steps,
            evidence,
        };
        let out = self.store_plan(
            room_id,
            plan,
            if unchanged { "room" } else { "human" },
            !unchanged,
        )?;
        self.conn().execute(
            "UPDATE rooms SET state = 'decided', turn_holder = NULL, turn_started_at = NULL, updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), room_id],
        )?;
        Ok(out)
    }

    fn store_plan(
        &self,
        room_id: &str,
        plan: RoomPlan,
        agent: &str,
        edited_by_human: bool,
    ) -> Result<RoomOutcome> {
        if self.room_outcome(room_id).is_some() {
            return Err(anyhow!("this room already has an outcome"));
        }
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        let mut symbols: Vec<String> = plan
            .steps
            .iter()
            .flat_map(|s| s.files.clone())
            .collect();
        symbols.sort();
        symbols.dedup();
        let decision = self.record_decision(crate::models::DecisionRecordArgs {
            project_id: room.project_id.as_deref(),
            topic: &plan.topic,
            decision: &plan.decision,
            reason: &plan.reason,
            alternatives: &[],
            dissent: plan.dissent.as_deref(),
            agent,
            symbols: &symbols,
            evidence: &plan.evidence,
        })?;
        let mut task_ids = Vec::new();
        let spec_base = format!(
            "From the planning room \"{}\": {}",
            plan.topic, plan.decision
        );
        for (idx, step) in plan.steps.iter().enumerate() {
            let t = self.create_room_task(
                room_id,
                room.project_id.as_deref(),
                idx as i64,
                &step.title,
                &spec_base,
                step.owner.as_deref(),
                &step.files,
                step.check.as_deref(),
            )?;
            task_ids.push(t.id);
        }
        let mut cancelled_tasks = 0;
        let mut flagged_claimed_tasks = Vec::new();
        let parent_id = room
            .parent_room
            .clone()
            .or_else(|| self.room_meta(room_id).and_then(|m| m.parent_room));
        if let Some(ref pid) = parent_id {
            if let Ok((cancelled, claimed)) = self.cancel_unclaimed_room_tasks(pid) {
                cancelled_tasks = cancelled;
                flagged_claimed_tasks = claimed.into_iter().map(|t| t.title).collect();
            }
            self.sync_kv_set(&format!("superseded_by_room:{pid}"), room_id);
        }
        let out = RoomOutcome {
            outcome: "approved".into(),
            decision_id: Some(decision.id),
            task_ids,
            at: Utc::now().to_rfc3339(),
            edited_by_human,
            superseded_room_id: parent_id,
            cancelled_tasks,
            flagged_claimed_tasks,
        };
        self.sync_kv_set(&format!("room_outcome:{room_id}"), &serde_json::to_string(&out)?);
        Ok(out)
    }

    /// A human remark in the room: no turn, no state change. Agents see it in room.status.
    pub fn add_room_note(&self, room_id: &str, text: &str) -> Result<RoomPost> {
        if self.get_room(room_id)?.is_none() {
            return Err(anyhow!("room not found"));
        }
        if self.room_outcome(room_id).is_some() {
            return Err(anyhow!("the room is closed"));
        }
        let text = clip(text, ROOM_BODY_BUDGET);
        if text.is_empty() {
            return Err(anyhow!("the note is empty"));
        }
        let post = RoomPost {
            id: format!("post-{}", Uuid::now_v7().simple()),
            room_id: room_id.to_string(),
            author_session: "human".into(),
            kind: PostKind::Note,
            body: text,
            evidence: vec![],
            created_at: Utc::now(),
        };
        self.conn().execute(
            "INSERT INTO room_posts (id, room_id, author_session, kind, body, evidence, created_at) VALUES (?1, ?2, 'human', 'note', ?3, '[]', ?4)",
            params![post.id, room_id, post.body, post.created_at.to_rfc3339()],
        )?;
        Ok(post)
    }

    /// A readable name per session: what the agent said it is, else the agent its hooks recorded, else a short id.
    pub fn session_label(&self, session: &str) -> String {
        if session == "human" {
            return "You".into();
        }
        let name = self.sync_kv_get(&format!("session_agent:{session}")).filter(|n| !n.is_empty()).or_else(|| {
            self.conn().query_row("SELECT agent_adapter FROM tool_events WHERE session_id = ?1 LIMIT 1", [session], |r| r.get::<_, String>(0)).ok()
        });
        let pretty = |n: &str| match n {
            "claude" | "claude-code" => "Claude Code".to_string(),
            "agy" | "antigravity" => "Antigravity".to_string(),
            "codex" => "Codex".to_string(),
            other => other.to_string(),
        };
        let short: String = session.chars().take(6).collect();
        match name {
            Some(n) => format!("{} ({short})", pretty(&n)),
            None => format!("agent {short}"),
        }
    }

    pub fn reject_room(&self, room_id: &str) -> Result<RoomOutcome> {
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        if room.state != RoomState::Decided || self.room_outcome(room_id).is_some() {
            return Err(anyhow!("only a decided room without an outcome can be rejected"));
        }
        let out = RoomOutcome {
            outcome: "rejected".into(),
            decision_id: None,
            task_ids: vec![],
            at: Utc::now().to_rfc3339(),
            edited_by_human: false,
            superseded_room_id: None,
            cancelled_tasks: 0,
            flagged_claimed_tasks: vec![],
        };
        self.sync_kv_set(&format!("room_outcome:{room_id}"), &serde_json::to_string(&out)?);
        Ok(out)
    }

    /// The human tiebreak: send a decided room back for another critique and revision.
    pub fn another_round(&self, room_id: &str) -> Result<Room> {
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        if room.state != RoomState::Decided || self.room_outcome(room_id).is_some() {
            return Err(anyhow!("only a decided room without an outcome can go another round"));
        }
        let last_author = self.room_posts(room_id)?.last().map(|p| p.author_session.clone()).unwrap_or_default();
        let turn = next_member(&room.members, &last_author);
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "UPDATE rooms SET state = 'critique', turn_holder = ?1, turn_started_at = ?2, updated_at = ?2 WHERE id = ?3",
            params![turn, now, room_id],
        )?;
        self.get_room(room_id)?.ok_or_else(|| anyhow!("room vanished"))
    }
    /// A lightweight watermark string of the latest room, post, or task change for WebSocket live updates.
    pub fn room_activity_watermark(&self) -> String {
        let conn = self.conn();
        let r: String = conn
            .query_row(
                "SELECT COALESCE(MAX(updated_at), '') FROM rooms",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        let p: String = conn
            .query_row(
                "SELECT COALESCE(MAX(created_at), '') FROM room_posts",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        let t: String = conn
            .query_row(
                "SELECT COALESCE(MAX(updated_at), '') FROM tasks",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        format!("{r}|{p}|{t}")
    }
}

/// Round-robin turn rotation. Solo rooms rotate to themselves.
fn next_member(members: &[String], author: &str) -> String {
    if members.is_empty() {
        return author.to_string();
    }
    let pos = members.iter().position(|m| m == author).unwrap_or(0);
    members[(pos + 1) % members.len()].clone()
}

/// Next member after `author` who is not `proposer` and has not yet critiqued in the current round.
/// One name per agent however it is spelled: `claude-code` and `Claude Code` are `claude`; `antigravity` and `jetski` are `agy`.
fn canon_agent(name: &str) -> String {
    let n = name.trim().to_lowercase().replace(['_', ' '], "-");
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["claude"]) {
        "claude".into()
    } else if has(&["antigravity", "jetski", "agy"]) {
        "agy".into()
    } else if has(&["codex"]) {
        "codex".into()
    } else {
        n
    }
}

fn next_unseen_critic(
    members: &[String],
    author: &str,
    proposer: &str,
    critiqued: &[&str],
) -> Option<String> {
    if members.len() <= 2 {
        return None;
    }
    let start = members.iter().position(|m| m == author).unwrap_or(0);
    for offset in 1..members.len() {
        let candidate = &members[(start + offset) % members.len()];
        if candidate != proposer && !critiqued.contains(&candidate.as_str()) {
            return Some(candidate.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Memory, MemoryStatus};
    use crate::models::{MemoryScope, MemorySourceKind, MemoryType};

    fn seed_memory(db: &Database) -> String {
        let mem = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: None,
            memory_type: MemoryType::Decision,
            content: "JWT in cookie".to_string(),
            summary: "auth decision".to_string(),
            confidence: 0.95,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory_direct(&mem).unwrap();
        mem.id
    }

    #[test]
    fn room_walks_propose_critique_revise_decided() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "auth", "sess-a").unwrap();
        assert_eq!(room.state, RoomState::Open);
        db.join_room(&room.id, "sess-b").unwrap();

        // Wrong turn and wrong kind are refused.
        assert!(db
            .post_to_room(
                &room.id,
                "sess-b",
                PostKind::Propose,
                "x - padded so it passes the minimum length",
                std::slice::from_ref(&ev)
            )
            .is_err());
        assert!(db
            .post_to_room(
                &room.id,
                "sess-a",
                PostKind::Critique,
                "x - padded so it passes the minimum length",
                std::slice::from_ref(&ev)
            )
            .is_err());
        // Dangling evidence is refused.
        assert!(db
            .post_to_room(
                &room.id,
                "sess-a",
                PostKind::Propose,
                "x - padded so it passes the minimum length",
                &["missing".to_string()]
            )
            .is_err());
        // Propose without evidence is refused.
        assert!(db
            .post_to_room(&room.id, "sess-a", PostKind::Propose, "x - padded so it passes the minimum length", &[])
            .is_err());

        db.post_to_room(
            &room.id,
            "sess-a",
            PostKind::Propose,
            "JWT plan - padded so it passes the minimum length",
            std::slice::from_ref(&ev),
        )
        .unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.state, RoomState::Critique);
        assert_eq!(r.turn_holder.as_deref(), Some("sess-b"));

        db.post_to_room(
            &room.id,
            "sess-b",
            PostKind::Critique,
            "rotation? - padded so it passes the minimum length",
            std::slice::from_ref(&ev),
        )
        .unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.state, RoomState::Revise);
        assert_eq!(r.turn_holder.as_deref(), Some("sess-a"));

        db.post_to_room(
            &room.id,
            "sess-a",
            PostKind::Revise,
            "JWT + rotation - padded so it passes the minimum length",
            &[ev],
        )
        .unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.state, RoomState::Decided);
        assert_eq!(r.turn_holder, None);

        // Decided rooms reject posts and joins.
        assert!(db
            .post_to_room(&room.id, "sess-b", PostKind::Critique, "late - padded so it passes the minimum length", &[])
            .is_err());
        assert!(db.join_room(&room.id, "sess-c").is_err());
        assert_eq!(db.room_posts(&room.id).unwrap().len(), 3);
    }

    fn decided_room(db: &Database, ev: &str, revise: &str) -> String {
        let room = db.create_room(None, "auth strategy", "sess-a").unwrap();
        db.join_room(&room.id, "sess-b").unwrap();
        db.post_to_room(&room.id, "sess-a", PostKind::Propose, "Use sessions - padded so it passes the minimum length", &[ev.to_string()]).unwrap();
        db.post_to_room(&room.id, "sess-b", PostKind::Critique, "Sessions need sticky routing", &[ev.to_string()]).unwrap();
        db.post_to_room(&room.id, "sess-a", PostKind::Revise, &format!("{revise}\n(agreed by both agents in this room)"), &[ev.to_string()]).unwrap();
        room.id
    }

    #[test]
    fn steps_are_read_from_bullets_and_numbers_only() {
        let steps = extract_steps("Plan\n1. Add the token table\n2) Write the login route\n- Add tests\nnot a step\n  * Update docs\n-\n");
        assert_eq!(steps, vec!["Add the token table", "Write the login route", "Add tests", "Update docs"]);
        assert_eq!(extract_steps(&"- x\n".repeat(30)).len(), 10);
        assert_eq!(step_owner("claude-code builds invoicer/pricing.py"), Some("claude".into()));
        assert_eq!(step_owner("codex adds sample orders"), Some("codex".into()));
        assert_eq!(step_owner("agy builds render"), Some("agy".into()));
        assert_eq!(step_owner("Antigravity writes the docs"), Some("agy".into()));
        assert_eq!(step_owner("Add the token table"), None);
        assert_eq!(step_owner(""), None);
    }

    #[test]
    fn approving_a_decided_room_stores_one_decision_and_its_tasks_once() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let id = decided_room(&db, &ev, "Use JWT in a cookie\n- Add the token table\n- Write the login route");
        let plan = db.room_plan(&id).unwrap();
        assert_eq!(plan.decision, "Use JWT in a cookie");
        assert_eq!(plan.tasks.len(), 2);
        assert!(plan.dissent.as_deref().unwrap().contains("sticky routing"));

        let out = db.approve_room(&id).unwrap();
        assert_eq!(out.task_ids.len(), 2);
        let decisions = db.list_decisions(None, None, 10).unwrap();
        assert!(decisions.iter().any(|d| d.topic == "auth strategy" && d.agent == "room" && d.dissent.is_some()));
        assert_eq!(db.list_tasks(None, None, 10).unwrap().len(), 2);
        assert!(db.approve_room(&id).is_err(), "no second approval");
        assert!(db.reject_room(&id).is_err() && db.another_round(&id).is_err(), "an outcome is final");
    }

    #[test]
    fn rejecting_stores_nothing_and_another_round_reopens_the_critique() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let open = db.create_room(None, "undecided", "sess-a").unwrap();
        assert!(db.approve_room(&open.id).is_err() && db.another_round(&open.id).is_err(), "only decided rooms");

        let r1 = decided_room(&db, &ev, "Plan A\n- one");
        db.reject_room(&r1).unwrap();
        assert!(db.list_decisions(None, None, 10).unwrap().is_empty() && db.list_tasks(None, None, 10).unwrap().is_empty());

        let r2 = decided_room(&db, &ev, "Plan B\n- two");
        let room = db.another_round(&r2).unwrap();
        assert_eq!(room.state, RoomState::Critique);
        assert_eq!(room.turn_holder.as_deref(), Some("sess-b"), "the other agent critiques the revision");
        db.post_to_room(&r2, "sess-b", PostKind::Critique, "Still unclear - padded so it passes the minimum length", std::slice::from_ref(&ev)).unwrap();
        db.post_to_room(&r2, "sess-a", PostKind::Revise, "Plan B2\n- two\n- three - padded so it passes the minimum length", &[ev]).unwrap();
        assert_eq!(db.room_plan(&r2).unwrap().tasks.len(), 2, "the latest revision is the plan");
    }

    #[test]
    fn pending_lists_only_actionable_rooms() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let r1 = db.create_room(None, "one", "sess-a").unwrap();
        let r2 = db.create_room(None, "two", "sess-b").unwrap();

        let mine = db.pending_rooms(None, Some("sess-a")).unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].id, r1.id);

        db.post_to_room(&r2.id, "sess-b", PostKind::Propose, "p - padded so it passes the minimum length", &[ev])
            .unwrap();
        // r2 now waits on sess-b itself (solo rotation); sess-a sees only r1.
        // Alone after its proposal, sess-b does not keep the turn: r2 is open to whoever joins, so sess-a sees both.
        let mine = db.pending_rooms(None, Some("sess-a")).unwrap();
        assert_eq!(mine.len(), 2);
    }

    #[test]
    fn a_lone_proposer_does_not_keep_the_turn_and_cannot_critique_itself() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "cart", "sess-a").unwrap();
        db.post_to_room(&room.id, "sess-a", PostKind::Propose, "db - padded so it passes the minimum length", std::slice::from_ref(&ev)).unwrap();
        assert_eq!(db.get_room(&room.id).unwrap().unwrap().turn_holder, None);
        assert!(db.post_to_room(&room.id, "sess-a", PostKind::Critique, "self - padded so it passes the minimum length", std::slice::from_ref(&ev)).is_err());
        let r = db.join_room(&room.id, "sess-b").unwrap().unwrap();
        assert_eq!(r.turn_holder.as_deref(), Some("sess-b"));
        db.post_to_room(&room.id, "sess-b", PostKind::Critique, "cookie is enough - padded so it passes the minimum length", std::slice::from_ref(&ev)).unwrap();
        assert_eq!(db.get_room(&room.id).unwrap().unwrap().turn_holder.as_deref(), Some("sess-a"));
    }

    #[test]
    fn a_room_stuck_on_its_own_author_is_handed_to_the_next_joiner() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "cart", "sess-a").unwrap();
        db.post_to_room(&room.id, "sess-a", PostKind::Propose, "db - padded so it passes the minimum length", &[ev]).unwrap();
        // Rooms made by the old rotation kept the turn on the proposer.
        db.conn().execute("UPDATE rooms SET turn_holder = 'sess-a' WHERE id = ?1", [&room.id]).unwrap();
        assert_eq!(db.join_room(&room.id, "sess-b").unwrap().unwrap().turn_holder.as_deref(), Some("sess-b"));
    }

    #[test]
    fn a_room_you_start_tells_invited_agents_once_per_turn() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        assert!(db.create_room_by_human(None, "cart", RoomMeta { invited: vec!["claude".into()], ..Default::default() }).is_err(), "a question is required");
        assert!(db.create_room_by_human(None, "cart", RoomMeta { question: "Q?".into(), ..Default::default() }).is_err(), "someone must be invited");
        let meta = RoomMeta { question: "Where do carts live?".into(), invited: vec!["claude".into(), "agy".into()], evidence: vec![ev.clone()], ..Default::default() };
        let room = db.create_room_by_human(None, "cart", meta).unwrap();
        assert_eq!((room.members.len(), room.turn_holder.clone()), (0, None));

        let lines = db.room_lines_for(None, "c1", "claude");
        assert!(lines.len() == 1 && lines[0].contains("waits for your proposal") && lines[0].contains("Where do carts live?"), "{lines:?}");
        assert!(db.room_lines_for(None, "c1", "claude").is_empty(), "told once per turn");
        assert!(db.room_lines_for(None, "x1", "codex").is_empty(), "not invited, not told");


        db.join_room(&room.id, "c1").unwrap();
        db.post_to_room(&room.id, "c1", PostKind::Propose, "SQLite - padded so it passes the minimum length", std::slice::from_ref(&ev)).unwrap();
        assert!(db.room_lines_for(None, "c1", "claude").is_empty(), "the proposer is not asked to critique itself");
        let agy = db.room_lines_for(None, "g1", "agy");
        assert!(agy.len() == 1 && agy[0].contains("critique"), "{agy:?}");
        db.join_room(&room.id, "g1").unwrap();
        db.post_to_room(&room.id, "g1", PostKind::Critique, "expiry? - padded so it passes the minimum length", std::slice::from_ref(&ev)).unwrap();
        assert!(db.room_lines_for(None, "c1", "claude")[0].contains("revision"), "the proposer is told to revise");
        db.post_to_room(&room.id, "c1", PostKind::Revise, "SQLite with expiry\n- add table", std::slice::from_ref(&ev)).unwrap();
        assert!(db.room_lines_for(None, "g1", "agy").is_empty() && db.room_plan(&room.id).unwrap().evidence.contains(&ev));
    }

    #[test]
    fn a_placeholder_post_is_refused_and_does_not_spend_the_turn() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "cart", "sess-a").unwrap();
        db.join_room(&room.id, "sess-b").unwrap();
        for junk in ["test", "ok", "lgtm", "   ", "looks fine to me"] {
            let e = db.post_to_room(&room.id, "sess-a", PostKind::Propose, junk, std::slice::from_ref(&ev)).unwrap_err().to_string();
            assert!(e.contains("at least"), "{junk:?}: {e}");
        }
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Open, Some("sess-a")), "the turn is still the proposer's");
        db.post_to_room(&room.id, "sess-a", PostKind::Propose, "Keep carts in SQLite, keyed by the cart cookie", &[ev]).unwrap();
    }

    #[test]
    fn a_human_room_without_a_question_says_so_and_agent_rooms_do_not() {
        let db = Database::open_in_memory().unwrap();
        db.sync_kv_set("room_meta:legacy", &serde_json::to_string(&RoomMeta { invited: vec!["claude".into()], ..Default::default() }).unwrap());
        db.conn().execute("INSERT INTO rooms (id, project_id, topic, state, members, turn_holder, round, created_at, updated_at) VALUES ('legacy', NULL, 'cart', 'open', '[]', NULL, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')", []).unwrap();
        let legacy = db.room_lines_for(None, "c2", "claude");
        assert!(legacy.len() == 1 && legacy[0].contains("No question was given") && legacy[0].contains("state your assumptions"), "{legacy:?}");
        let made = db.create_room(None, "by agent", "g9").unwrap();
        let line = db.room_lines_for(None, "g9", "agy");
        assert!(line.iter().all(|l| !l.contains("No question")), "an agent-made room has no brief and no such remark: {line:?} {}", made.id);
    }

    #[test]
    fn the_human_can_note_and_decide_at_any_stage() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "cart", "sess-a").unwrap();
        db.post_to_room(&room.id, "sess-a", PostKind::Propose, "db - padded so it passes the minimum length", &[ev]).unwrap();
        let note = db.add_room_note(&room.id, "It must survive a login").unwrap();
        assert_eq!(note.kind, PostKind::Note);
        assert_eq!(db.get_room(&room.id).unwrap().unwrap().state, RoomState::Critique, "a note changes no state");
        assert!(db.post_to_room(&room.id, "sess-a", PostKind::Note, "x", &[]).is_err(), "agents cannot post notes");
        let out = db.decide_room(&room.id, "Store the cart in SQLite", &["Add cart table".into(), "".into(), "Migrate cookie carts".into()]).unwrap();
        assert_eq!(out.task_ids.len(), 2);
        assert_eq!(db.get_room(&room.id).unwrap().unwrap().state, RoomState::Decided);
        let d = db.list_decisions(None, None, 5).unwrap();
        assert!(d.iter().any(|d| d.agent == "human" && d.decision == "Store the cart in SQLite"));
        assert!(db.add_room_note(&room.id, "late").is_err() && db.decide_room(&room.id, "again", &[]).is_err());
        assert_eq!(db.session_label("human"), "You");
        assert!(db.session_label("abcdef123").starts_with("agent abcdef"));
    }

    #[test]
    fn room_post_truncation_is_utf8_safe() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "utf8 topic", "sess-a").unwrap();
        let mut body = "a".repeat(ROOM_BODY_BUDGET - 2);
        body.push_str("🦀🦀🦀🦀");
        let post = db
            .post_to_room(&room.id, "sess-a", PostKind::Propose, &body, &[ev])
            .unwrap();
        assert!(post.body.len() <= ROOM_BODY_BUDGET + 20);
        assert!(post.body.contains("…[truncated]"));
    }

    #[test]
    fn custom_protocol_replaces_turn_rules() {
        use crate::models::RoomState;
        struct ReviewOnly;
        impl crate::ports::RoomProtocol for ReviewOnly {
            fn supported_kinds(&self) -> Vec<PostKind> {
                vec![PostKind::Critique]
            }
            fn validate_turn(
                &self,
                _state: RoomState,
                kind: PostKind,
                _turn: Option<&str>,
                _author: &str,
                _empty: bool,
            ) -> anyhow::Result<()> {
                if kind == PostKind::Critique {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("review-only room"))
                }
            }
        }
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let room = db.create_room(None, "review topic", "sess-a").unwrap();
        // Default protocol would demand Propose here; the injected one
        // accepts Critique straight away.
        let post = db
            .post_to_room_with(
                &ReviewOnly,
                &room.id,
                "sess-b",
                PostKind::Critique,
                "lgtm - padded so it passes the minimum length",
                &[ev],
            )
            .unwrap();
        assert_eq!(post.kind, PostKind::Critique);
        assert!(db
            .post_to_room_with(&ReviewOnly, &room.id, "sess-b", PostKind::Propose, "x - padded so it passes the minimum length", &[])
            .is_err());
    }

    #[test]
    fn draft_overrides_plan_and_claiming_task_locks_scoped_files() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let id = decided_room(&db, &ev, "Use JWT in a cookie\n- Add token table\n- Write login route");

        let draft = db
            .save_room_draft(
                &id,
                "Use signed session cookies in SQLite",
                &[
                    PlanStep {
                        title: "Add session table".into(),
                        owner: Some("claude".into()),
                        files: vec!["src/db/sessions.rs".into(), "src/models/session.rs".into()],
                        check: Some("cargo test session".into()),
                    },
                    PlanStep {
                        title: "Add cookie middleware".into(),
                        owner: Some("agy".into()),
                        files: vec!["src/http/auth.rs".into()],
                        check: None,
                    },
                ],
            )
            .unwrap();
        assert_eq!(draft.steps.len(), 2);

        let out = db.approve_room(&id).unwrap();
        assert!(out.edited_by_human);
        assert_eq!(out.task_ids.len(), 2);

        let tasks = db.list_room_tasks(&id).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].assigned_agent.as_deref(), Some("claude"));
        assert_eq!(tasks[0].files, vec!["src/db/sessions.rs", "src/models/session.rs"]);
        assert_eq!(tasks[0].check_cmd.as_deref(), Some("cargo test session"));

        // An agent with a different identity skips the task assigned to claude and claims its own.
        db.sync_kv_set("session_agent:agy-sess", "agy");
        let agy_claimed = db.claim_next_task(None, "agy-sess").unwrap().unwrap();
        assert_eq!(agy_claimed.title, "Add cookie middleware");

        // Submitting records check_status when check_cmd exists (file locks are Tracewise's, not the task queue's).
        db.sync_kv_set("session_agent:cl-sess", "claude");
        let cl_claimed = db.claim_next_task(None, "cl-sess").unwrap().unwrap();
        assert_eq!(cl_claimed.title, "Add session table");
        db.submit_task(&cl_claimed.id, "cl-sess", Some("all tests passed")).unwrap();
        let after = db.get_task(&cl_claimed.id).unwrap().unwrap();
        assert_eq!(after.check_status.as_deref(), Some("passed"));
    }

    #[test]
    fn revise_room_supersedes_decision_and_cancels_only_unclaimed_tasks() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let r1 = decided_room(&db, &ev, "Plan v1\n- Task one\n- Task two");
        let out1 = db.approve_room(&r1).unwrap();
        assert_eq!(out1.task_ids.len(), 2);

        // Claim the first task; leave the second open.
        let claimed = db.claim_next_task(None, "worker-1").unwrap().unwrap();
        assert_eq!(claimed.title, "Task one");

        // Revise opens a follow-up room linked to r1 and seeded with r1's decision id as evidence.
        let follow = db.revise_room(&r1).unwrap();
        assert_eq!(follow.parent_room.as_deref(), Some(r1.as_str()));
        let meta = db.room_meta(&follow.id).unwrap();
        assert!(meta.evidence.contains(&out1.decision_id.unwrap()));

        // Decide the follow-up room.
        let out2 = db
            .decide_room_steps(
                &follow.id,
                "Plan v2",
                &[PlanStep {
                    title: "Replacement task".into(),
                    ..Default::default()
                }],
            )
            .unwrap();
        assert_eq!(out2.superseded_room_id.as_deref(), Some(r1.as_str()));
        assert_eq!(out2.cancelled_tasks, 1);
        assert_eq!(out2.flagged_claimed_tasks, vec!["Task one".to_string()]);
        assert_eq!(db.superseded_by_room(&r1).as_deref(), Some(follow.id.as_str()));

        let old_tasks = db.list_room_tasks(&r1).unwrap();
        assert!(!old_tasks[0].cancelled, "claimed task is kept and flagged, not silently cancelled");
        assert!(old_tasks[1].cancelled, "unclaimed task is cancelled");

        let active_decisions = db
            .list_decisions(None, Some(crate::models::DecisionStatus::Active), 10)
            .unwrap()
            .into_iter()
            .filter(|d| d.topic == "auth strategy")
            .collect::<Vec<_>>();
        assert_eq!(active_decisions.len(), 1);
        assert_eq!(active_decisions[0].decision, "Plan v2");
    }

    #[test]
    fn an_invited_agent_that_has_not_joined_yet_still_gets_its_critique() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let meta = RoomMeta {
            question: "How do we shard?".into(),
            invited: vec!["claude".into(), "codex".into(), "agy".into()],
            evidence: vec![ev.clone()],
            ..Default::default()
        };
        let room = db.create_room_by_human(None, "sharding", meta).unwrap();
        // Only two of the three invited agents have joined when the first critique lands (what happened in the live run).
        db.join_room(&room.id, "claude-1").unwrap();
        db.join_room(&room.id, "codex-1").unwrap();
        let evid = std::slice::from_ref(&ev);
        db.post_to_room(&room.id, "claude-1", PostKind::Propose, "Hash by user id - padded so it passes the minimum length", evid).unwrap();
        db.post_to_room(&room.id, "codex-1", PostKind::Critique, "Hot tenants will skew shards - padded so it passes", evid).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Critique, None), "the room waits for agy instead of moving to Revise");

        // agy arrives, critiques, and only then does the proposer revise.
        db.join_room(&room.id, "agy-1").unwrap();
        db.post_to_room(&room.id, "agy-1", PostKind::Critique, "Rebalancing needs a bucket key - padded so it passes", evid).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Revise, Some("claude-1")));
        assert_eq!(db.agent_label("claude-1"), "claude-1", "a session id that already starts with the agent name stays as it is");
        assert_eq!(db.agent_label("b3d66002-308f-49e7-bf5e-3d363d9e00c0"), "b3d66002-308f-49e7-bf5e-3d363d9e00c0", "unknown session: shown as is");
    }

    #[test]
    fn multi_critic_and_multi_round_rotation_and_skip_end() {
        let db = Database::open_in_memory().unwrap();
        let ev = seed_memory(&db);
        let meta = RoomMeta {
            question: "How do we shard?".into(),
            invited: vec!["claude".into(), "agy".into(), "codex".into()],
            evidence: vec![ev.clone()],
            rounds: Some(2),
            ..Default::default()
        };
        let room = db.create_room_by_human(None, "sharding", meta).unwrap();
        assert_eq!(room.rounds, 2);

        db.join_room(&room.id, "s-a").unwrap();
        db.join_room(&room.id, "s-b").unwrap();
        db.join_room(&room.id, "s-c").unwrap();

        // Round 1: s-a proposes, s-b critiques, s-c critiques, then s-a revises.
        db.post_to_room(&room.id, "s-a", PostKind::Propose, "Hash by user id - padded so it passes the minimum length", std::slice::from_ref(&ev)).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Critique, Some("s-b")));

        db.post_to_room(&room.id, "s-b", PostKind::Critique, "Hot tenants will skew shards - padded so it passes", std::slice::from_ref(&ev)).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Critique, Some("s-c")), "second critic gets a turn before Revise");

        db.post_to_room(&room.id, "s-c", PostKind::Critique, "Need rebalancing key - padded so it passes the minimum", std::slice::from_ref(&ev)).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Revise, Some("s-a")));

        db.post_to_room(&room.id, "s-a", PostKind::Revise, "Compound key (tenant, bucket) - padded so it passes", std::slice::from_ref(&ev)).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Critique, Some("s-b")), "round 2 starts automatically");

        // Skip s-b's turn in round 2 -> hands turn to s-c.
        let skipped = db.skip_room_turn(&room.id).unwrap();
        assert_eq!(skipped.turn_holder.as_deref(), Some("s-c"));
        db.post_to_room(&room.id, "s-c", PostKind::Critique, "Looks good with compound key - padded so it passes", std::slice::from_ref(&ev)).unwrap();
        // s-b still hasn't critiqued in round 2 unless we skip or post; let's have s-b post now.
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.turn_holder.as_deref(), Some("s-b"));
        db.post_to_room(&room.id, "s-b", PostKind::Critique, "Agreed on compound key - padded so it passes the minimum", std::slice::from_ref(&ev)).unwrap();

        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!((r.state, r.turn_holder.as_deref()), (RoomState::Revise, Some("s-a")));
        db.post_to_room(&room.id, "s-a", PostKind::Revise, "Use (tenant, bucket) sharding\n- Add shard router", std::slice::from_ref(&ev)).unwrap();

        let final_room = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(final_room.state, RoomState::Decided);

        // Ending an open room marks it decided with outcome "ended".
        let r_end = db.create_room(None, "abandoned", "s-a").unwrap();
        let ended = db.end_room(&r_end.id).unwrap();
        assert_eq!(ended.outcome, "ended");
        assert_eq!(db.get_room(&r_end.id).unwrap().unwrap().state, RoomState::Decided);
    }
}
