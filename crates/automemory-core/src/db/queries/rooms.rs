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

impl Database {
    pub fn create_room(
        &self,
        project_id: Option<&str>,
        topic: &str,
        creator_session: &str,
    ) -> Result<Room> {
        let now = Utc::now();
        let room = Room {
            id: format!("room-{}", Uuid::now_v7().simple()),
            project_id: project_id.map(str::to_string),
            topic: topic.to_string(),
            state: RoomState::Open,
            members: vec![creator_session.to_string()],
            turn_holder: Some(creator_session.to_string()),
            round: 0,
            created_at: now,
            updated_at: now,
        };
        self.conn().execute(
            "INSERT INTO rooms (id, project_id, topic, state, members, turn_holder, round, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'open', ?4, ?5, 0, ?6, ?6)",
            params![
                room.id,
                room.project_id,
                room.topic,
                serde_json::to_string(&room.members)?,
                room.turn_holder,
                now.to_rfc3339(),
            ],
        )?;
        Ok(room)
    }

    pub fn get_room(&self, id: &str) -> Result<Option<Room>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, topic, state, members, turn_holder, round, created_at, updated_at
             FROM rooms WHERE id = ?1",
        )?;
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
            // An unheld turn, or one an agent holds only because it was alone after its own post, goes to the newcomer.
            let last_author = self.room_posts(id)?.last().map(|p| p.author_session.clone());
            if room.turn_holder.is_none() || (room.turn_holder.is_some() && room.turn_holder == last_author) {
                room.turn_holder = Some(session_id.to_string());
            }
            room.updated_at = Utc::now();
            self.conn().execute(
                "UPDATE rooms SET members = ?1, turn_holder = ?2, updated_at = ?3 WHERE id = ?4",
                params![
                    serde_json::to_string(&room.members)?,
                    room.turn_holder,
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
        let mut stmt = conn.prepare(
            "SELECT id, project_id, topic, state, members, turn_holder, round, created_at, updated_at
             FROM rooms
             WHERE state != 'decided'
               AND (?1 IS NULL OR project_id = ?1 OR project_id IS NULL)
               AND (?2 IS NULL OR turn_holder IS NULL OR turn_holder = ?2)
             ORDER BY updated_at DESC
             LIMIT 20",
        )?;
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
        let room = self
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
        let previous = self.room_posts(room_id)?.into_iter().rev().find(|p| p.kind != PostKind::Note);
        if kind == PostKind::Critique && previous.as_ref().is_some_and(|p| p.author_session == author_session) {
            return Err(anyhow!("a critique must come from another agent: wait for one to join"));
        }
        for id in evidence {
            if !self.evidence_resolves(id)? {
                return Err(anyhow!("evidence does not resolve: {id}"));
            }
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
        self.conn().execute(
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

        let next_state = match room.state {
            RoomState::Open => RoomState::Critique,
            RoomState::Critique => RoomState::Revise,
            RoomState::Revise | RoomState::Decided => RoomState::Decided,
        };
        // The next turn goes to another member; alone, it stays open for whoever joins (nobody critiques their own post).
        let next_turn = if next_state == RoomState::Decided {
            None
        } else {
            Some(next_member(&room.members, author_session)).filter(|n| n != author_session)
        };
        self.conn().execute(
            "UPDATE rooms SET state = ?1, turn_holder = ?2, round = round + 1, updated_at = ?3 WHERE id = ?4",
            params![next_state.to_string(), next_turn, now.to_rfc3339(), room_id],
        )?;
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
        if self.get_memory_by_id(id)?.is_some() {
            return Ok(true);
        }
        if self.get_trace_by_id(id)?.is_some() {
            return Ok(true);
        }
        if self.get_failure_by_id(id)?.is_some() {
            return Ok(true);
        }
        Ok(false)
    }

    fn row_to_room(row: &rusqlite::Row) -> rusqlite::Result<Room> {
        let members_str: String = row.get(4)?;
        let state_str: String = row.get(3)?;
        let created_str: String = row.get(7)?;
        let updated_str: String = row.get(8)?;
        Ok(Room {
            id: row.get(0)?,
            project_id: row.get(1)?,
            topic: row.get(2)?,
            state: RoomState::from_str(&state_str).unwrap_or(RoomState::Open),
            members: serde_json::from_str(&members_str).unwrap_or_default(),
            turn_holder: row.get(5)?,
            round: row.get(6)?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

/// What approving a decided room would store: the decision and the tasks.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RoomPlan {
    pub topic: String,
    pub decision: String,
    pub reason: String,
    pub dissent: Option<String>,
    pub tasks: Vec<String>,
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
}

/// The human's verdict on a decided room, kept so approving twice is impossible.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomOutcome {
    pub outcome: String, // "approved" | "rejected"
    pub decision_id: Option<String>,
    pub task_ids: Vec<String>,
    pub at: String,
}

fn clip(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max { t.to_string() } else { format!("{}…", t.chars().take(max).collect::<String>().trim_end()) }
}

/// Steps of a plan: bullet or numbered lines, at most 10.
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

impl Database {
    /// Every room, newest first, with how many posts it holds.
    pub fn list_rooms(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<(Room, i64)>> {
        let rooms: Vec<Room> = {
            let conn = self.conn();
            let mut stmt = conn.prepare(
                "SELECT id, project_id, topic, state, members, turn_holder, round, created_at, updated_at FROM rooms
                 WHERE (?1 IS NULL OR project_id = ?1 OR project_id IS NULL) ORDER BY updated_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![project_id, limit as i64], Self::row_to_room)?.collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        Ok(rooms.into_iter().map(|r| { let n = self.room_posts(&r.id).map(|p| p.len() as i64).unwrap_or(0); (r, n) }).collect())
    }

    pub fn room_meta(&self, room_id: &str) -> Option<RoomMeta> {
        self.sync_kv_get(&format!("room_meta:{room_id}")).and_then(|j| serde_json::from_str(&j).ok())
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
        meta.question = clip(&meta.question, 400);
        meta.created_by = "human".into();
        let now = Utc::now();
        let id = format!("room-{}", Uuid::now_v7().simple());
        self.conn().execute(
            "INSERT INTO rooms (id, project_id, topic, state, members, turn_holder, round, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'open', '[]', NULL, 0, ?4, ?4)",
            params![id, project_id, topic, now.to_rfc3339()],
        )?;
        self.sync_kv_set(&format!("room_meta:{id}"), &serde_json::to_string(&meta)?);
        self.get_room(&id)?.ok_or_else(|| anyhow!("room vanished"))
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
        Ok(RoomPlan {
            topic: room.topic,
            decision,
            reason: format!("Agreed in planning room {room_id}: {}", clip(&revise.body, 600)),
            dissent: critique.map(|c| clip(&c.body, 400)),
            tasks: extract_steps(&revise.body),
            evidence,
        })
    }

    /// Human approval of the agents' plan as written.
    pub fn approve_room(&self, room_id: &str) -> Result<RoomOutcome> {
        let plan = self.room_plan(room_id)?;
        self.store_plan(room_id, plan, "room")
    }

    /// The human decides, at any stage: the decision text and steps are theirs (prefilled from the agents' plan
    /// when there is one). Ends the discussion.
    pub fn decide_room(&self, room_id: &str, decision: &str, steps: &[String]) -> Result<RoomOutcome> {
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        if decision.trim().is_empty() {
            return Err(anyhow!("the decision is empty"));
        }
        let agents = self.room_plan(room_id).ok();
        let steps: Vec<String> = steps.iter().map(|t| clip(t, 120)).filter(|t| !t.is_empty()).take(20).collect();
        let unchanged = agents.as_ref().is_some_and(|p| p.decision == decision.trim() && p.tasks == steps);
        let posts = self.room_posts(room_id)?;
        let mut evidence: Vec<String> = posts.iter().flat_map(|p| p.evidence.clone()).collect();
        evidence.sort();
        evidence.dedup();
        let plan = RoomPlan {
            topic: room.topic.clone(),
            decision: clip(decision, 200),
            reason: match &agents {
                Some(p) if unchanged => p.reason.clone(),
                Some(p) => format!("Decided by the human in planning room {room_id}, starting from the agents' plan: {}", clip(&p.decision, 300)),
                None => format!("Decided by the human in planning room {room_id} after {} post(s).", posts.len()),
            },
            dissent: posts.iter().rev().find(|p| p.kind == PostKind::Critique).map(|c| clip(&c.body, 400)),
            tasks: steps,
            evidence,
        };
        let out = self.store_plan(room_id, plan, if unchanged { "room" } else { "human" })?;
        self.conn().execute(
            "UPDATE rooms SET state = 'decided', turn_holder = NULL, updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), room_id],
        )?;
        Ok(out)
    }

    fn store_plan(&self, room_id: &str, plan: RoomPlan, agent: &str) -> Result<RoomOutcome> {
        if self.room_outcome(room_id).is_some() {
            return Err(anyhow!("this room already has an outcome"));
        }
        let room = self.get_room(room_id)?.ok_or_else(|| anyhow!("room not found"))?;
        let decision = self.record_decision(crate::models::DecisionRecordArgs {
            project_id: room.project_id.as_deref(),
            topic: &plan.topic,
            decision: &plan.decision,
            reason: &plan.reason,
            alternatives: &[],
            dissent: plan.dissent.as_deref(),
            agent,
            symbols: &[],
            evidence: &plan.evidence,
        })?;
        let mut task_ids = Vec::new();
        for step in &plan.tasks {
            task_ids.push(self.create_task(&format!("room:{room_id}"), room.project_id.as_deref(), step, &format!("From the planning room \"{}\": {}", plan.topic, plan.decision), &[])?.id);
        }
        let out = RoomOutcome { outcome: "approved".into(), decision_id: Some(decision.id), task_ids, at: Utc::now().to_rfc3339() };
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
        let out = RoomOutcome { outcome: "rejected".into(), decision_id: None, task_ids: vec![], at: Utc::now().to_rfc3339() };
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
        self.conn().execute(
            "UPDATE rooms SET state = 'critique', turn_holder = ?1, updated_at = ?2 WHERE id = ?3",
            params![turn, Utc::now().to_rfc3339(), room_id],
        )?;
        Ok(self.get_room(room_id)?.ok_or_else(|| anyhow!("room vanished"))?)
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
        db.post_to_room(&r2, "sess-b", PostKind::Critique, "Still unclear - padded so it passes the minimum length", &[ev.clone()]).unwrap();
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
        db.post_to_room(&room.id, "c1", PostKind::Revise, "SQLite with expiry\n- add table", &[ev.clone()]).unwrap();
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
}
