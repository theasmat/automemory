use crate::db::Database;
use crate::extraction::heuristics::{EventHeuristics, ExtractionOutcome};
use crate::extraction::skills::SkillDistiller;
use crate::models::ProceduralSkill;
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};

pub struct ExtractionWorker {
    db: Arc<Database>,
}

impl ExtractionWorker {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Process a batch of pending unprocessed events from the ledger.
    pub fn process_pending_batch(&self, batch_size: usize) -> Result<usize> {
        let events = self.db.get_unprocessed_events(batch_size)?;
        let count = events.len();
        if count == 0 {
            return Ok(0);
        }

        for event in &events {
            match EventHeuristics::evaluate(event) {
                ExtractionOutcome::Skip => {}
                ExtractionOutcome::AddMemory(memory) => {
                    info!("Extracted memory: {}", memory.summary);
                    if let Err(e) = self.db.save_memory(&memory) {
                        warn!("Failed to save extracted memory: {}", e);
                    }
                }
                ExtractionOutcome::AddTrace(trace) => {
                    info!("Extracted trace: {}", trace.title);
                    if let Err(e) = self.db.save_trace(&trace) {
                        warn!("Failed to save extracted trace: {}", e);
                    }
                }
                ExtractionOutcome::AddFailure(failure) => {
                    info!("Extracted failure record: {}", failure.task_title);
                    if let Err(e) = self.db.save_failure(&failure) {
                        warn!("Failed to save extracted failure: {}", e);
                    }
                }
            }
            if let Err(e) = self.db.mark_event_processed(&event.event_id) {
                warn!("Failed to mark event {} as processed: {}", event.event_id, e);
            }
        }

        Ok(count)
    }

    /// Autonomous skill promotion pass:
    /// Clusters repeated success traces (>= 3 runs, >= 0.85 reproducibility)
    /// into persistent ProceduralSkill playbooks.
    pub fn promote_skills(&self, project_id: Option<&str>) -> Result<Vec<ProceduralSkill>> {
        let distiller = SkillDistiller::new(&self.db);
        let promoted = distiller.distill_skills_with_threshold(project_id, 3, 0.85)?;
        for skill in &promoted {
            if let Err(e) = self.db.save_skill(skill) {
                warn!("Failed to persist promoted skill {}: {}", skill.name, e);
            } else {
                info!("Promoted procedural skill: {} (from {} traces)", skill.name, skill.source_trace_ids.len());
            }
        }
        Ok(promoted)
    }

    /// Continuous background extraction loop
    pub async fn run_loop(self: Arc<Self>, interval: std::time::Duration) {
        let mut ticker = tokio::time::interval(interval);
        let mut tick_counter: u64 = 0;
        loop {
            ticker.tick().await;
            tick_counter = tick_counter.wrapping_add(1);

            if let Err(e) = self.process_pending_batch(50) {
                warn!("Extraction batch error: {}", e);
            }

            // Every 10 iterations (~40s), run autonomous skill promotion pass
            if tick_counter % 10 == 0 {
                if let Err(e) = self.promote_skills(None) {
                    warn!("Autonomous skill promotion error: {}", e);
                }
            }
        }
    }
}
