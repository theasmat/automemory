use crate::db::Database;
use crate::models::Baton;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn save_baton(&self, baton: &Baton) -> Result<()> {
        self.conn().execute(
            "INSERT INTO batons (id, project_id, from_session, to_session, agent, packet, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                baton.id,
                baton.project_id,
                baton.from_session,
                baton.to_session,
                baton.agent,
                baton.packet,
                baton.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Latest baton visible to `session_id`: addressed to it or broadcast.
    pub fn latest_baton(
        &self,
        project_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Option<Baton>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, from_session, to_session, agent, packet, created_at
             FROM batons
             WHERE (project_id IS ?1 OR ?1 IS NULL)
               AND (?2 IS NULL OR to_session IS NULL OR to_session = ?2)
             ORDER BY (CASE WHEN ?2 IS NOT NULL AND to_session = ?2 THEN 0 ELSE 1 END) ASC, created_at DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![project_id, session_id])?;
        if let Some(row) = rows.next()? {
            let created_str: String = row.get(6)?;
            Ok(Some(Baton {
                id: row.get(0)?,
                project_id: row.get(1)?,
                from_session: row.get(2)?,
                to_session: row.get(3)?,
                agent: row.get(4)?,
                packet: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baton::pass_baton;

    #[test]
    fn pass_then_pickup_prefers_direct_over_broadcast() {
        let db = Database::open_in_memory().unwrap();
        // Direct baton created first, broadcast created second (newer).
        // Direct must STILL be preferred for sess-b.
        pass_baton(&db, None, "sess-a", Some("sess-b"), "agent-a", Some("your turn")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        pass_baton(&db, None, "sess-a", None, "agent-a", Some("general note")).unwrap();

        let for_b = latest_baton_helper(&db, None, Some("sess-b"));
        assert!(for_b.packet.contains("your turn"));

        let for_c = latest_baton_helper(&db, None, Some("sess-c"));
        assert!(for_c.packet.contains("general note"));
    }

    fn latest_baton_helper(db: &Database, project: Option<&str>, session: Option<&str>) -> Baton {
        db.latest_baton(project, session).unwrap().expect("baton")
    }
}
