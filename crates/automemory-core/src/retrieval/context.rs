use crate::db::Database;
use crate::models::*;
use anyhow::Result;

pub struct RetrievalEngine<'a> {
    db: &'a Database,
}

impl<'a> RetrievalEngine<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Assembles the bounded context package for session boot or task execution.
    pub fn assemble_context(
        &self,
        project_id: Option<&str>,
        task_query: Option<&str>,
        max_tokens: usize,
    ) -> Result<String> {
        let max_chars = max_tokens * 4;
        let mut sections: Vec<String> = Vec::new();
        let mut current_chars = 0;

        let add_section = |title: &str, body: &str, sections: &mut Vec<String>, current_chars: &mut usize| {
            if body.trim().is_empty() {
                return;
            }
            let formatted = format!("### {}\n\n{}\n", title, body.trim());
            if *current_chars + formatted.len() <= max_chars {
                *current_chars += formatted.len();
                sections.push(formatted);
            }
        };

        // 1. Locked Global Preferences & Active Preferences
        let prefs = self.db.list_preferences(project_id)?;
        let mut locked_prefs_str = String::new();
        let mut other_prefs_str = String::new();
        for p in prefs {
            let line = format!("- **{}**: {}\n", p.key, p.value);
            if p.locked {
                locked_prefs_str.push_str(&line);
            } else {
                other_prefs_str.push_str(&line);
            }
        }
        if !locked_prefs_str.is_empty() {
            add_section("Locked Global Preferences", &locked_prefs_str, &mut sections, &mut current_chars);
        }

        // 2. Project Identity & Stack Summary
        if let Some(pid) = project_id {
            if let Some(proj) = self.db.get_project_by_id(pid)? {
                let paths = self.db.get_project_paths(pid)?;
                let path_list = paths
                    .iter()
                    .map(|p| format!("`{}`", p.local_path))
                    .collect::<Vec<_>>()
                    .join(", ");
                let summary = format!(
                    "- **Project**: {} (`{}`)\n- **Registered Paths**: {}\n- **Sync Mode**: {}\n",
                    proj.display_name, proj.slug, path_list, proj.sync_config.mode
                );
                add_section("Project Identity", &summary, &mut sections, &mut current_chars);
            }
        }

        // 3. High-confidence Project Decisions and Architectural Facts
        let memories = if let Some(query) = task_query {
            let fts_results = self.db.search_memories_fts(query, project_id, 10)?;
            fts_results.into_iter().map(|(m, _)| m).collect()
        } else {
            self.db.list_memories(project_id, Some(MemoryStatus::Active), None, 15)?
        };

        let mut decisions_and_facts = String::new();
        let mut warnings_str = String::new();

        for m in memories {
            let line = format!("- [{}] {}\n", m.memory_type, m.summary);
            match m.memory_type {
                MemoryType::Warning => warnings_str.push_str(&line),
                MemoryType::Decision | MemoryType::Fact | MemoryType::Pattern => {
                    decisions_and_facts.push_str(&line);
                }
                _ => {}
            }
        }

        if !decisions_and_facts.is_empty() {
            add_section("High-Confidence Decisions & Architectural Facts", &decisions_and_facts, &mut sections, &mut current_chars);
        }

        // 4. Relevant Preferences
        if !other_prefs_str.is_empty() {
            add_section("Preferences & Conventions", &other_prefs_str, &mut sections, &mut current_chars);
        }

        // 5. Relevant Warnings and Unresolved Failures
        let failures = self.db.list_failures(project_id, Some("unresolved"), 5)?;
        for f in failures {
            let line = format!("- **Failure in `{}`**: {}\n", f.task_title, f.error_message);
            warnings_str.push_str(&line);
        }
        if !warnings_str.is_empty() {
            add_section("Active Warnings & Unresolved Failures", &warnings_str, &mut sections, &mut current_chars);
        }

        // 6. Relevant Successful Procedures (Success Traces)
        let traces = self.db.list_traces(project_id, 5)?;
        let mut traces_str = String::new();
        for t in traces {
            if t.status == "active" {
                let steps_preview = t
                    .steps
                    .iter()
                    .take(2)
                    .map(|s| s.action.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                traces_str.push_str(&format!(
                    "- **Trace: {}** (category: {}, score: {:.2}): {}\n",
                    t.title, t.category, t.reproducibility_score, steps_preview
                ));
            }
        }
        if !traces_str.is_empty() {
            add_section("Relevant Success Traces (Repeatable Procedures)", &traces_str, &mut sections, &mut current_chars);
        }

        // 7. Tracked Document Excerpts & File References
        if let Some(pid) = project_id {
            let docs = self.db.list_documents(pid)?;
            let mut docs_str = String::new();
            for d in docs {
                let sum = d.summary.unwrap_or_else(|| "Tracked project document".to_string());
                docs_str.push_str(&format!("- `{}`: {}\n", d.relative_path, sum));
            }
            if !docs_str.is_empty() {
                add_section("Tracked Project Documents", &docs_str, &mut sections, &mut current_chars);
            }
        }

        // 8. Memory Instructions
        let instructions = "- Memories and traces are persistent across chats.\n- High-confidence facts have been verified by prior tool executions.\n- Use `memory.save` or `trace.save` for durable learnings, or `failure.resolve` when an error is fixed.\n";
        add_section("AutoMemory Context Guidelines", instructions, &mut sections, &mut current_chars);

        let mut output = String::new();
        output.push_str("# AutoMemory Pro Active Context\n\n");
        for sec in sections {
            output.push_str(&sec);
            output.push('\n');
        }

        Ok(output)
    }

    /// Hybrid search across memories, traces, and failures
    pub fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // 1. Search memories via FTS5
        let mem_results = self.db.search_memories_fts(query, project_id, limit)?;
        for (m, rank) in mem_results {
            results.push(SearchResult {
                id: m.id,
                kind: format!("memory:{}", m.memory_type),
                title: m.summary,
                content: m.content,
                score: 1.0 / (1.0 + rank.abs()),
                project_id: m.project_id,
            });
        }

        // 2. Search traces
        let traces = self.db.list_traces(project_id, limit)?;
        let q_lower = query.to_lowercase();
        for t in traces {
            if t.title.to_lowercase().contains(&q_lower) || t.category.to_lowercase().contains(&q_lower) {
                results.push(SearchResult {
                    id: t.id,
                    kind: "trace".to_string(),
                    title: t.title.clone(),
                    content: format!("Category: {}, Preconditions: {:?}", t.category, t.preconditions),
                    score: 0.85,
                    project_id: t.project_id,
                });
            }
        }

        // 3. Search failures
        let failures = self.db.list_failures(project_id, None, limit)?;
        for f in failures {
            if f.task_title.to_lowercase().contains(&q_lower)
                || f.error_message.to_lowercase().contains(&q_lower)
            {
                results.push(SearchResult {
                    id: f.id,
                    kind: "failure".to_string(),
                    title: f.task_title.clone(),
                    content: format!("Error: {}, Fix: {:?}", f.error_message, f.fix_applied),
                    score: 0.80,
                    project_id: f.project_id,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub score: f64,
    pub project_id: Option<String>,
}
