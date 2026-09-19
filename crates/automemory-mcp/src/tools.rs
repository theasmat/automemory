use crate::protocol::{ToolCallResult, ToolDefinition};
use automemory_core::db::Database;
use automemory_core::models::*;
use automemory_core::redaction::redact_text;
use automemory_core::retrieval::RetrievalEngine;
use chrono::Utc;
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

pub struct ToolRegistry {
    db: Arc<Database>,
}

impl ToolRegistry {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        vec![
            // Read Tools
            ToolDefinition {
                name: "memory.recall".to_string(),
                description: "Retrieve relevant project or global memories for a query".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" },
                        "limit": { "type": "integer", "description": "Max results to return (default 5)" }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "memory.context".to_string(),
                description: "Build a bounded active context package for current project and task".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Project ID or slug" },
                        "task_query": { "type": "string", "description": "Optional user prompt or task query" },
                        "max_tokens": { "type": "integer", "description": "Token budget (default 2000)" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.get".to_string(),
                description: "Read a specific memory record including version history and provenance".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Memory ID" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "memory.search".to_string(),
                description: "Hybrid search over memories, success traces, failures, and document summaries".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "project_id": { "type": "string", "description": "Optional project ID" },
                        "limit": { "type": "integer", "description": "Max results (default 10)" }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "memory.list".to_string(),
                description: "List memories filtered by project, type, status, or confidence".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "status": { "type": "string", "enum": ["active", "superseded", "archived", "invalid", "review"] },
                        "type": { "type": "string", "enum": ["fact", "decision", "lesson", "pattern", "warning", "observation", "preference", "skill"] },
                        "limit": { "type": "integer" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.stats".to_string(),
                description: "Return memory health statistics and counts".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" }
                    }
                }),
            },
            ToolDefinition {
                name: "trace.list".to_string(),
                description: "List repeatable success traces for a project".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "limit": { "type": "integer" }
                    }
                }),
            },
            ToolDefinition {
                name: "trace.get".to_string(),
                description: "Read a success trace with ordered steps and preconditions".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "failure.list".to_string(),
                description: "List recorded failure records and root-cause fixes".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "status": { "type": "string", "enum": ["unresolved", "resolved", "obsolete"] },
                        "limit": { "type": "integer" }
                    }
                }),
            },
            ToolDefinition {
                name: "failure.get".to_string(),
                description: "Read a failure record and its resolution details".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "project.list".to_string(),
                description: "List registered projects and their paths".to_string(),
                input_schema: json!({ "type": "object" }),
            },
            ToolDefinition {
                name: "project.get".to_string(),
                description: "Get project configuration and memory summary".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "doc.list".to_string(),
                description: "List tracked project documents and revision status".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" }
                    },
                    "required": ["project_id"]
                }),
            },
            ToolDefinition {
                name: "doc.get".to_string(),
                description: "Read tracked document metadata or revision".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "relative_path": { "type": "string" }
                    },
                    "required": ["project_id", "relative_path"]
                }),
            },
            ToolDefinition {
                name: "preference.get".to_string(),
                description: "Read a global or project preference".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" },
                        "project_id": { "type": "string" }
                    },
                    "required": ["key"]
                }),
            },
            ToolDefinition {
                name: "sync.status".to_string(),
                description: "View sync status and pending operations".to_string(),
                input_schema: json!({ "type": "object" }),
            },
            // Write Tools
            ToolDefinition {
                name: "memory.save".to_string(),
                description: "Explicitly save a durable memory (fact, decision, lesson, pattern, etc.)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "content": { "type": "string", "description": "Detailed explanation" },
                        "summary": { "type": "string", "description": "Concise 1-line summary" },
                        "type": { "type": "string", "enum": ["fact", "decision", "lesson", "pattern", "warning", "observation", "preference", "skill"] },
                        "scope": { "type": "string", "enum": ["global", "project"] },
                        "project_id": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                    },
                    "required": ["content", "summary"]
                }),
            },
            ToolDefinition {
                name: "memory.update".to_string(),
                description: "Update an active memory record, creating a new version with reason".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "content": { "type": "string" },
                        "summary": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["id", "content", "summary"]
                }),
            },
            ToolDefinition {
                name: "memory.archive".to_string(),
                description: "Archive a memory so it is excluded from active retrieval".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "memory.restore".to_string(),
                description: "Restore an archived memory back to active status".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" }
                    },
                    "required": ["id"]
                }),
            },
            ToolDefinition {
                name: "trace.save".to_string(),
                description: "Explicitly save a reusable success procedure with steps".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "category": { "type": "string" },
                        "steps": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "position": { "type": "integer" },
                                    "action": { "type": "string" },
                                    "tool": { "type": "string" },
                                    "command": { "type": "string" },
                                    "verification": { "type": "string" }
                                },
                                "required": ["position", "action", "tool"]
                            }
                        },
                        "project_id": { "type": "string" },
                        "preconditions": { "type": "array", "items": { "type": "string" } },
                        "caveats": { "type": "array", "items": { "type": "string" } },
                        "reproducibility_score": { "type": "number" }
                    },
                    "required": ["title", "category", "steps"]
                }),
            },
            ToolDefinition {
                name: "failure.save".to_string(),
                description: "Explicitly save a failure record and root-cause hypothesis".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task_title": { "type": "string" },
                        "error_message": { "type": "string" },
                        "project_id": { "type": "string" },
                        "failed_tool": { "type": "string" },
                        "root_cause_hypothesis": { "type": "string" },
                        "fix_applied": { "type": "string" },
                        "verification_result": { "type": "string" }
                    },
                    "required": ["task_title", "error_message"]
                }),
            },
            ToolDefinition {
                name: "failure.resolve".to_string(),
                description: "Mark a failure as resolved with fix and verification".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "fix_applied": { "type": "string" },
                        "verification_result": { "type": "string" }
                    },
                    "required": ["id", "fix_applied", "verification_result"]
                }),
            },
            ToolDefinition {
                name: "preference.set".to_string(),
                description: "Set a global or project preference".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" },
                        "value": {},
                        "scope": { "type": "string", "enum": ["global", "project"] },
                        "project_id": { "type": "string" }
                    },
                    "required": ["key", "value"]
                }),
            },
            ToolDefinition {
                name: "preference.lock".to_string(),
                description: "Lock a preference so agents cannot modify it".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string" },
                        "project_id": { "type": "string" },
                        "locked": { "type": "boolean" }
                    },
                    "required": ["key", "locked"]
                }),
            },
            ToolDefinition {
                name: "project.register".to_string(),
                description: "Register a project with local path mapping".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "slug": { "type": "string" },
                        "display_name": { "type": "string" },
                        "path": { "type": "string" }
                    },
                    "required": ["slug", "display_name", "path"]
                }),
            },
            // Governance Tools
            ToolDefinition {
                name: "memory.proposals.list".to_string(),
                description: "List pending review proposals for conflict resolution or archiving".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.proposals.decide".to_string(),
                description: "Approve or reject a review proposal".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "approved": { "type": "boolean" }
                    },
                    "required": ["id", "approved"]
                }),
            },
            ToolDefinition {
                name: "memory.redact".to_string(),
                description: "Test secret redaction patterns on sensitive text".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" }
                    },
                    "required": ["text"]
                }),
            },
        ]
    }

    pub fn call_tool(&self, name: &str, arguments: &Value) -> ToolCallResult {
        match name {
            // ==========================================
            // Read Tools
            // ==========================================
            "memory.recall" => {
                let query = match arguments.get("query").and_then(|v| v.as_str()) {
                    Some(q) => q,
                    None => return ToolCallResult::error("Missing required parameter: query"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

                match self.db.search_memories_fts(query, project_id, limit) {
                    Ok(results) => {
                        let formatted: Vec<_> = results
                            .into_iter()
                            .map(|(m, rank)| {
                                json!({
                                    "id": m.id,
                                    "type": m.memory_type.to_string(),
                                    "summary": m.summary,
                                    "content": m.content,
                                    "confidence": m.confidence,
                                    "rank": rank,
                                    "tags": m.tags,
                                })
                            })
                            .collect();
                        ToolCallResult::json(&formatted)
                    }
                    Err(e) => ToolCallResult::error(format!("Search failed: {}", e)),
                }
            }

            "memory.context" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let task_query = arguments.get("task_query").and_then(|v| v.as_str());
                let max_tokens = arguments.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

                let engine = RetrievalEngine::new(&self.db);
                match engine.assemble_context(project_id, task_query, max_tokens) {
                    Ok(ctx) => ToolCallResult::text(ctx),
                    Err(e) => ToolCallResult::error(format!("Context assembly failed: {}", e)),
                }
            }

            "memory.get" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing required parameter: id"),
                };
                match self.db.get_memory_by_id(id) {
                    Ok(Some(mem)) => {
                        let versions = self.db.get_memory_versions(id).unwrap_or_default();
                        ToolCallResult::json(&json!({
                            "memory": mem,
                            "versions": versions,
                        }))
                    }
                    Ok(None) => ToolCallResult::error("Memory not found"),
                    Err(e) => ToolCallResult::error(format!("Failed to get memory: {}", e)),
                }
            }

            "memory.search" => {
                let query = match arguments.get("query").and_then(|v| v.as_str()) {
                    Some(q) => q,
                    None => return ToolCallResult::error("Missing required parameter: query"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

                let engine = RetrievalEngine::new(&self.db);
                match engine.search(query, project_id, limit) {
                    Ok(results) => ToolCallResult::json(&results),
                    Err(e) => ToolCallResult::error(format!("Search error: {}", e)),
                }
            }

            "memory.list" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let status = arguments.get("status").and_then(|v| v.as_str()).and_then(|s| MemoryStatus::from_str(s).ok());
                let mem_type = arguments.get("type").and_then(|v| v.as_str()).and_then(|s| MemoryType::from_str(s).ok());
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

                match self.db.list_memories(project_id, status, mem_type, limit) {
                    Ok(mems) => ToolCallResult::json(&mems),
                    Err(e) => ToolCallResult::error(format!("List memories error: {}", e)),
                }
            }

            "memory.stats" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                match self.db.count_memories(project_id) {
                    Ok((active, archived)) => ToolCallResult::json(&json!({
                        "active_memories": active,
                        "archived_memories": archived,
                        "total": active + archived
                    })),
                    Err(e) => ToolCallResult::error(format!("Stats error: {}", e)),
                }
            }

            "trace.list" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                match self.db.list_traces(project_id, limit) {
                    Ok(traces) => ToolCallResult::json(&traces),
                    Err(e) => ToolCallResult::error(format!("List traces error: {}", e)),
                }
            }

            "trace.get" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                match self.db.get_trace_by_id(id) {
                    Ok(Some(t)) => ToolCallResult::json(&t),
                    Ok(None) => ToolCallResult::error("Trace not found"),
                    Err(e) => ToolCallResult::error(format!("Get trace error: {}", e)),
                }
            }

            "failure.list" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let status = arguments.get("status").and_then(|v| v.as_str());
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                match self.db.list_failures(project_id, status, limit) {
                    Ok(fls) => ToolCallResult::json(&fls),
                    Err(e) => ToolCallResult::error(format!("List failures error: {}", e)),
                }
            }

            "failure.get" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                match self.db.get_failure_by_id(id) {
                    Ok(Some(f)) => ToolCallResult::json(&f),
                    Ok(None) => ToolCallResult::error("Failure record not found"),
                    Err(e) => ToolCallResult::error(format!("Get failure error: {}", e)),
                }
            }

            "project.list" => match self.db.list_projects() {
                Ok(projects) => ToolCallResult::json(&projects),
                Err(e) => ToolCallResult::error(format!("List projects error: {}", e)),
            },

            "project.get" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                match self.db.get_project_by_id(id).or_else(|_| self.db.get_project_by_slug(id)) {
                    Ok(Some(proj)) => {
                        let paths = self.db.get_project_paths(&proj.id).unwrap_or_default();
                        ToolCallResult::json(&json!({
                            "project": proj,
                            "paths": paths
                        }))
                    }
                    Ok(None) => ToolCallResult::error("Project not found"),
                    Err(e) => ToolCallResult::error(format!("Get project error: {}", e)),
                }
            }

            "doc.list" => {
                let project_id = match arguments.get("project_id").and_then(|v| v.as_str()) {
                    Some(pid) => pid,
                    None => return ToolCallResult::error("Missing parameter: project_id"),
                };
                match self.db.list_documents(project_id) {
                    Ok(docs) => ToolCallResult::json(&docs),
                    Err(e) => ToolCallResult::error(format!("List documents error: {}", e)),
                }
            }

            "doc.get" => {
                let project_id = match arguments.get("project_id").and_then(|v| v.as_str()) {
                    Some(pid) => pid,
                    None => return ToolCallResult::error("Missing parameter: project_id"),
                };
                let rel_path = match arguments.get("relative_path").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => return ToolCallResult::error("Missing parameter: relative_path"),
                };
                match self.db.get_document_by_path(project_id, rel_path) {
                    Ok(Some(doc)) => ToolCallResult::json(&doc),
                    Ok(None) => ToolCallResult::error("Tracked document not found"),
                    Err(e) => ToolCallResult::error(format!("Get document error: {}", e)),
                }
            }

            "preference.get" => {
                let key = match arguments.get("key").and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return ToolCallResult::error("Missing parameter: key"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                match self.db.get_preference(key, project_id) {
                    Ok(Some(p)) => ToolCallResult::json(&p),
                    Ok(None) => ToolCallResult::error("Preference not found"),
                    Err(e) => ToolCallResult::error(format!("Get preference error: {}", e)),
                }
            }

            "sync.status" => ToolCallResult::json(&json!({
                "status": "idle",
                "mode": "github",
                "last_synced_at": Utc::now().to_rfc3339(),
                "pending_events": 0
            })),

            // ==========================================
            // Write Tools
            // ==========================================
            "memory.save" => {
                let content = match arguments.get("content").and_then(|v| v.as_str()) {
                    Some(c) => redact_text(c),
                    None => return ToolCallResult::error("Missing parameter: content"),
                };
                let summary = match arguments.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => redact_text(s),
                    None => return ToolCallResult::error("Missing parameter: summary"),
                };
                let mem_type = arguments
                    .get("type")
                    .and_then(|v| v.as_str())
                    .and_then(|s| MemoryType::from_str(s).ok())
                    .unwrap_or(MemoryType::Fact);
                let scope = arguments
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .and_then(|s| MemoryScope::from_str(s).ok())
                    .unwrap_or(MemoryScope::Project);
                let project_id = arguments.get("project_id").and_then(|v| v.as_str()).map(String::from);
                let confidence = arguments.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.9);
                let tags = arguments
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let mem = Memory {
                    id: Uuid::now_v7().to_string(),
                    scope,
                    project_id,
                    memory_type: mem_type,
                    content,
                    summary,
                    confidence,
                    importance: 0.8,
                    freshness: 1.0,
                    status: MemoryStatus::Active,
                    source_kind: MemorySourceKind::Agent,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    last_recalled_at: None,
                    tags,
                    entities: Vec::new(),
                    metadata: json!({}),
                };

                match self.db.save_memory(&mem) {
                    Ok(_) => ToolCallResult::json(&json!({ "saved": true, "id": mem.id })),
                    Err(e) => ToolCallResult::error(format!("Failed to save memory: {}", e)),
                }
            }

            "memory.update" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                match self.db.get_memory_by_id(id) {
                    Ok(Some(mut mem)) => {
                        if let Some(c) = arguments.get("content").and_then(|v| v.as_str()) {
                            mem.content = redact_text(c);
                        }
                        if let Some(s) = arguments.get("summary").and_then(|v| v.as_str()) {
                            mem.summary = redact_text(s);
                        }
                        mem.updated_at = Utc::now();
                        let reason = arguments.get("reason").and_then(|v| v.as_str());
                        match self.db.update_memory(&mem, reason, &[]) {
                            Ok(_) => ToolCallResult::json(&json!({ "updated": true, "id": mem.id })),
                            Err(e) => ToolCallResult::error(format!("Update failed: {}", e)),
                        }
                    }
                    Ok(None) => ToolCallResult::error("Memory not found"),
                    Err(e) => ToolCallResult::error(format!("Get memory error: {}", e)),
                }
            }

            "memory.archive" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                match self.db.archive_memory(id) {
                    Ok(true) => ToolCallResult::json(&json!({ "archived": true, "id": id })),
                    Ok(false) => ToolCallResult::error("Memory not found or already archived"),
                    Err(e) => ToolCallResult::error(format!("Archive error: {}", e)),
                }
            }

            "memory.restore" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                match self.db.restore_memory(id) {
                    Ok(true) => ToolCallResult::json(&json!({ "restored": true, "id": id })),
                    Ok(false) => ToolCallResult::error("Memory not found or already active"),
                    Err(e) => ToolCallResult::error(format!("Restore error: {}", e)),
                }
            }

            "trace.save" => {
                let title = match arguments.get("title").and_then(|v| v.as_str()) {
                    Some(t) => t.to_string(),
                    None => return ToolCallResult::error("Missing parameter: title"),
                };
                let category = match arguments.get("category").and_then(|v| v.as_str()) {
                    Some(c) => c.to_string(),
                    None => return ToolCallResult::error("Missing parameter: category"),
                };
                let steps: Vec<TraceStep> = match arguments.get("steps") {
                    Some(s) => serde_json::from_value(s.clone()).unwrap_or_default(),
                    None => return ToolCallResult::error("Missing parameter: steps"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str()).map(String::from);
                let preconditions: Vec<String> = arguments
                    .get("preconditions")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let caveats: Vec<String> = arguments
                    .get("caveats")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let reproducibility_score = arguments.get("reproducibility_score").and_then(|v| v.as_f64()).unwrap_or(0.9);

                let trace = SuccessTrace {
                    id: Uuid::now_v7().to_string(),
                    project_id,
                    title,
                    category,
                    status: "active".to_string(),
                    preconditions,
                    steps,
                    reproducibility_score,
                    caveats,
                    source_task_id: None,
                    evidence_event_ids: Vec::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                match self.db.save_trace(&trace) {
                    Ok(_) => ToolCallResult::json(&json!({ "saved": true, "id": trace.id })),
                    Err(e) => ToolCallResult::error(format!("Failed to save trace: {}", e)),
                }
            }

            "failure.save" => {
                let task_title = match arguments.get("task_title").and_then(|v| v.as_str()) {
                    Some(t) => t.to_string(),
                    None => return ToolCallResult::error("Missing parameter: task_title"),
                };
                let error_message = match arguments.get("error_message").and_then(|v| v.as_str()) {
                    Some(e) => redact_text(e),
                    None => return ToolCallResult::error("Missing parameter: error_message"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str()).map(String::from);
                let failed_tool = arguments.get("failed_tool").and_then(|v| v.as_str()).map(String::from);
                let root_cause_hypothesis = arguments.get("root_cause_hypothesis").and_then(|v| v.as_str()).map(String::from);
                let fix_applied = arguments.get("fix_applied").and_then(|v| v.as_str()).map(String::from);
                let verification_result = arguments.get("verification_result").and_then(|v| v.as_str()).map(String::from);

                let failure = FailureRecord {
                    id: Uuid::now_v7().to_string(),
                    project_id,
                    task_title,
                    category: Some("runtime_error".to_string()),
                    failed_tool,
                    error_message,
                    root_cause_hypothesis,
                    fix_applied,
                    verification_result,
                    status: "unresolved".to_string(),
                    evidence_event_ids: Vec::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                match self.db.save_failure(&failure) {
                    Ok(_) => ToolCallResult::json(&json!({ "saved": true, "id": failure.id })),
                    Err(e) => ToolCallResult::error(format!("Failed to save failure: {}", e)),
                }
            }

            "failure.resolve" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                let fix = match arguments.get("fix_applied").and_then(|v| v.as_str()) {
                    Some(f) => f,
                    None => return ToolCallResult::error("Missing parameter: fix_applied"),
                };
                let verification = match arguments.get("verification_result").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => return ToolCallResult::error("Missing parameter: verification_result"),
                };

                match self.db.resolve_failure(id, fix, verification) {
                    Ok(true) => ToolCallResult::json(&json!({ "resolved": true, "id": id })),
                    Ok(false) => ToolCallResult::error("Failure record not found"),
                    Err(e) => ToolCallResult::error(format!("Failed to resolve failure: {}", e)),
                }
            }

            "preference.set" => {
                let key = match arguments.get("key").and_then(|v| v.as_str()) {
                    Some(k) => k.to_string(),
                    None => return ToolCallResult::error("Missing parameter: key"),
                };
                let value = match arguments.get("value") {
                    Some(v) => v.clone(),
                    None => return ToolCallResult::error("Missing parameter: value"),
                };
                let scope = arguments
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .and_then(|s| MemoryScope::from_str(s).ok())
                    .unwrap_or(MemoryScope::Global);
                let project_id = arguments.get("project_id").and_then(|v| v.as_str()).map(String::from);

                let pref = Preference {
                    id: Uuid::now_v7().to_string(),
                    key: key.clone(),
                    value,
                    scope,
                    project_id,
                    locked: false,
                    source: "agent".to_string(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                match self.db.set_preference(&pref) {
                    Ok(_) => ToolCallResult::json(&json!({ "saved": true, "key": key })),
                    Err(e) => ToolCallResult::error(format!("Failed to set preference: {}", e)),
                }
            }

            "preference.lock" => {
                let key = match arguments.get("key").and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => return ToolCallResult::error("Missing parameter: key"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let locked = arguments.get("locked").and_then(|v| v.as_bool()).unwrap_or(true);

                match self.db.lock_preference(key, project_id, locked) {
                    Ok(true) => ToolCallResult::json(&json!({ "locked": locked, "key": key })),
                    Ok(false) => ToolCallResult::error("Preference not found to lock"),
                    Err(e) => ToolCallResult::error(format!("Failed to lock preference: {}", e)),
                }
            }

            "project.register" => {
                let slug = match arguments.get("slug").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return ToolCallResult::error("Missing parameter: slug"),
                };
                let display_name = match arguments.get("display_name").and_then(|v| v.as_str()) {
                    Some(d) => d.to_string(),
                    None => return ToolCallResult::error("Missing parameter: display_name"),
                };
                let path = match arguments.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => return ToolCallResult::error("Missing parameter: path"),
                };

                let proj = Project {
                    id: Uuid::now_v7().to_string(),
                    slug: slug.clone(),
                    display_name,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    capture_policy: CapturePolicy::default(),
                    context_budget: ContextBudget::default(),
                    document_config: DocumentConfig::default(),
                    sync_config: SyncConfig::default(),
                };

                if let Err(e) = self.db.save_project(&proj) {
                    return ToolCallResult::error(format!("Failed to register project: {}", e));
                }
                if let Err(e) = self.db.add_project_path(&proj.id, &path, true) {
                    return ToolCallResult::error(format!("Failed to add project path: {}", e));
                }

                ToolCallResult::json(&json!({ "registered": true, "id": proj.id, "slug": slug }))
            }

            // ==========================================
            // Governance Tools
            // ==========================================
            "memory.proposals.list" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                match self.db.list_pending_proposals(project_id) {
                    Ok(props) => ToolCallResult::json(&props),
                    Err(e) => ToolCallResult::error(format!("Failed to list proposals: {}", e)),
                }
            }

            "memory.proposals.decide" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: id"),
                };
                let approved = match arguments.get("approved").and_then(|v| v.as_bool()) {
                    Some(a) => a,
                    None => return ToolCallResult::error("Missing parameter: approved"),
                };
                match self.db.decide_proposal(id, approved) {
                    Ok(true) => ToolCallResult::json(&json!({ "decided": true, "id": id, "approved": approved })),
                    Ok(false) => ToolCallResult::error("Proposal not found"),
                    Err(e) => ToolCallResult::error(format!("Decide proposal error: {}", e)),
                }
            }

            "memory.redact" => {
                let text = match arguments.get("text").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return ToolCallResult::error("Missing parameter: text"),
                };
                let redacted = redact_text(text);
                ToolCallResult::json(&json!({ "redacted": redacted }))
            }

            unknown => ToolCallResult::error(format!("Unknown tool: {}", unknown)),
        }
    }
}
