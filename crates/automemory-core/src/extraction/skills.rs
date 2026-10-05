use crate::db::Database;
use crate::models::{ProceduralSkill, SuccessTrace};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

pub struct SkillDistiller<'a> {
    db: &'a Database,
}

impl<'a> SkillDistiller<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Clusters active SuccessTraces by category and title similarity,
    /// synthesizing consolidated reusable ProceduralSkill playbooks.
    pub fn distill_skills(&self, project_id: Option<&str>) -> Result<Vec<ProceduralSkill>> {
        self.distill_skills_with_threshold(project_id, 1, 0.0)
    }

    /// Autonomous skill promotion with minimum repetition count and reproducibility threshold.
    pub fn distill_skills_with_threshold(
        &self,
        project_id: Option<&str>,
        min_occurrences: usize,
        min_reproducibility: f64,
    ) -> Result<Vec<ProceduralSkill>> {
        let traces = self.db.list_traces(project_id, 100)?;
        if traces.is_empty() {
            return Ok(Vec::new());
        }

        // Group traces by normalized category / goal
        let mut clusters: HashMap<String, Vec<&SuccessTrace>> = HashMap::new();
        for trace in &traces {
            if trace.status != "active" {
                continue;
            }
            let key = format!(
                "{}:{}",
                trace.category.trim().to_lowercase(),
                trace.title.trim().to_lowercase()
            );
            clusters.entry(key).or_default().push(trace);
        }

        let mut skills = Vec::new();

        for (_cluster_key, cluster_traces) in clusters {
            if cluster_traces.len() < min_occurrences {
                continue;
            }

            let avg_repro = cluster_traces
                .iter()
                .map(|t| t.reproducibility_score)
                .sum::<f64>()
                / (cluster_traces.len() as f64);

            if avg_repro < min_reproducibility {
                continue;
            }

            let first = cluster_traces[0];
            let skill_id = format!("skill_{}", Uuid::new_v4().simple());
            let name = format!("Procedural Playbook: {}", first.title);
            let description = format!(
                "Autonomous procedural playbook distilled from {} successful execution trace(s) in category '{}' (avg reproducibility: {:.2}).",
                cluster_traces.len(),
                first.category,
                avg_repro
            );

            let trigger_patterns = vec![first.category.clone(), first.title.clone()];

            let mut ordered_steps = Vec::new();
            for (idx, step) in first.steps.iter().enumerate() {
                let desc = if let Some(ref cmd) = step.command {
                    format!("{}. {} (`{}`)", idx + 1, step.action, cmd)
                } else {
                    format!("{}. {}", idx + 1, step.action)
                };
                ordered_steps.push(desc);
            }

            let source_trace_ids = cluster_traces.iter().map(|t| t.id.clone()).collect();
            let caveats = first.caveats.clone();

            skills.push(ProceduralSkill {
                id: skill_id,
                project_id: project_id.map(String::from),
                name,
                description,
                trigger_patterns,
                preconditions: first.preconditions.clone(),
                steps: ordered_steps,
                caveats,
                success_count: cluster_traces.len() as u32,
                failure_count: 0,
                source_trace_ids,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
        }

        Ok(skills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TraceStep;

    #[test]
    fn test_trace_to_skill_synthesis() {
        let db = Database::open_in_memory().unwrap();
        let distiller = SkillDistiller::new(&db);

        let t1 = SuccessTrace {
            id: "t1".to_string(),
            project_id: Some("p1".to_string()),
            title: "Resolve SQLite Contention".to_string(),
            category: "database".to_string(),
            status: "active".to_string(),
            preconditions: vec!["SQLite DB initialized".to_string()],
            steps: vec![TraceStep {
                position: 1,
                action: "Set busy timeout pragma".to_string(),
                tool: "edit".to_string(),
                command: Some("PRAGMA busy_timeout = 5000".to_string()),
                verification: Some("Passed".to_string()),
            }],
            reproducibility_score: 0.98,
            caveats: vec![],
            source_task_id: None,
            evidence_event_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        db.save_trace(&t1).unwrap();

        let skills = distiller.distill_skills(Some("p1")).unwrap();
        assert_eq!(skills.len(), 1);
        assert!(skills[0].name.contains("Resolve SQLite Contention"));
        assert_eq!(skills[0].steps.len(), 1);
    }

    #[test]
    fn test_autonomous_skill_promotion_thresholds() {
        let db = Database::open_in_memory().unwrap();
        let distiller = SkillDistiller::new(&db);

        // Add 2 traces (less than min_occurrences = 3)
        for i in 1..=2 {
            let trace = SuccessTrace {
                id: format!("trace_{}", i),
                project_id: Some("proj_alpha".to_string()),
                title: "Run cargo build in release mode".to_string(),
                category: "build".to_string(),
                status: "active".to_string(),
                preconditions: vec![],
                steps: vec![TraceStep {
                    position: 1,
                    action: "Compile release binary".to_string(),
                    tool: "run_command".to_string(),
                    command: Some("cargo build --release".to_string()),
                    verification: Some("Success".to_string()),
                }],
                reproducibility_score: 0.95,
                caveats: vec![],
                source_task_id: None,
                evidence_event_ids: vec![],
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            db.save_trace(&trace).unwrap();
        }

        // With threshold 3, it should NOT distill a skill yet
        let pending = distiller
            .distill_skills_with_threshold(Some("proj_alpha"), 3, 0.85)
            .unwrap();
        assert_eq!(pending.len(), 0);

        // Add the 3rd trace to cross the threshold
        let trace3 = SuccessTrace {
            id: "trace_3".to_string(),
            project_id: Some("proj_alpha".to_string()),
            title: "Run cargo build in release mode".to_string(),
            category: "build".to_string(),
            status: "active".to_string(),
            preconditions: vec![],
            steps: vec![TraceStep {
                position: 1,
                action: "Compile release binary".to_string(),
                tool: "run_command".to_string(),
                command: Some("cargo build --release".to_string()),
                verification: Some("Success".to_string()),
            }],
            reproducibility_score: 0.92,
            caveats: vec![],
            source_task_id: None,
            evidence_event_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db.save_trace(&trace3).unwrap();

        // Now it crosses the threshold of 3 occurrences and >= 0.85 reproducibility!
        let promoted = distiller
            .distill_skills_with_threshold(Some("proj_alpha"), 3, 0.85)
            .unwrap();
        assert_eq!(promoted.len(), 1);
        assert_eq!(promoted[0].success_count, 3);
        assert!(promoted[0].name.contains("Run cargo build in release mode"));

        // Save to DB and verify DB roundtrip
        db.save_skill(&promoted[0]).unwrap();
        let listed = db.list_skills(Some("proj_alpha"), 10).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, promoted[0].id);
    }
}
