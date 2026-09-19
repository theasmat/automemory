pub mod config;
pub mod db;
pub mod docs;
pub mod extraction;
pub mod models;
pub mod redaction;
pub mod retrieval;
pub mod sync;

pub use config::Config;
pub use db::Database;
pub use docs::{DocumentScanner, DocumentWatcher};
pub use extraction::{ConsolidationReport, ConsolidationWorker, EventHeuristics, ExtractionWorker, LlmExtractor, LlmProviderConfig};
pub use models::*;
pub use redaction::{contains_secrets, redact_json, redact_text};
pub use retrieval::{RetrievalEngine, SearchResult};
pub use sync::{AutoSyncDaemon, GitSync, VaultManager};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_core_database_flow() {
        let db = Database::open_in_memory().expect("in-memory db open failed");

        // 1. Create Project
        let project = Project {
            id: "proj-1".to_string(),
            slug: "test-project".to_string(),
            display_name: "Test Project".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            capture_policy: CapturePolicy::default(),
            context_budget: ContextBudget::default(),
            document_config: DocumentConfig::default(),
            sync_config: SyncConfig::default(),
        };
        db.save_project(&project).expect("save_project failed");
        db.add_project_path("proj-1", "/dev/test-project", true).expect("add_project_path failed");

        // Lookup
        let found = db.find_project_by_path("/dev/test-project/src/main.rs").expect("find failed");
        assert!(found.is_some());
        assert_eq!(found.unwrap().slug, "test-project");

        // 2. Save Memory
        let mem = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj-1".to_string()),
            memory_type: MemoryType::Decision,
            content: "Authentication uses 30-minute JWT expiry tokens.".to_string(),
            summary: "Use 30m JWT expiry".to_string(),
            confidence: 0.95,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["auth".to_string(), "jwt".to_string()],
            entities: vec!["jwt".to_string()],
            metadata: serde_json::json!({}),
        };
        db.save_memory(&mem).expect("save_memory failed");

        // 3. FTS5 Search
        let results = db.search_memories_fts("JWT", Some("proj-1"), 5).expect("fts search failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.summary, "Use 30m JWT expiry");

        // 4. Memory Update & Versioning
        let mut updated_mem = mem.clone();
        updated_mem.content = "Authentication uses 60-minute JWT expiry tokens.".to_string();
        updated_mem.summary = "Use 60m JWT expiry".to_string();
        db.update_memory(&updated_mem, Some("Increased timeout for stability"), &[])
            .expect("update_memory failed");

        let versions = db.get_memory_versions(&mem.id).expect("get_memory_versions failed");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[1].version_num, 2);
        assert_eq!(versions[1].summary, "Use 60m JWT expiry");

        // 5. Context Assembly
        let engine = RetrievalEngine::new(&db);
        let ctx = engine
            .assemble_context(Some("proj-1"), None, 2000)
            .expect("assemble_context failed");
        assert!(ctx.contains("Test Project"));
        assert!(ctx.contains("Use 60m JWT expiry"));

        // 6. Consolidation pass test
        let mut dup_mem = updated_mem.clone();
        dup_mem.id = Uuid::now_v7().to_string();
        dup_mem.confidence = 0.5;
        db.save_memory(&dup_mem).expect("save duplicate failed");

        let consolidator = ConsolidationWorker::new(std::sync::Arc::new(db));
        let report = consolidator.consolidate(Some("proj-1")).expect("consolidate failed");
        assert_eq!(report.duplicates_merged, 1);
        assert_eq!(report.proposals_created, 1);
    }
}
