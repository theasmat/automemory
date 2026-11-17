//! Stable project identity across machines.
//!
//! A local `project_id` is a random UUID per machine, so the same repository on a second PC is a
//! different project. The *project key* is derived from the repository itself: the normalized
//! `origin` URL, or the root commit when there is no remote. Synced records carry the key and
//! are mapped to the local project on arrival.

use crate::db::Database;
use crate::models::Project;
use crate::sync::GitSync;
use anyhow::Result;
use rusqlite::params;
use std::path::Path;
use std::process::Command;

/// `git@github.com:Owner/Repo.git`, `https://user:tok@github.com/owner/repo/` and
/// `ssh://git@github.com:22/owner/repo.git` all become `git:github.com/owner/repo`.
pub fn normalize_remote(url: &str) -> Option<String> {
    let mut u = url.trim().to_string();
    if u.is_empty() {
        return None;
    }
    for scheme in ["https://", "http://", "ssh://", "git://", "git+ssh://"] {
        if let Some(rest) = u.strip_prefix(scheme) {
            u = rest.to_string();
            break;
        }
    }
    if let Some((_, rest)) = u.split_once('@') {
        u = rest.to_string(); // user[:token]@
    }
    // scp-like `host:owner/repo` and ssh `host:22/owner/repo`
    if let Some((host, path)) = u.split_once(':') {
        let path = path.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start_matches('/');
        u = format!("{host}/{path}");
    }
    let u = u.trim_end_matches('/').trim_end_matches(".git").to_lowercase();
    (u.contains('/')).then(|| format!("git:{u}"))
}

