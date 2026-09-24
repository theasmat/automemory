use super::decay::MemoryDecayScorer;
use super::scorer::HybridScorer;
use super::vector::VectorEngine;
use crate::db::Database;
use crate::models::*;
use anyhow::Result;

pub struct RetrievalEngine<'a> {
    db: &'a Database,
    vec_engine: VectorEngine,
    scorer: HybridScorer,
    decay_scorer: MemoryDecayScorer,
}

impl<'a> RetrievalEngine<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            vec_engine: VectorEngine::default(),
            scorer: HybridScorer::default(),
            decay_scorer: MemoryDecayScorer::default(),
        }
    }

    /// Generates a token-capped (max 500 tokens) Hot Memory Constitution for the project.
    pub fn generate_constitution(&self, project_id: Option<&str>) -> Result<String> {
        let mut lines = Vec::new();
        lines.push("# Project Constitution & Safety Rules\n".to_string());

        // 1. Locked & Core Preferences
        let prefs = self.db.list_preferences(project_id)?;
        if !prefs.is_empty() {
            lines.push("## Core Conventions".to_string());
            for p in prefs.iter().take(5) {
                lines.push(format!("- **{}**: {}", p.key, p.value));
            }
            lines.push("".to_string());
        }

        // 2. High-Confidence Architectural Decisions
        let memories = self.db.list_memories(project_id, Some(MemoryStatus::Active), None, 10)?;
        let decisions: Vec<_> = memories
            .iter()
            .filter(|m| m.memory_type == MemoryType::Decision || m.memory_type == MemoryType::Fact)
            .take(5)
            .collect();
        if !decisions.is_empty() {
            lines.push("## Architecture Invariants".to_string());
            for d in decisions {
                lines.push(format!("- {}", d.summary));
            }
            lines.push("".to_string());
        }

        // 3. Known Gotchas & Pitfalls to Avoid
        let failures = self.db.list_failures(project_id, Some("resolved"), 5)?;
        if !failures.is_empty() {
            lines.push("## Known Pitfalls & Past Resolutions".to_string());
            for f in failures.iter().take(3) {
                lines.push(format!("- **Avoid**: {} -> **Fix**: {}", f.task_title, f.fix_applied.as_deref().unwrap_or("See trace")));
            }
            lines.push("".to_string());
        }

        // 4. Verification Check
        lines.push("## Inviolable Verification".to_string());
        lines.push("- Always verify code changes with existing test suites before declaring tasks complete.".to_string());
        lines.push("- Never execute destructive system/git commands without explicit user confirmation.".to_string());

        Ok(lines.join("\n"))
    }

    /// HyDE (Hypothetical Document Embeddings) Query Rewriting
    pub fn expand_hyde_query(&self, query: &str) -> String {
        let q_clean = query.trim();
        let q_lower = q_clean.to_lowercase();

        if q_lower.starts_with("how") || q_lower.starts_with("where") || q_lower.starts_with("why") || q_lower.starts_with("what") {
            format!("{} architecture setup configuration pattern implementation", q_clean)
        } else {
            q_clean.to_string()
        }
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

        // 3. High-confidence Project Decisions and Architectural Facts (HyDE + RRF Hybrid Search)
        let memories = if let Some(raw_query) = task_query {
            let expanded_query = self.expand_hyde_query(raw_query);
            let fts_results = self.db.search_memories_fts(&expanded_query, project_id, 15).unwrap_or_default();
            let query_emb = self.vec_engine.embed_text(&expanded_query).unwrap_or_default();
            let vec_results = if !query_emb.is_empty() {
                self.db.search_memories_vector(&query_emb, project_id, 15).unwrap_or_default()
            } else {
                Vec::new()
            };

            let scored = self.scorer.fuse_and_rank(fts_results, vec_results, 10);
            if scored.is_empty() {
                self.db.list_memories(project_id, Some(MemoryStatus::Active), None, 10)?
            } else {
                scored.into_iter().map(|c| c.memory).collect()
            }
        } else {
            self.db.list_memories(project_id, Some(MemoryStatus::Active), None, 15)?
        };

        let mut decisions_and_facts = String::new();
        let mut warnings_str = String::new();

        for m in memories {
            let eval = self.decay_scorer.evaluate(
                m.importance,
                m.confidence,
                m.updated_at,
                m.version.unwrap_or(1) as u32,
                false,
                None,
            );

            if eval.is_stale {
                continue;
            }

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

        // 7. Memory Instructions
        let instructions = "- Memories and traces are persistent across chats.\n- High-confidence facts have been verified by prior tool executions.\n- Use `memory_save` or `trace_save` for durable learnings, or `failure_resolve` when an error is fixed.\n";
        add_section("AutoMemory Context Guidelines", instructions, &mut sections, &mut current_chars);

        let mut output = String::new();
        output.push_str("# AutoMemory Pro Active Context\n\n");
        for sec in sections {
            output.push_str(&sec);
            output.push('\n');
        }

        Ok(output)
    }

    /// Hybrid search across memories (FTS5 + Vector Embeddings RRF), traces, and failures
    pub fn search(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();
        let expanded_query = self.expand_hyde_query(query);

        // 1. Hybrid Search over memories via FTS5 + Vector Embeddings with RRF
        let fts_results = self.db.search_memories_fts(&expanded_query, project_id, limit * 2).unwrap_or_default();
        let query_emb = self.vec_engine.embed_text(&expanded_query).unwrap_or_default();
        let vec_results = if !query_emb.is_empty() {
            self.db.search_memories_vector(&query_emb, project_id, limit * 2).unwrap_or_default()
        } else {
            Vec::new()
        };

        let scored_memories = self.scorer.fuse_and_rank(fts_results, vec_results, limit);
        for candidate in scored_memories {
            results.push(SearchResult {
                id: candidate.memory.id,
                kind: format!("memory:{}", candidate.memory.memory_type),
                title: candidate.memory.summary,
                content: candidate.memory.content,
                score: candidate.final_score,
                project_id: candidate.memory.project_id,
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
