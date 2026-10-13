use crate::db::Database;
use crate::models::{PostKind, Room, RoomPost, RoomState};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

/// Rationale stays short: stored posts are capped, never raw reasoning.
pub const ROOM_BODY_BUDGET: usize = 2000;

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
            if room.turn_holder.is_none() {
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
        let room = self
            .get_room(room_id)?
            .ok_or_else(|| anyhow!("room not found"))?;
        let expected = PostKind::expected_in(room.state)
            .ok_or_else(|| anyhow!("room is decided"))?;
        if kind != expected {
            return Err(anyhow!(
                "room expects {expected}, not {kind}"
            ));
        }
        if room.turn_holder.as_deref() != Some(author_session) {
            return Err(anyhow!("not your turn"));
        }
        if kind == PostKind::Propose && evidence.is_empty() {
            return Err(anyhow!("propose requires evidence"));
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
        let next_turn = if next_state == RoomState::Decided {
            None
        } else {
            Some(next_member(&room.members, author_session))
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
                    kind: PostKind::from_str(&kind_str)
                        .unwrap_or(PostKind::Propose),
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
        assert!(db.post_to_room(&room.id, "sess-b", PostKind::Propose, "x", std::slice::from_ref(&ev)).is_err());
        assert!(db.post_to_room(&room.id, "sess-a", PostKind::Critique, "x", std::slice::from_ref(&ev)).is_err());
        // Dangling evidence is refused.
        assert!(db.post_to_room(&room.id, "sess-a", PostKind::Propose, "x", &["missing".to_string()]).is_err());
        // Propose without evidence is refused.
        assert!(db.post_to_room(&room.id, "sess-a", PostKind::Propose, "x", &[]).is_err());

        db.post_to_room(&room.id, "sess-a", PostKind::Propose, "JWT plan", std::slice::from_ref(&ev)).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.state, RoomState::Critique);
        assert_eq!(r.turn_holder.as_deref(), Some("sess-b"));

        db.post_to_room(&room.id, "sess-b", PostKind::Critique, "rotation?", std::slice::from_ref(&ev)).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.state, RoomState::Revise);
        assert_eq!(r.turn_holder.as_deref(), Some("sess-a"));

        db.post_to_room(&room.id, "sess-a", PostKind::Revise, "JWT + rotation", &[ev]).unwrap();
        let r = db.get_room(&room.id).unwrap().unwrap();
        assert_eq!(r.state, RoomState::Decided);
        assert_eq!(r.turn_holder, None);

        // Decided rooms reject posts and joins.
        assert!(db.post_to_room(&room.id, "sess-b", PostKind::Critique, "late", &[]).is_err());
        assert!(db.join_room(&room.id, "sess-c").is_err());
        assert_eq!(db.room_posts(&room.id).unwrap().len(), 3);
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

        db.post_to_room(&r2.id, "sess-b", PostKind::Propose, "p", &[ev]).unwrap();
        // r2 now waits on sess-b itself (solo rotation); sess-a sees only r1.
        let mine = db.pending_rooms(None, Some("sess-a")).unwrap();
        assert_eq!(mine.len(), 1);
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
}
