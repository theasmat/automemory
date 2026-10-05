use crate::db::Database;
use crate::models::*;
use anyhow::Result;
use chrono::Utc;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use tracing::warn;

pub struct VaultManager<'a> {
    db: &'a Database,
    vault_root: PathBuf,
}

impl<'a> VaultManager<'a> {
    pub fn new<P: AsRef<Path>>(db: &'a Database, vault_root: P) -> Self {
        Self {
            db,
            vault_root: vault_root.as_ref().to_path_buf(),
        }
    }

    /// Import one JSONL file, saving each valid record and warning on skipped lines.
    /// Returns `(imported, skipped)` so silent data loss is impossible to miss in logs.
    fn import_jsonl<T>(path: &Path, mut save: impl FnMut(T)) -> (usize, usize)
    where
        T: serde::de::DeserializeOwned,
    {
        let Ok(file) = File::open(path) else {
            return (0, 0);
        };
        let (mut imported, mut skipped) = (0usize, 0usize);
        for (idx, line) in BufReader::new(file).lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    skipped += 1;
                    warn!(path = %path.display(), line_no = idx + 1, error = %e, "vault import: unreadable line skipped");
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<T>(&line) {
                Ok(v) => {
                    save(v);
                    imported += 1;
                }
                Err(e) => {
                    skipped += 1;
                    warn!(path = %path.display(), line_no = idx + 1, error = %e, "vault import: unparseable line skipped");
                }
            }
        }
        if skipped > 0 {
            warn!(path = %path.display(), imported, skipped, "vault import: some lines skipped");
        }
        (imported, skipped)
    }

    /// Save one imported memory. Import is last-writer-wins by id (restore
    /// semantics); when the incoming record is older than the local one, say
    /// so loudly instead of silently discarding newer local state.
    fn save_imported_memory(&self, path: &Path, mem: Memory) {
        if let Ok(Some(local)) = self.db.get_memory_by_id(&mem.id) {
            if local.updated_at > mem.updated_at {
                warn!(id = %mem.id, path = %path.display(), local_updated_at = %local.updated_at, incoming_updated_at = %mem.updated_at, "vault import: overwriting newer local record (last-writer-wins)");
            }
        }
        let _ = self.db.save_memory(&mem);
    }

    /// Export local state to the text-first vault directory layout.
    pub fn export_to_vault(&self) -> Result<()> {
        let global_dir = self.vault_root.join("global");
        let projects_dir = self.vault_root.join("projects");
        fs::create_dir_all(&global_dir)?;
        fs::create_dir_all(&projects_dir)?;

        // 1. Export Global Preferences
        let global_prefs = self.db.list_preferences(None)?;
        let prefs_file = global_dir.join("preferences.json");
        let mut f = File::create(&prefs_file)?;
        serde_json::to_writer_pretty(&mut f, &global_prefs)?;

        // 2. Export Global Memories
        let global_mem_dir = global_dir.join("memories");
        fs::create_dir_all(&global_mem_dir)?;
        let global_active = self
            .db
            .list_memories(None, Some(MemoryStatus::Active), None, 10000)?;
        let mut g_active_file = File::create(global_mem_dir.join("active.jsonl"))?;
        for m in &global_active {
            if m.project_id.is_none() || m.scope == MemoryScope::Global {
                writeln!(g_active_file, "{}", serde_json::to_string(m)?)?;
            }
        }

        let global_archived =
            self.db
                .list_memories(None, Some(MemoryStatus::Archived), None, 10000)?;
        let mut g_archived_file = File::create(global_mem_dir.join("archived.jsonl"))?;
        for m in &global_archived {
            if m.project_id.is_none() || m.scope == MemoryScope::Global {
                writeln!(g_archived_file, "{}", serde_json::to_string(m)?)?;
            }
        }

        // 3. Export Global Procedural Skills
        let global_skills_dir = global_dir.join("skills");
        fs::create_dir_all(&global_skills_dir)?;
        let all_skills = self.db.list_skills(None, 10000)?;
        let mut g_skills_file = File::create(global_skills_dir.join("skills.jsonl"))?;
        for s in &all_skills {
            if s.project_id.is_none() {
                writeln!(g_skills_file, "{}", serde_json::to_string(s)?)?;
            }
        }

        // 4. Export Projects
        let projects = self.db.list_projects()?;
        for proj in projects {
            let p_dir = projects_dir.join(&proj.slug);
            let mem_dir = p_dir.join("memories");
            let traces_dir = p_dir.join("traces").join("success");
            let failures_dir = p_dir.join("traces").join("failures");
            let skills_dir = p_dir.join("skills");
            let docs_dir = p_dir.join("docs");

            fs::create_dir_all(&mem_dir)?;
            fs::create_dir_all(&traces_dir)?;
            fs::create_dir_all(&failures_dir)?;
            fs::create_dir_all(&skills_dir)?;
            fs::create_dir_all(&docs_dir)?;

            // Project JSON
            let mut p_file = File::create(p_dir.join("project.json"))?;
            serde_json::to_writer_pretty(&mut p_file, &proj)?;

            // Memories active.jsonl & archived.jsonl
            let active_memories =
                self.db
                    .list_memories(Some(&proj.id), Some(MemoryStatus::Active), None, 10000)?;
            let mut active_file = File::create(mem_dir.join("active.jsonl"))?;
            for m in active_memories {
                if m.project_id.as_deref() == Some(&proj.id)
                    || m.project_id.as_deref() == Some(&proj.slug)
                {
                    writeln!(active_file, "{}", serde_json::to_string(&m)?)?;
                }
            }

            let archived_memories =
                self.db
                    .list_memories(Some(&proj.id), Some(MemoryStatus::Archived), None, 10000)?;
            let mut archived_file = File::create(mem_dir.join("archived.jsonl"))?;
            for m in archived_memories {
                if m.project_id.as_deref() == Some(&proj.id)
                    || m.project_id.as_deref() == Some(&proj.slug)
                {
                    writeln!(archived_file, "{}", serde_json::to_string(&m)?)?;
                }
            }

            // Project Skills
            let proj_skills = self.db.list_skills(Some(&proj.id), 10000)?;
            let mut p_skills_file = File::create(skills_dir.join("skills.jsonl"))?;
            for s in proj_skills {
                if s.project_id.as_deref() == Some(&proj.id)
                    || s.project_id.as_deref() == Some(&proj.slug)
                {
                    writeln!(p_skills_file, "{}", serde_json::to_string(&s)?)?;
                }
            }

            // Traces
            let traces = self.db.list_traces(Some(&proj.id), 5000)?;
            for t in traces {
                let mut t_file = File::create(traces_dir.join(format!("{}.json", t.id)))?;
                serde_json::to_writer_pretty(&mut t_file, &t)?;
            }

            // Failures
            let failures = self.db.list_failures(Some(&proj.id), None, 5000)?;
            for fl in failures {
                let mut fl_file = File::create(failures_dir.join(format!("{}.json", fl.id)))?;
                serde_json::to_writer_pretty(&mut fl_file, &fl)?;
            }

            // Tracked Documents
            let docs = self.db.list_documents(&proj.id)?;
            for d in docs {
                if let Ok(Some(rev)) = self.db.get_latest_revision(&d.id) {
                    let doc_file = docs_dir.join(&d.relative_path);
                    if let Some(parent) = doc_file.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(doc_file, &rev.content);
                }
            }
        }

        // Append to sync-log.jsonl
        let log_file = self.vault_root.join("sync-log.jsonl");
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;
        let log_entry = serde_json::json!({
            "action": "export",
            "timestamp": Utc::now().to_rfc3339(),
            "status": "success",
        });
        writeln!(log, "{}", log_entry)?;

        Ok(())
    }

    /// Import text-first vault records back into local database.
    pub fn import_from_vault(&self) -> Result<()> {
        let global_dir = self.vault_root.join("global");

        // 1. Import Global Preferences
        let global_prefs_path = global_dir.join("preferences.json");
        if global_prefs_path.exists() {
            if let Ok(data) = fs::read_to_string(&global_prefs_path) {
                if let Ok(prefs) = serde_json::from_str::<Vec<Preference>>(&data) {
                    for p in prefs {
                        let _ = self.db.set_preference(&p);
                    }
                }
            }
        }

        // 2. Import Global Memories
        let global_mem_active = global_dir.join("memories").join("active.jsonl");
        Self::import_jsonl::<Memory>(&global_mem_active, |mem| {
            self.save_imported_memory(&global_mem_active, mem);
        });

        let global_mem_archived = global_dir.join("memories").join("archived.jsonl");
        Self::import_jsonl::<Memory>(&global_mem_archived, |mem| {
            self.save_imported_memory(&global_mem_archived, mem);
        });

        // 3. Import Global Skills
        let global_skills = global_dir.join("skills").join("skills.jsonl");
        Self::import_jsonl::<ProceduralSkill>(&global_skills, |s| {
            let _ = self.db.save_skill(&s);
        });

        // 4. Import Projects
        let projects_dir = self.vault_root.join("projects");
        if !projects_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(projects_dir)? {
            let entry = entry?;
            let p_dir = entry.path();
            if !p_dir.is_dir() {
                continue;
            }

            let proj_json = p_dir.join("project.json");
            if proj_json.exists() {
                if let Ok(data) = fs::read_to_string(&proj_json) {
                    if let Ok(proj) = serde_json::from_str::<Project>(&data) {
                        let _ = self.db.save_project(&proj);
                    }
                }
            }

            // Import memories
            let active_jsonl = p_dir.join("memories").join("active.jsonl");
            Self::import_jsonl::<Memory>(&active_jsonl, |mem| {
                self.save_imported_memory(&active_jsonl, mem);
            });

            let archived_jsonl = p_dir.join("memories").join("archived.jsonl");
            Self::import_jsonl::<Memory>(&archived_jsonl, |mem| {
                self.save_imported_memory(&archived_jsonl, mem);
            });

            // Import skills
            let skills_jsonl = p_dir.join("skills").join("skills.jsonl");
            Self::import_jsonl::<ProceduralSkill>(&skills_jsonl, |s| {
                let _ = self.db.save_skill(&s);
            });

            // Import traces
            let traces_dir = p_dir.join("traces").join("success");
            if traces_dir.exists() {
                for t_entry in fs::read_dir(traces_dir)? {
                    let t_entry = t_entry?;
                    if t_entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(data) = fs::read_to_string(t_entry.path()) {
                            if let Ok(t) = serde_json::from_str::<SuccessTrace>(&data) {
                                let _ = self.db.save_trace(&t);
                            }
                        }
                    }
                }
            }

            // Import failures
            let failures_dir = p_dir.join("traces").join("failures");
            if failures_dir.exists() {
                for fl_entry in fs::read_dir(failures_dir)? {
                    let fl_entry = fl_entry?;
                    if fl_entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(data) = fs::read_to_string(fl_entry.path()) {
                            if let Ok(fl) = serde_json::from_str::<FailureRecord>(&data) {
                                let _ = self.db.save_failure(&fl);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
