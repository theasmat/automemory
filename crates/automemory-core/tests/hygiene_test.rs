use automemory_core::db::Database;
use automemory_core::models::*;
use automemory_core::retrieval::RetrievalEngine;
use chrono::Utc;
use serde_json::json;

fn memory(id: &str, text: &str) -> Memory {
    Memory {
        id: id.into(), scope: MemoryScope::Global, project_id: None, memory_type: MemoryType::Fact,
        content: text.into(), summary: text.into(), confidence: 0.9, importance: 0.5, freshness: 1.0,
        status: MemoryStatus::Active, source_kind: MemorySourceKind::Agent, version: Some(1),
        created_at: Utc::now(), updated_at: Utc::now(), last_recalled_at: None,
        tags: vec![], entities: vec![], metadata: json!({}),
    }
}

#[test]
fn recalling_a_memory_records_when_it_was_used() {
    let db = Database::open_in_memory().unwrap();
    db.save_memory(&memory("m-used", "the payment gateway timeout is 30 seconds")).unwrap();
    db.save_memory(&memory("m-idle", "the mascot is a purple heron")).unwrap();

    RetrievalEngine::keyword_only(&db).search("payment gateway timeout", None, 5).unwrap();

    let rows = db.hygiene_rows(None).unwrap();
    let recalled = |id: &str| rows.iter().find(|r| r.0 == id).unwrap().5.is_some();
    assert!(recalled("m-used"), "the memory handed to the agent gets a last-recalled time");
    assert!(!recalled("m-idle"), "a memory nobody asked for stays unused");
}