/// The key for the repository containing `dir`: `git:<host>/<owner>/<repo>` from `origin`,
/// else `root:<sha>` of the smallest root commit. `None` outside a git repository.
pub fn project_key_for_dir(dir: &Path) -> Option<String> {
    let root = GitSync::resolve_git_canonical_root(dir)?;
    if let Some(key) = GitSync::get_remote_url(&root, "origin").and_then(|u| normalize_remote(&u)) {
        return Some(key);
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .min()
        .map(|sha| format!("root:{sha}"))
}

impl Database {
    /// Maps `project_id` to `key`. A key already used by another project keeps its owner.
    pub fn set_project_key(&self, project_id: &str, key: &str) -> Result<bool> {
        let n = self.conn().execute(
            "INSERT OR IGNORE INTO project_keys (project_id, project_key) VALUES (?1, ?2)",
            params![project_id, key],
        )?;
        Ok(n > 0)
    }

    pub fn project_key_of(&self, project_id: &str) -> Option<String> {
        self.conn()
            .query_row("SELECT project_key FROM project_keys WHERE project_id = ?1", params![project_id], |r| r.get(0))
            .ok()
    }

    pub fn project_id_for_key(&self, key: &str) -> Option<String> {
        self.conn()
            .query_row("SELECT project_id FROM project_keys WHERE project_key = ?1", params![key], |r| r.get(0))
            .ok()
    }

    /// The local project for `key`, created (with a unique slug based on `slug_hint`) if this
    /// machine has not seen the repository yet.
    pub fn ensure_project_for_key(&self, key: &str, slug_hint: &str) -> Result<String> {
        if let Some(id) = self.project_id_for_key(key) {
            return Ok(id);
        }
        let base: String = slug_hint
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect();
        let base = if base.trim_matches('-').is_empty() { "project".to_string() } else { base };
        let mut slug = base.clone();
        let mut n = 2;
        while self.get_project_by_slug(&slug)?.is_some() {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let now = chrono::Utc::now();
        let project = Project {
            id: uuid::Uuid::now_v7().to_string(),
            slug: slug.clone(),
            display_name: slug,
            created_at: now,
            updated_at: now,
            capture_policy: Default::default(),
            context_budget: Default::default(),
            document_config: Default::default(),
            sync_config: Default::default(),
        };
        self.save_project(&project)?;
        self.set_project_key(&project.id, key)?;
        Ok(project.id)
    }

    /// Gives every registered project a key. A git repository's key comes from the repository (so another computer's checkout is the same project);
    /// a folder that is not a git repository gets `local:<project id>`, which makes its data syncable and lets the other computer create the project
    /// from the key. A project whose folder is registered but missing here is left alone (it may be a repository on another path). Cheap when all have one.
    pub fn link_project_keys(&self) -> usize {
        let mut linked = 0;
        for p in self.list_projects().unwrap_or_default() {
            if p.id == "demo" || self.project_key_of(&p.id).is_some() {
                continue;
            }
            let paths = self.get_project_paths(&p.id).unwrap_or_default();
            for pp in &paths {
                if let Some(key) = project_key_for_dir(Path::new(&pp.local_path)) {
                    if self.set_project_key(&p.id, &key).unwrap_or(false) {
                        linked += 1;
                    }
                    break;
                }
            }
            // Still no key: not a git repository, or the repository's key already belongs to another registered project (the same folder registered
            // twice, for instance by `connect` and by hand). Either way the project gets a key of its own, so its data syncs.
            let folder_here = paths.is_empty() || paths.iter().any(|pp| Path::new(&pp.local_path).is_dir());
            if self.project_key_of(&p.id).is_none() && folder_here && self.set_project_key(&p.id, &format!("local:{}", p.id)).unwrap_or(false) {
                linked += 1;
            }
        }
        linked
    }

    /// Moves everything filed under `from` (a project id that is not, or no longer, the right one: an orphan id, an unregistered folder's scope) to the
    /// project `to`. Returns how many rows moved per table. `to` must be a registered project.
    pub fn adopt_project_data(&self, from: &str, to: &str) -> Result<Vec<(&'static str, usize)>> {
        if from == to {
            anyhow::bail!("those are the same project");
        }
        if self.get_project_by_id(to)?.is_none() {
            anyhow::bail!("'{to}' is not a registered project");
        }
        let mut moved = Vec::new();
        let conn = self.conn();
        for table in ["memories", "decisions", "batons", "success_traces", "failure_records", "procedural_skills", "tool_events", "review_proposals", "tasks"] {
            let has_col: bool = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .map(|mut st| st.query_map([], |r| r.get::<_, String>(1)).map(|rows| rows.flatten().any(|c| c == "project_id")).unwrap_or(false))
                .unwrap_or(false);
            if !has_col {
                continue;
            }
            let n = conn.execute(&format!("UPDATE {table} SET project_id = ?1 WHERE project_id = ?2"), params![to, from])?;
            if n > 0 {
                moved.push((table, n));
            }
        }
        Ok(moved)
    }

    /// Resolves a `project_id` or slug to `(project_key, slug)`, if it has a key.
    pub fn key_and_slug(&self, project_ref: &str) -> Option<(String, String)> {
        let project = self
            .get_project_by_id(project_ref)
            .ok()
            .flatten()
            .or_else(|| self.get_project_by_slug(project_ref).ok().flatten())?;
        Some((self.project_key_of(&project.id)?, project.slug))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remotes_normalize_to_one_key() {
        for url in [
            "git@github.com:Owner/Repo.git",
            "https://github.com/owner/repo",
            "https://user:ghp_token@github.com/owner/repo.git/",
            "ssh://git@github.com:22/owner/repo.git",
            "git://github.com/OWNER/REPO",
        ] {
            assert_eq!(normalize_remote(url).as_deref(), Some("git:github.com/owner/repo"), "{url}");
        }
        assert_eq!(normalize_remote(""), None);
        assert_eq!(normalize_remote("nonsense"), None);
    }

    #[test]
    fn a_project_without_git_gets_a_local_key_and_adopting_moves_orphaned_rows() {
        let db = Database::open_in_memory().unwrap();
        let dir = std::env::temp_dir().join(format!("am-nogit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let now = chrono::Utc::now();
        let mk = |id: &str, slug: &str| Project { id: id.into(), slug: slug.into(), display_name: slug.into(), created_at: now, updated_at: now, capture_policy: Default::default(), context_budget: Default::default(), document_config: Default::default(), sync_config: Default::default() };
        db.save_project(&mk("p-plain", "plain")).unwrap();
        db.add_project_path("p-plain", &dir.to_string_lossy(), true).unwrap();
        db.save_project(&mk("p-gone", "gone")).unwrap();
        db.add_project_path("p-gone", "/definitely/not/here", true).unwrap();
        assert_eq!(db.link_project_keys(), 1);
        assert_eq!(db.project_key_of("p-plain").as_deref(), Some("local:p-plain"));
        assert_eq!(db.project_key_of("p-gone"), None, "a registered folder that is missing here is left alone: it may be a repository elsewhere");
        assert_eq!(db.link_project_keys(), 0, "idempotent");
        let _ = std::fs::remove_dir_all(&dir);

        // adopt: rows filed under an unregistered id move to a registered project
        db.conn().execute("INSERT INTO decisions (id, project_id, topic, decision, reason, agent, created_at, updated_at) VALUES ('d1', 'auto', 't', 'x', 'r', 'a', 'now', 'now')", []).unwrap();
        let moved = db.adopt_project_data("auto", "p-plain").unwrap();
        assert_eq!(moved, vec![("decisions", 1)]);
        let now_in: String = db.conn().query_row("SELECT project_id FROM decisions WHERE id = 'd1'", [], |r| r.get(0)).unwrap();
        assert_eq!(now_in, "p-plain");
        assert!(db.adopt_project_data("auto", "nope").is_err(), "the target must be a registered project");
        assert!(db.adopt_project_data("p-plain", "p-plain").is_err());
    }

    #[test]
    fn keys_map_both_ways_and_unknown_repos_create_projects() {
        let db = Database::open_in_memory().unwrap();
        let a = db.ensure_project_for_key("git:h/o/r", "App").unwrap();
        assert_eq!(db.ensure_project_for_key("git:h/o/r", "ignored").unwrap(), a, "same key, same project");
        assert_eq!(db.project_key_of(&a).as_deref(), Some("git:h/o/r"));
        assert_eq!(db.project_id_for_key("git:h/o/r").as_deref(), Some(a.as_str()));
        // A second repo with the same folder name gets its own project and a unique slug.
        let b = db.ensure_project_for_key("git:h/o/other", "App").unwrap();
        assert_ne!(a, b);
        assert_eq!(db.get_project_by_id(&b).unwrap().unwrap().slug, "app-2");
        // A key already owned is never reassigned.
        assert!(!db.set_project_key(&b, "git:h/o/r").unwrap());
        let (key, slug) = db.key_and_slug("app").unwrap();
        assert_eq!((key.as_str(), slug.as_str()), ("git:h/o/r", "app"));
    }
}
