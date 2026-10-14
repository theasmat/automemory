use crate::db::Database;
use crate::models::{Review, ReviewStatus};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

/// Diffs over this size are truncated with a marker; reviewers see the head.
pub const REVIEW_DIFF_BUDGET: usize = 8000;

impl Database {
    pub fn submit_review(
        &self,
        project_id: Option<&str>,
        task_id: Option<&str>,
        author_session: &str,
        title: &str,
        diff: &str,
        test_command: Option<&str>,
    ) -> Result<Review> {
        let now = Utc::now();
        let mut capped = diff.to_string();
        if capped.len() > REVIEW_DIFF_BUDGET {
            let mut idx = REVIEW_DIFF_BUDGET;
            while !capped.is_char_boundary(idx) {
                idx = idx.saturating_sub(1);
            }
            capped.truncate(idx);
            capped.push_str("\n…[diff truncated]");
        }
        let review = Review {
            id: format!("rev-{}", Uuid::now_v7().simple()),
            project_id: project_id.map(str::to_string),
            task_id: task_id.map(str::to_string),
            author_session: author_session.to_string(),
            title: title.to_string(),
            diff: capped,
            test_command: test_command.map(str::to_string),
            status: ReviewStatus::Pending,
            reviewer_session: None,
            tests_passed: None,
            verdict: None,
            created_at: now,
            updated_at: now,
        };
        self.conn().execute(
            "INSERT INTO reviews (id, project_id, task_id, author_session, title, diff, test_command, status, reviewer_session, tests_passed, verdict, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', NULL, NULL, NULL, ?8, ?8)",
            params![
                review.id,
                review.project_id,
                review.task_id,
                review.author_session,
                review.title,
                review.diff,
                review.test_command,
                now.to_rfc3339(),
            ],
        )?;
        Ok(review)
    }

    /// Oldest pending review authored by a *different* session: agents review
    /// each other's diffs, never their own.
    pub fn next_review(
        &self,
        project_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Option<Review>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, task_id, author_session, title, diff, test_command, status, reviewer_session, tests_passed, verdict, created_at, updated_at
             FROM reviews
             WHERE status = 'pending'
               AND (?1 IS NULL OR project_id = ?1 OR project_id IS NULL)
               AND (?2 IS NULL OR author_session != ?2)
             ORDER BY created_at ASC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![project_id, session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_review(row)?))
        } else {
            Ok(None)
        }
    }

    /// Records a verdict. Tests are the judge: `tests_passed = false`
    /// rejects regardless of the reviewer's approval vote.
    pub fn post_review(
        &self,
        id: &str,
        reviewer_session: &str,
        approved: bool,
        tests_passed: bool,
        verdict: Option<&str>,
    ) -> Result<Option<Review>> {
        let status = if approved && tests_passed {
            "approved"
        } else {
            "rejected"
        };
        let now = Utc::now().to_rfc3339();
        let tests_int = i32::from(tests_passed);
        let rows = self.conn().execute(
            "UPDATE reviews SET status = ?1, reviewer_session = ?2, tests_passed = ?3, verdict = ?4, updated_at = ?5
             WHERE id = ?6 AND status = 'pending' AND author_session != ?2",
            params![status, reviewer_session, tests_int, verdict, now, id],
        )?;
        if rows == 0 {
            return Ok(None);
        }
        self.get_review(id)
    }

    pub fn get_review(&self, id: &str) -> Result<Option<Review>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, task_id, author_session, title, diff, test_command, status, reviewer_session, tests_passed, verdict, created_at, updated_at
             FROM reviews WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_review(row)?))
        } else {
            Ok(None)
        }
    }

    fn row_to_review(row: &rusqlite::Row) -> rusqlite::Result<Review> {
        let status_str: String = row.get(7)?;
        let tests_int: Option<i32> = row.get(9)?;
        let created_str: String = row.get(11)?;
        let updated_str: String = row.get(12)?;
        Ok(Review {
            id: row.get(0)?,
            project_id: row.get(1)?,
            task_id: row.get(2)?,
            author_session: row.get(3)?,
            title: row.get(4)?,
            diff: row.get(5)?,
            test_command: row.get(6)?,
            status: ReviewStatus::from_str(&status_str).unwrap_or(ReviewStatus::Pending),
            reviewer_session: row.get(8)?,
            tests_passed: tests_int.map(|v| v != 0),
            verdict: row.get(10)?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submit(db: &Database, author: &str, title: &str) -> Review {
        db.submit_review(
            None,
            None,
            author,
            title,
            "diff --git a/x",
            Some("cargo test"),
        )
        .unwrap()
    }

    #[test]
    fn next_skips_own_diff_and_tests_judge() {
        let db = Database::open_in_memory().unwrap();
        submit(&db, "sess-a", "own work");

        // Author never reviews their own diff.
        assert!(db.next_review(None, Some("sess-a")).unwrap().is_none());

        // A different agent gets it, approves, tests green → approved.
        let job = db.next_review(None, Some("sess-b")).unwrap().expect("job");
        let done = db
            .post_review(&job.id, "sess-b", true, true, Some("clean"))
            .unwrap()
            .expect("posted");
        assert_eq!(done.status, ReviewStatus::Approved);

        // Approval with red tests → rejected: tests are the judge.
        submit(&db, "sess-c", "risky change");
        let job2 = db.next_review(None, Some("sess-b")).unwrap().expect("job2");
        let rejected = db
            .post_review(&job2.id, "sess-b", true, false, Some("2 failures"))
            .unwrap()
            .expect("posted");
        assert_eq!(rejected.status, ReviewStatus::Rejected);

        // Decided reviews leave the queue; posting twice fails.
        assert!(db.next_review(None, Some("sess-b")).unwrap().is_none());
        assert!(db
            .post_review(&job2.id, "sess-d", true, true, None)
            .unwrap()
            .is_none());
    }

    #[test]
    fn diff_truncation_is_utf8_safe() {
        let db = Database::open_in_memory().unwrap();
        let mut diff = "a".repeat(REVIEW_DIFF_BUDGET - 2);
        diff.push_str("🦀🦀🦀🦀");
        let review = db
            .submit_review(None, None, "author", "utf8 test", &diff, None)
            .unwrap();
        assert!(review.diff.len() <= REVIEW_DIFF_BUDGET + 25);
        assert!(review.diff.contains("…[diff truncated]"));
    }
}
