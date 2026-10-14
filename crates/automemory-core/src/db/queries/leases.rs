use crate::db::Database;
use crate::models::{Lease, LeaseAcquireArgs, LeaseAcquireResult, LeaseConflict, LeaseScopeType};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use uuid::Uuid;

impl Database {
    /// Acquires a new lease or renews an existing lease held by the same session.
    /// If another session holds an active unexpired lease on an overlapping scope, returns a conflict.
    pub fn acquire_lease(&self, args: LeaseAcquireArgs<'_>) -> Result<LeaseAcquireResult> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let conn = self.conn();

        // 1. Purge expired leases
        let _ = conn.execute("DELETE FROM leases WHERE expires_at < ?1", params![now_str]);

        // 2. Fetch all active leases in the project (or global)
        let mut stmt = conn.prepare(
            "SELECT id, project_id, agent, session_id, scope, scope_type, goal, ttl_seconds, acquired_at, heartbeat_at, expires_at
             FROM leases
             WHERE (project_id IS ?1 OR project_id IS NULL OR ?1 IS NULL)",
        )?;

        let active_leases = stmt
            .query_map(params![args.project_id], |row| {
                let acquired_str: String = row.get(8)?;
                let heartbeat_str: String = row.get(9)?;
                let expires_str: String = row.get(10)?;
                let scope_type_str: String = row.get(5)?;

                Ok(Lease {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    agent: row.get(2)?,
                    session_id: row.get(3)?,
                    scope: row.get(4)?,
                    scope_type: LeaseScopeType::from(scope_type_str.as_str()),
                    goal: row.get(6)?,
                    ttl_seconds: row.get(7)?,
                    acquired_at: DateTime::parse_from_rfc3339(&acquired_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                    heartbeat_at: DateTime::parse_from_rfc3339(&heartbeat_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                    expires_at: DateTime::parse_from_rfc3339(&expires_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // 3. Check for conflict or existing lease for same session
        for existing in active_leases {
            if scopes_conflict(
                &existing.scope,
                &existing.scope_type,
                args.scope,
                &args.scope_type,
            ) {
                if existing.session_id == args.session_id {
                    // Same session: renew the lease
                    let new_expires = now + Duration::seconds(args.ttl_seconds);
                    let new_expires_str = new_expires.to_rfc3339();

                    conn.execute(
                        "UPDATE leases
                         SET goal = ?1,
                             ttl_seconds = ?2,
                             heartbeat_at = ?3,
                             expires_at = ?4
                         WHERE id = ?5",
                        params![
                            args.goal,
                            args.ttl_seconds,
                            now_str,
                            new_expires_str,
                            existing.id
                        ],
                    )
                    .context("Failed to renew lease")?;

                    let mut renewed = existing;
                    renewed.goal = args.goal.to_string();
                    renewed.ttl_seconds = args.ttl_seconds;
                    renewed.heartbeat_at = now;
                    renewed.expires_at = new_expires;
                    return Ok(LeaseAcquireResult::Renewed { lease: renewed });
                } else {
                    // Different session: collision detected!
                    let remaining = (existing.expires_at - now).num_seconds().max(1);
                    return Ok(LeaseAcquireResult::Conflict {
                        conflict: LeaseConflict {
                            scope: existing.scope,
                            holder_agent: existing.agent,
                            holder_session_id: existing.session_id,
                            goal: existing.goal,
                            remaining_seconds: remaining,
                            expires_at: existing.expires_at,
                        },
                    });
                }
            }
        }

        // 4. No conflict: create new lease
        let id = format!("lease-{}", Uuid::new_v4().simple());
        let expires = now + Duration::seconds(args.ttl_seconds);
        let expires_str = expires.to_rfc3339();

        conn.execute(
            "INSERT INTO leases (
                id, project_id, agent, session_id, scope, scope_type, goal, ttl_seconds,
                acquired_at, heartbeat_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                args.project_id,
                args.agent,
                args.session_id,
                args.scope,
                args.scope_type.to_string(),
                args.goal,
                args.ttl_seconds,
                now_str,
                now_str,
                expires_str,
            ],
        )
        .context("Failed to insert new lease")?;

        let lease = Lease {
            id,
            project_id: args.project_id.map(ToString::to_string),
            agent: args.agent.to_string(),
            session_id: args.session_id.to_string(),
            scope: args.scope.to_string(),
            scope_type: args.scope_type,
            goal: args.goal.to_string(),
            ttl_seconds: args.ttl_seconds,
            acquired_at: now,
            heartbeat_at: now,
            expires_at: expires,
        };

        Ok(LeaseAcquireResult::Acquired { lease })
    }

    /// Checks if any target paths or symbols collide with an active lease held by another session.
    pub fn check_collision(
        &self,
        project_id: Option<&str>,
        session_id: &str,
        paths: &[String],
        symbols: &[String],
    ) -> Result<Option<LeaseConflict>> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let conn = self.conn();

        // Purge expired leases
        let _ = conn.execute("DELETE FROM leases WHERE expires_at < ?1", params![now_str]);

        let mut stmt = conn.prepare(
            "SELECT id, project_id, agent, session_id, scope, scope_type, goal, ttl_seconds, acquired_at, heartbeat_at, expires_at
             FROM leases
             WHERE (project_id IS ?1 OR project_id IS NULL OR ?1 IS NULL) AND session_id != ?2",
        )?;

        let other_leases = stmt
            .query_map(params![project_id, session_id], |row| {
                let expires_str: String = row.get(10)?;
                let scope_type_str: String = row.get(5)?;
                Ok((
                    row.get::<_, String>(2)?, // agent
                    row.get::<_, String>(3)?, // session_id
                    row.get::<_, String>(4)?, // scope
                    LeaseScopeType::from(scope_type_str.as_str()),
                    row.get::<_, String>(6)?, // goal
                    DateTime::parse_from_rfc3339(&expires_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (agent, s_id, scope, scope_type, goal, expires_at) in other_leases {
            if scope_type == LeaseScopeType::Symbol {
                for sym in symbols {
                    if sym == &scope {
                        let remaining = (expires_at - now).num_seconds().max(1);
                        return Ok(Some(LeaseConflict {
                            scope,
                            holder_agent: agent,
                            holder_session_id: s_id,
                            goal,
                            remaining_seconds: remaining,
                            expires_at,
                        }));
                    }
                }
            } else {
                for path in paths {
                    if scope_matches(&scope, &scope_type, path) {
                        let remaining = (expires_at - now).num_seconds().max(1);
                        return Ok(Some(LeaseConflict {
                            scope,
                            holder_agent: agent,
                            holder_session_id: s_id,
                            goal,
                            remaining_seconds: remaining,
                            expires_at,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Releases leases for a session. If scope is specified, releases only that scope.
    pub fn release_leases(&self, session_id: &str, scope: Option<&str>) -> Result<usize> {
        let conn = self.conn();
        let count = match scope {
            Some(s) => conn.execute(
                "DELETE FROM leases WHERE session_id = ?1 AND scope = ?2",
                params![session_id, s],
            )?,
            None => conn.execute(
                "DELETE FROM leases WHERE session_id = ?1",
                params![session_id],
            )?,
        };
        Ok(count)
    }

    /// Unlocks a scope regardless of session holder (CLI / Admin override).
    pub fn unlock_scope(&self, project_id: Option<&str>, scope: &str) -> Result<usize> {
        let conn = self.conn();
        let count = conn.execute(
            "DELETE FROM leases WHERE (project_id IS ?1 OR project_id IS NULL OR ?1 IS NULL) AND scope = ?2",
            params![project_id, scope],
        )?;
        Ok(count)
    }

    /// Kill-switch: releases every active lease regardless of holder.
    pub fn release_all_leases(&self, project_id: Option<&str>) -> Result<usize> {
        let count = self.conn().execute(
            "DELETE FROM leases WHERE (?1 IS NULL OR project_id = ?1 OR project_id IS NULL)",
            params![project_id],
        )?;
        Ok(count)
    }

    /// Heartbeats active leases held by a session, extending their TTL.
    pub fn heartbeat_lease(&self, session_id: &str, scope: Option<&str>) -> Result<usize> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let conn = self.conn();

        let count = match scope {
            Some(s) => conn.execute(
                "UPDATE leases
                 SET heartbeat_at = ?1,
                     expires_at = datetime('now', printf('+%d seconds', ttl_seconds))
                 WHERE session_id = ?2 AND scope = ?3",
                params![now_str, session_id, s],
            )?,
            None => conn.execute(
                "UPDATE leases
                 SET heartbeat_at = ?1,
                     expires_at = datetime('now', printf('+%d seconds', ttl_seconds))
                 WHERE session_id = ?2",
                params![now_str, session_id],
            )?,
        };
        Ok(count)
    }

    /// Lists all currently active, unexpired leases.
    pub fn list_active_leases(&self, project_id: Option<&str>) -> Result<Vec<Lease>> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let conn = self.conn();

        // Purge expired leases first
        let _ = conn.execute("DELETE FROM leases WHERE expires_at < ?1", params![now_str]);

        let mut stmt = conn.prepare(
            "SELECT id, project_id, agent, session_id, scope, scope_type, goal, ttl_seconds, acquired_at, heartbeat_at, expires_at
             FROM leases
             WHERE (?1 IS NULL OR project_id = ?1)
             ORDER BY acquired_at DESC",
        )?;

        let leases = stmt
            .query_map(params![project_id], |row| {
                let acquired_str: String = row.get(8)?;
                let heartbeat_str: String = row.get(9)?;
                let expires_str: String = row.get(10)?;
                let scope_type_str: String = row.get(5)?;

                Ok(Lease {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    agent: row.get(2)?,
                    session_id: row.get(3)?,
                    scope: row.get(4)?,
                    scope_type: LeaseScopeType::from(scope_type_str.as_str()),
                    goal: row.get(6)?,
                    ttl_seconds: row.get(7)?,
                    acquired_at: DateTime::parse_from_rfc3339(&acquired_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                    heartbeat_at: DateTime::parse_from_rfc3339(&heartbeat_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                    expires_at: DateTime::parse_from_rfc3339(&expires_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or(now),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(leases)
    }
}

/// Determines whether a lease scope matches a given target path or symbol.
fn scope_matches(scope: &str, scope_type: &LeaseScopeType, target: &str) -> bool {
    let clean_target = target.trim_start_matches("./");
    let clean_scope = scope.trim_start_matches("./");

    match scope_type {
        LeaseScopeType::File => {
            if clean_scope == clean_target {
                return true;
            }
            if clean_target.ends_with(clean_scope) {
                let prefix_len = clean_target.len() - clean_scope.len();
                if prefix_len == 0 || clean_target.as_bytes()[prefix_len - 1] == b'/' {
                    return true;
                }
            }
            if clean_scope.ends_with(clean_target) {
                let prefix_len = clean_scope.len() - clean_target.len();
                if prefix_len == 0 || clean_scope.as_bytes()[prefix_len - 1] == b'/' {
                    return true;
                }
            }
            false
        }
        LeaseScopeType::Glob => {
            if let Ok(pattern) = glob::Pattern::new(clean_scope) {
                if pattern.matches(clean_target) {
                    return true;
                }
                if let Some(pos) = clean_target.rfind('/') {
                    if pattern.matches(&clean_target[pos + 1..]) {
                        return true;
                    }
                }
            }
            false
        }
        LeaseScopeType::Symbol => clean_scope == clean_target,
    }
}

/// Checks if two scopes potentially overlap or conflict.
fn scopes_conflict(
    scope_a: &str,
    type_a: &LeaseScopeType,
    scope_b: &str,
    type_b: &LeaseScopeType,
) -> bool {
    if type_a == &LeaseScopeType::Symbol || type_b == &LeaseScopeType::Symbol {
        return type_a == type_b && scope_a == scope_b;
    }
    scope_matches(scope_a, type_a, scope_b) || scope_matches(scope_b, type_b, scope_a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_matching() {
        assert!(scope_matches(
            "src/auth/session.ts",
            &LeaseScopeType::File,
            "src/auth/session.ts"
        ));
        assert!(scope_matches(
            "src/auth/session.ts",
            &LeaseScopeType::File,
            "./src/auth/session.ts"
        ));
        assert!(scope_matches(
            "src/auth/session.ts",
            &LeaseScopeType::File,
            "/home/user/project/src/auth/session.ts"
        ));
        assert!(!scope_matches(
            "src/auth/session.ts",
            &LeaseScopeType::File,
            "src/auth/token.ts"
        ));

        // Glob matching
        assert!(scope_matches(
            "src/**/*.rs",
            &LeaseScopeType::Glob,
            "src/models/lease.rs"
        ));
        assert!(scope_matches(
            "*.rs",
            &LeaseScopeType::Glob,
            "src/models/lease.rs"
        ));
        assert!(!scope_matches(
            "src/**/*.ts",
            &LeaseScopeType::Glob,
            "src/models/lease.rs"
        ));

        // Symbol matching
        assert!(scope_matches(
            "auth::create_session",
            &LeaseScopeType::Symbol,
            "auth::create_session"
        ));
        assert!(!scope_matches(
            "auth::create_session",
            &LeaseScopeType::Symbol,
            "auth::destroy_session"
        ));
    }

    #[test]
    fn test_acquire_and_conflict() {
        let db = Database::open_in_memory().expect("in-memory db open failed");

        // Agent 1 acquires lease
        let res1 = db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("proj-1"),
            agent: "agent-alpha",
            session_id: "session-1",
            scope: "src/auth/session.ts",
            scope_type: LeaseScopeType::File,
            goal: "Refactoring JWT authentication",
            ttl_seconds: 300,
        });
        assert!(matches!(res1.unwrap(), LeaseAcquireResult::Acquired { .. }));

        // Agent 1 renews same lease
        let res_renew = db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("proj-1"),
            agent: "agent-alpha",
            session_id: "session-1",
            scope: "src/auth/session.ts",
            scope_type: LeaseScopeType::File,
            goal: "Refactoring JWT authentication part 2",
            ttl_seconds: 300,
        });
        assert!(matches!(
            res_renew.unwrap(),
            LeaseAcquireResult::Renewed { .. }
        ));

        // Agent 2 attempts to acquire same file -> Conflict
        let res2 = db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("proj-1"),
            agent: "agent-beta",
            session_id: "session-2",
            scope: "src/auth/session.ts",
            scope_type: LeaseScopeType::File,
            goal: "Fixing token expiry bug",
            ttl_seconds: 300,
        });
        match res2.unwrap() {
            LeaseAcquireResult::Conflict { conflict } => {
                assert_eq!(conflict.holder_agent, "agent-alpha");
                assert_eq!(conflict.holder_session_id, "session-1");
            }
            _ => panic!("Expected conflict"),
        }

        // Collision check
        let collision = db
            .check_collision(
                Some("proj-1"),
                "session-2",
                &["src/auth/session.ts".to_string()],
                &[],
            )
            .unwrap();
        assert!(collision.is_some());
        assert_eq!(collision.unwrap().holder_agent, "agent-alpha");

        // Collision check for session-1 (holder) returns None
        let holder_collision = db
            .check_collision(
                Some("proj-1"),
                "session-1",
                &["src/auth/session.ts".to_string()],
                &[],
            )
            .unwrap();
        assert!(holder_collision.is_none());

        // Agent 1 releases lease
        let released = db.release_leases("session-1", None).unwrap();
        assert_eq!(released, 1);

        // Now Agent 2 can acquire
        let res3 = db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("proj-1"),
            agent: "agent-beta",
            session_id: "session-2",
            scope: "src/auth/session.ts",
            scope_type: LeaseScopeType::File,
            goal: "Fixing token expiry bug",
            ttl_seconds: 300,
        });
        assert!(matches!(res3.unwrap(), LeaseAcquireResult::Acquired { .. }));
    }
}
