use crate::protocol::{ToolCallResult, ToolDefinition};
use automemory_core::db::Database;
use automemory_core::graph::GraphBuilder;
use automemory_core::models::*;
use automemory_core::ports::CodeIntel;
use automemory_core::redaction::redact_text;
use automemory_core::retrieval::RetrievalEngine;
use automemory_core::verify::{DecayVerifier, Verifier};
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
                name: "memory.ask".to_string(),
                description: "PRD alias for memory.recall: ranked short entries with ids, status and provenance".to_string(),
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
                name: "memory.about".to_string(),
                description: "Everything known about a file or symbol: graph entities, callers, and linked memories".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string", "description": "File path or symbol name" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["target"]
                }),
            },
            ToolDefinition {
                name: "memory.topics".to_string(),
                description: "What is queryable: active decision topics plus distinct memory tags".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.propose".to_string(),
                description: "Write a candidate memory. Rejected without provenance (evidence)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "content": { "type": "string", "description": "Detailed explanation" },
                        "summary": { "type": "string", "description": "Concise 1-line summary" },
                        "evidence": { "type": "array", "items": { "type": "string" }, "description": "Provenance: memory/trace/failure IDs or references (required)" },
                        "type": { "type": "string", "enum": ["fact", "decision", "lesson", "pattern", "warning", "observation", "preference", "skill"] },
                        "project_id": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["content", "summary", "evidence"]
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
                name: "memory.digest".to_string(),
                description: "Generate an activity and progress digest for a specific time span (e.g. '15m', '30m', '1h', '2h', '4h', 'today', 'session'), including editing metrics, files modified, achievements, and failures.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Optional project ID or slug" },
                        "span": { "type": "string", "description": "Time span window: '15m', '30m', '1h', '2h', '4h', '24h', 'today', or 'session' (default '1h')" },
                        "session_id": { "type": "string", "description": "Session ID (used if span is 'session')" },
                        "auto_save": { "type": "boolean", "description": "Whether to automatically save this digest as a durable checkpoint milestone memory (default false)" }
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
                        "category": { "type": "string", "description": "Domain category: architecture, decision, feature, bugfix, lesson, milestone" },
                        "achievements": { "type": "array", "items": { "type": "string" }, "description": "List of milestones achieved" },
                        "failures_resolved": { "type": "array", "items": { "type": "string" }, "description": "List of errors diagnosed and resolved" },
                        "files_modified": { "type": "array", "items": { "type": "string" }, "description": "Codebase files affected" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                        "metadata": { "type": "object", "description": "Arbitrary extra metadata" }
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
                description: "List review proposals with optional status, type, and pagination filters".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "status": { "type": "string", "enum": ["pending", "approved", "rejected", "all"] },
                        "proposal_type": { "type": "string" },
                        "limit": { "type": "integer" },
                        "offset": { "type": "integer" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.proposals.summary".to_string(),
                description: "Get aggregate summary and counts of proposals grouped by status and proposal type".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.proposals.decide".to_string(),
                description: "Approve or reject a review proposal (automatically executes proposal side-effects such as archiving duplicate memories)".to_string(),
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
                name: "memory.proposals.decide_all".to_string(),
                description: "Atomically approve or reject all pending proposals matching optional project and proposal_type filters (automatically executes side-effects)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "proposal_type": { "type": "string" },
                        "approved": { "type": "boolean" }
                    },
                    "required": ["approved"]
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
            // Knowledge Graph Tools
            ToolDefinition {
                name: "graph.index".to_string(),
                description: "Scan a project directory and index code structural entities and relations into the knowledge graph".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Project ID or slug" },
                        "path": { "type": "string", "description": "Local workspace root directory path" }
                    },
                    "required": ["project_id", "path"]
                }),
            },
            ToolDefinition {
                name: "graph.query".to_string(),
                description: "Query codebase structural knowledge graph entities (files, functions, structs, docs)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "entity_type": { "type": "string", "enum": ["file", "function", "struct", "class", "module", "trait", "doc"] },
                        "name_pattern": { "type": "string", "description": "Substring match on symbol name" },
                        "path_prefix": { "type": "string", "description": "Path prefix filter" },
                        "limit": { "type": "integer", "description": "Max results (default 20)" }
                    }
                }),
            },
            ToolDefinition {
                name: "graph.neighbors".to_string(),
                description: "Get incoming and outgoing relationship edges and neighbor entities for a specific entity ID".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "string", "description": "Target entity ID" }
                    },
                    "required": ["entity_id"]
                }),
            },
            ToolDefinition {
                name: "graph.impact".to_string(),
                description: "Perform blast-radius impact analysis for changing a file or symbol, tracing dependent callers and files".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string", "description": "Symbol name or relative file path to analyze" },
                        "project_id": { "type": "string", "description": "Optional project ID" },
                        "depth": { "type": "integer", "description": "Max traversal depth (default 3)" }
                    },
                    "required": ["target"]
                }),
            },
            ToolDefinition {
                name: "memory.skills".to_string(),
                description: "List auto-distilled procedural skill playbooks synthesized from verified success traces".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Optional project ID" }
                    }
                }),
            },
            ToolDefinition {
                name: "memory.constitution".to_string(),
                description: "Retrieve token-capped Hot Memory Constitution containing core rules, architecture invariants, and gotchas".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Optional project ID" }
                    }
                }),
            },
            ToolDefinition {
                name: "lock.acquire".to_string(),
                description: "Acquire or renew a cooperative multi-agent lease on a file, glob pattern, or symbol to prevent collisions".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "scope": { "type": "string", "description": "Target resource: file path (e.g. 'src/auth/session.ts'), glob ('src/models/*.rs'), or symbol ('auth::login')" },
                        "goal": { "type": "string", "description": "Clear explanation of what is being modified and why" },
                        "scope_type": { "type": "string", "enum": ["file", "glob", "symbol"], "description": "Scope granularity (default 'file')" },
                        "ttl_seconds": { "type": "integer", "description": "Lease duration in seconds (default 300)" },
                        "agent": { "type": "string", "description": "Agent name or harness (e.g. 'claude-code', 'antigravity')" },
                        "session_id": { "type": "string", "description": "Unique session identifier (default 'default-session')" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["scope", "goal"]
                }),
            },
            ToolDefinition {
                name: "lock.release".to_string(),
                description: "Release a held resource lease, or all leases held by a session".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Session ID that holds the lease" },
                        "scope": { "type": "string", "description": "Specific scope to release (if omitted, all leases held by session are released)" }
                    },
                    "required": ["session_id"]
                }),
            },
            ToolDefinition {
                name: "lock.status".to_string(),
                description: "List currently active multi-agent resource leases and their remaining TTLs".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    }
                }),
            },
            ToolDefinition {
                name: "decision.record".to_string(),
                description: "Record an architectural decision with rationale, alternatives, and dissent. Supersedes prior active decision on the same topic while preserving history.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "topic": { "type": "string", "description": "Subject or architectural topic (e.g. 'auth', 'database', 'caching')" },
                        "decision": { "type": "string", "description": "The exact decision adopted" },
                        "reason": { "type": "string", "description": "Technical rationale and justification" },
                        "alternatives": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Considered alternatives that were rejected"
                        },
                        "dissent": { "type": "string", "description": "Documented counterarguments or dissenting view points" },
                        "agent": { "type": "string", "description": "Agent or user recording the decision" },
                        "symbols": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Affected symbols or modules (e.g. ['auth::jwt', 'api::login'])"
                        },
                        "evidence": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Supporting evidence IDs or ADR references"
                        },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["topic", "decision", "reason"]
                }),
            },
            ToolDefinition {
                name: "decision.get".to_string(),
                description: "Fetch the active architectural decision for a topic along with full history and preserved dissent".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "topic": { "type": "string", "description": "Subject or architectural topic (e.g. 'auth')" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["topic"]
                }),
            },
            ToolDefinition {
                name: "baton.pass".to_string(),
                description: "Hand off work to another agent session: builds a sub-400-token packet from ledger facts (tool calls, files, leases, decisions, blockers) and stores it for pickup".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Passing session ID (source)" },
                        "to_session": { "type": "string", "description": "Receiving session ID (omit to broadcast)" },
                        "note": { "type": "string", "description": "Next step for the receiving agent" },
                        "agent": { "type": "string", "description": "Passing agent name" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["session_id"]
                }),
            },
            ToolDefinition {
                name: "baton.pickup".to_string(),
                description: "Pick up the latest handoff packet for a session (direct first, then broadcast)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Receiving session ID" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    }
                }),
            },
            ToolDefinition {
                name: "task.create".to_string(),
                description: "Enqueue a work item with spec and symbol scope for agents to claim".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Creating session ID" },
                        "title": { "type": "string", "description": "Short task title" },
                        "spec": { "type": "string", "description": "What needs doing" },
                        "symbols": { "type": "array", "items": { "type": "string" }, "description": "Affected symbols or modules" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["session_id", "title"]
                }),
            },
            ToolDefinition {
                name: "task.next".to_string(),
                description: "Claim the oldest open work item (FIFO), or resume the one this session already holds".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Claiming session ID" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["session_id"]
                }),
            },
            ToolDefinition {
                name: "task.submit".to_string(),
                description: "Hand claimed work to review with a result note. Only the owning session can submit".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Task ID" },
                        "session_id": { "type": "string", "description": "Owning session ID" },
                        "result": { "type": "string", "description": "What was done / review notes" }
                    },
                    "required": ["id", "session_id"]
                }),
            },
            ToolDefinition {
                name: "review.submit".to_string(),
                description: "Queue a diff for cross-review by a different agent session".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Authoring session ID" },
                        "diff": { "type": "string", "description": "Unified diff under review" },
                        "title": { "type": "string", "description": "Short review title" },
                        "test_command": { "type": "string", "description": "Command the reviewer runs to judge it" },
                        "task_id": { "type": "string", "description": "Source task ID" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["session_id", "diff"]
                }),
            },
            ToolDefinition {
                name: "review.next".to_string(),
                description: "Fetch the oldest pending diff authored by another session".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Reviewing session ID" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    }
                }),
            },
            ToolDefinition {
                name: "review.post".to_string(),
                description: "Post a review verdict. Failing tests reject regardless of approval vote".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Review ID" },
                        "session_id": { "type": "string", "description": "Reviewing session ID" },
                        "approved": { "type": "boolean", "description": "Reviewer vote" },
                        "tests_passed": { "type": "boolean", "description": "Test run outcome (judges the diff)" },
                        "verdict": { "type": "string", "description": "Review notes" }
                    },
                    "required": ["id", "session_id", "tests_passed"]
                }),
            },
            ToolDefinition {
                name: "room.pending".to_string(),
                description: "List planning rooms awaiting action (turn unheld or held by this session)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Session asking for its turns" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    }
                }),
            },
            ToolDefinition {
                name: "room.join".to_string(),
                description: "Join a planning room, or create one on a topic when room_id is omitted".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Joining session ID" },
                        "room_id": { "type": "string", "description": "Room to join (omit to create)" },
                        "topic": { "type": "string", "description": "Topic for a new room" },
                        "project_id": { "type": "string", "description": "Optional project ID or slug" }
                    },
                    "required": ["session_id"]
                }),
            },
            ToolDefinition {
                name: "room.post".to_string(),
                description: "Post one turn (propose/critique/revise). Turn holder only; evidence IDs must resolve".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": { "type": "string", "description": "Room ID" },
                        "session_id": { "type": "string", "description": "Posting session ID" },
                        "kind": { "type": "string", "enum": ["propose", "critique", "revise"] },
                        "body": { "type": "string", "description": "Short rationale (capped, never raw reasoning)" },
                        "evidence": { "type": "array", "items": { "type": "string" }, "description": "Memory/trace/failure IDs backing the post" }
                    },
                    "required": ["room_id", "session_id", "kind", "body"]
                }),
            },
            ToolDefinition {
                name: "room.status".to_string(),
                description: "Fetch a room with its posts in order".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": { "type": "string", "description": "Room ID" }
                    },
                    "required": ["room_id"]
                }),
            },
        ]
    }

    pub fn call_tool(&self, name: &str, arguments: &Value) -> ToolCallResult {
        match name {
            // ==========================================
            // Read Tools
            // ==========================================
            "memory.recall" | "memory.ask" => {
                let query = match arguments.get("query").and_then(|v| v.as_str()) {
                    Some(q) => q,
                    None => return ToolCallResult::error("Missing required parameter: query"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

                match self.db.search_memories_fts(query, project_id, limit) {
                    Ok(results) => {
                        let verifier = DecayVerifier::default();
                        let formatted: Vec<_> = results
                            .into_iter()
                            .map(|(m, rank)| {
                                let verify_status = verifier.verify(&m).to_string();
                                json!({
                                    "id": m.id,
                                    "type": m.memory_type.to_string(),
                                    "trust_tier": m.trust_tier().as_u8(),
                                    "status": m.status.to_string(),
                                    "verify_status": verify_status,
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

            "memory.about" => {
                let target = match arguments.get("target").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return ToolCallResult::error("Missing required parameter: target"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());

                let by_name = self
                    .db
                    .query_graph_entities(project_id, None, Some(target), None, 10)
                    .unwrap_or_default();
                let by_path = self
                    .db
                    .query_graph_entities(project_id, None, None, Some(target), 10)
                    .unwrap_or_default();
                let mut seen = std::collections::HashSet::new();
                let entities: Vec<_> = by_name
                    .into_iter()
                    .chain(by_path)
                    .filter(|e| seen.insert(e.id.clone()))
                    .collect();
                let intel = automemory_core::ports::GraphCodeIntel::new(&self.db);
                let callers = intel.callers(project_id, target);
                let memories = self
                    .db
                    .search_memories_fts(target, project_id, 5)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(m, _)| {
                        json!({ "id": m.id, "summary": m.summary, "status": m.status.to_string() })
                    })
                    .collect::<Vec<_>>();
                ToolCallResult::json(&json!({
                    "target": target,
                    "entities": entities,
                    "callers": callers,
                    "memories": memories,
                    "next_action": "Use memory.get(id) for full detail on any entry."
                }))
            }

            "memory.topics" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                match self.db.list_topics(project_id, 50) {
                    Ok(topics) => ToolCallResult::json(&json!({
                        "topics": topics,
                        "next_action": "Query any topic with memory.ask."
                    })),
                    Err(e) => ToolCallResult::error(format!("Failed to list topics: {}", e)),
                }
            }

            "memory.propose" => {
                let content = match arguments.get("content").and_then(|v| v.as_str()) {
                    Some(c) => redact_text(c),
                    None => return ToolCallResult::error("Missing parameter: content"),
                };
                let summary = match arguments.get("summary").and_then(|v| v.as_str()) {
                    Some(s) => redact_text(s),
                    None => return ToolCallResult::error("Missing parameter: summary"),
                };
                let evidence: Vec<String> = match arguments
                    .get("evidence")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect::<Vec<String>>()
                    }) {
                    Some(list) if !list.is_empty() => list,
                    _ => {
                        return ToolCallResult::error(
                            "Proposal rejected without provenance: provide evidence",
                        )
                    }
                };
                let mem_type = arguments
                    .get("type")
                    .and_then(|v| v.as_str())
                    .and_then(|s| MemoryType::from_str(s).ok())
                    .unwrap_or(MemoryType::Fact);
                let project_id = arguments
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let tags = arguments
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let mem = Memory {
                    id: Uuid::now_v7().to_string(),
                    scope: MemoryScope::Project,
                    project_id,
                    memory_type: mem_type,
                    content,
                    summary,
                    confidence: 0.5,
                    importance: 0.8,
                    freshness: 1.0,
                    status: MemoryStatus::Candidate,
                    source_kind: MemorySourceKind::Agent,
                    version: Some(1),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    last_recalled_at: None,
                    tags,
                    entities: Vec::new(),
                    metadata: json!({ "evidence": evidence }),
                };
                match self.db.save_memory_direct(&mem) {
                    Ok(_) => ToolCallResult::json(&json!({
                        "proposed": true,
                        "id": mem.id,
                        "next_action": "Candidate queued; a human confirms via proposal review, pin, or veto."
                    })),
                    Err(e) => ToolCallResult::error(format!("Failed to propose memory: {}", e)),
                }
            }

            "memory.context" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let task_query = arguments.get("task_query").and_then(|v| v.as_str());
                let max_tokens = arguments
                    .get("max_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2000) as usize;

                let engine = RetrievalEngine::new(&self.db);
                match engine.assemble_context(project_id, task_query, max_tokens) {
                    Ok(ctx) => ToolCallResult::text(ctx),
                    Err(e) => ToolCallResult::error(format!("Context assembly failed: {}", e)),
                }
            }

            "memory.skills" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let distiller = automemory_core::extraction::SkillDistiller::new(&self.db);
                match distiller.distill_skills(project_id) {
                    Ok(skills) => ToolCallResult::json(&skills),
                    Err(e) => ToolCallResult::error(format!("Skill distillation failed: {}", e)),
                }
            }

            "memory.constitution" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let engine = RetrievalEngine::new(&self.db);
                match engine.generate_constitution(project_id) {
                    Ok(const_doc) => ToolCallResult::text(const_doc),
                    Err(e) => {
                        ToolCallResult::error(format!("Constitution generation failed: {}", e))
                    }
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
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as usize;

                let engine = RetrievalEngine::new(&self.db);
                match engine.search(query, project_id, limit) {
                    Ok(results) => ToolCallResult::json(&results),
                    Err(e) => ToolCallResult::error(format!("Search error: {}", e)),
                }
            }

            "memory.list" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let status = arguments
                    .get("status")
                    .and_then(|v| v.as_str())
                    .and_then(|s| MemoryStatus::from_str(s).ok());
                let mem_type = arguments
                    .get("type")
                    .and_then(|v| v.as_str())
                    .and_then(|s| MemoryType::from_str(s).ok());
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                match self.db.list_memories(project_id, status, mem_type, limit) {
                    Ok(mems) => ToolCallResult::json(&mems),
                    Err(e) => ToolCallResult::error(format!("List memories error: {}", e)),
                }
            }

            "memory.stats" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let (active, archived) = self.db.count_memories(project_id).unwrap_or((0, 0));
                let prop_summary = self.db.get_proposals_summary(project_id).ok();
                let failures = self
                    .db
                    .list_failures(project_id, None, 1000)
                    .unwrap_or_default();
                let traces = self.db.list_traces(project_id, 1000).unwrap_or_default();
                ToolCallResult::json(&json!({
                    "active_memories": active,
                    "archived_memories": archived,
                    "total_memories": active + archived,
                    "failures_count": failures.len(),
                    "traces_count": traces.len(),
                    "proposals": prop_summary
                }))
            }

            "memory.digest" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let span_str = arguments
                    .get("span")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1h");
                let auto_save = arguments
                    .get("auto_save")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let engine = automemory_core::extraction::TimeSpanDigestEngine::new(&self.db);
                let digest_res = if span_str.eq_ignore_ascii_case("session") {
                    if let Some(session_id) = arguments.get("session_id").and_then(|v| v.as_str()) {
                        engine.generate_session_digest(session_id)
                    } else {
                        return ToolCallResult::error(
                            "Missing parameter: session_id is required when span is 'session'",
                        );
                    }
                } else {
                    match automemory_core::models::timespan::parse_time_span_str(span_str) {
                        Ok((start, end)) => engine.generate_digest(project_id, start, end),
                        Err(e) => {
                            return ToolCallResult::error(format!("Invalid span format: {}", e))
                        }
                    }
                };

                match digest_res {
                    Ok(digest) => {
                        let mut saved_memory_id = None;
                        if auto_save {
                            if let Ok(mem) = engine.synthesize_milestone_memory(&digest, None) {
                                if self.db.save_memory(&mem).is_ok() {
                                    saved_memory_id = Some(mem.id);
                                }
                            }
                        }
                        ToolCallResult::json(&json!({
                            "digest": digest,
                            "executive_summary": digest.executive_summary,
                            "saved_memory_id": saved_memory_id
                        }))
                    }
                    Err(e) => {
                        ToolCallResult::error(format!("Failed to generate time span digest: {}", e))
                    }
                }
            }

            "trace.list" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;
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
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;
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
                match self
                    .db
                    .get_project_by_id(id)
                    .or_else(|_| self.db.get_project_by_slug(id))
                {
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
                let project_id = arguments
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let confidence = arguments
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.9);
                let tags = arguments
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut metadata_map = serde_json::Map::new();
                if let Some(cat) = arguments.get("category").and_then(|v| v.as_str()) {
                    metadata_map.insert("category".to_string(), json!(cat));
                }
                if let Some(ach) = arguments.get("achievements") {
                    metadata_map.insert("achievements".to_string(), ach.clone());
                }
                if let Some(fail) = arguments.get("failures_resolved") {
                    metadata_map.insert("failures_resolved".to_string(), fail.clone());
                }
                if let Some(fls) = arguments.get("files_modified") {
                    metadata_map.insert("files_modified".to_string(), fls.clone());
                }
                if let Some(custom_meta) = arguments.get("metadata").and_then(|v| v.as_object()) {
                    for (k, v) in custom_meta {
                        metadata_map.insert(k.clone(), v.clone());
                    }
                }

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
                    version: Some(1),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    last_recalled_at: None,
                    tags,
                    entities: Vec::new(),
                    metadata: Value::Object(metadata_map),
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
                            Ok(_) => {
                                ToolCallResult::json(&json!({ "updated": true, "id": mem.id }))
                            }
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
                let project_id = arguments
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let preconditions: Vec<String> = arguments
                    .get("preconditions")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let caveats: Vec<String> = arguments
                    .get("caveats")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let reproducibility_score = arguments
                    .get("reproducibility_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.9);

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
                let project_id = arguments
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let failed_tool = arguments
                    .get("failed_tool")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let root_cause_hypothesis = arguments
                    .get("root_cause_hypothesis")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let fix_applied = arguments
                    .get("fix_applied")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let verification_result = arguments
                    .get("verification_result")
                    .and_then(|v| v.as_str())
                    .map(String::from);

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
                let verification = match arguments
                    .get("verification_result")
                    .and_then(|v| v.as_str())
                {
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
                let project_id = arguments
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);

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
                let locked = arguments
                    .get("locked")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

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
                let status = arguments.get("status").and_then(|v| v.as_str());
                let proposal_type = arguments.get("proposal_type").and_then(|v| v.as_str());
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                let offset = arguments
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);
                match self
                    .db
                    .list_proposals(project_id, status, proposal_type, limit, offset)
                {
                    Ok(props) => ToolCallResult::json(&props),
                    Err(e) => ToolCallResult::error(format!("Failed to list proposals: {}", e)),
                }
            }

            "memory.proposals.summary" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                match self.db.get_proposals_summary(project_id) {
                    Ok(summary) => ToolCallResult::json(&summary),
                    Err(e) => {
                        ToolCallResult::error(format!("Failed to get proposals summary: {}", e))
                    }
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
                    Ok(true) => ToolCallResult::json(
                        &json!({ "decided": true, "id": id, "approved": approved }),
                    ),
                    Ok(false) => ToolCallResult::error("Proposal not found"),
                    Err(e) => ToolCallResult::error(format!("Decide proposal error: {}", e)),
                }
            }

            "memory.proposals.decide_all" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let proposal_type = arguments.get("proposal_type").and_then(|v| v.as_str());
                let approved = match arguments.get("approved").and_then(|v| v.as_bool()) {
                    Some(a) => a,
                    None => return ToolCallResult::error("Missing parameter: approved"),
                };
                match self
                    .db
                    .decide_all_proposals(project_id, proposal_type, approved)
                {
                    Ok(batch) => ToolCallResult::json(&batch),
                    Err(e) => ToolCallResult::error(format!("Decide all proposals error: {}", e)),
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

            // ==========================================
            // Knowledge Graph Tools
            // ==========================================
            "graph.index" => {
                let project_id = match arguments.get("project_id").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => return ToolCallResult::error("Missing parameter: project_id"),
                };
                let path = match arguments.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => return ToolCallResult::error("Missing parameter: path"),
                };

                let builder = GraphBuilder::new(&self.db);
                match builder.scan_and_index_project(project_id, path) {
                    Ok((entities_count, edges_count)) => ToolCallResult::json(&json!({
                        "indexed": true,
                        "project_id": project_id,
                        "entities_indexed": entities_count,
                        "edges_indexed": edges_count,
                    })),
                    Err(e) => ToolCallResult::error(format!("Graph indexing failed: {}", e)),
                }
            }

            "graph.query" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let entity_type = arguments.get("entity_type").and_then(|v| v.as_str());
                let name_pattern = arguments.get("name_pattern").and_then(|v| v.as_str());
                let path_prefix = arguments.get("path_prefix").and_then(|v| v.as_str());
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                match self.db.query_graph_entities(
                    project_id,
                    entity_type,
                    name_pattern,
                    path_prefix,
                    limit,
                ) {
                    Ok(entities) => ToolCallResult::json(&entities),
                    Err(e) => ToolCallResult::error(format!("Graph query failed: {}", e)),
                }
            }

            "graph.neighbors" => {
                let entity_id = match arguments.get("entity_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return ToolCallResult::error("Missing parameter: entity_id"),
                };

                let outgoing = self
                    .db
                    .get_outgoing_graph_edges(entity_id)
                    .unwrap_or_default();
                let incoming = self
                    .db
                    .get_incoming_graph_edges(entity_id)
                    .unwrap_or_default();

                ToolCallResult::json(&json!({
                    "entity_id": entity_id,
                    "outgoing_edges": outgoing,
                    "incoming_edges": incoming,
                }))
            }

            "graph.impact" => {
                let target = match arguments.get("target").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return ToolCallResult::error("Missing parameter: target"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let depth = arguments.get("depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

                let builder = GraphBuilder::new(&self.db);
                match builder.trace_impact(project_id, target, depth) {
                    Ok(Some(report)) => ToolCallResult::json(&report),
                    Ok(None) => ToolCallResult::error(format!(
                        "Target symbol or file '{}' not found in graph",
                        target
                    )),
                    Err(e) => ToolCallResult::error(format!("Impact analysis failed: {}", e)),
                }
            }

            // ==========================================
            // Multi-Agent Lock & Lease Coordination Tools
            // ==========================================
            "lock.acquire" => {
                let scope = match arguments.get("scope").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: scope"),
                };
                let goal = match arguments.get("goal").and_then(|v| v.as_str()) {
                    Some(g) => g,
                    None => return ToolCallResult::error("Missing required parameter: goal"),
                };
                let scope_type = arguments
                    .get("scope_type")
                    .and_then(|v| v.as_str())
                    .map(LeaseScopeType::from)
                    .unwrap_or(LeaseScopeType::File);
                let ttl_seconds = arguments
                    .get("ttl_seconds")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(300);
                let agent = arguments
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent");
                let session_id = arguments
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default-session");
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());

                match self.db.acquire_lease(LeaseAcquireArgs {
                    project_id,
                    agent,
                    session_id,
                    scope,
                    scope_type,
                    goal,
                    ttl_seconds,
                }) {
                    Ok(LeaseAcquireResult::Acquired { lease }) => {
                        let result_payload = json!({
                            "status": "granted",
                            "lease_id": lease.id,
                            "scope": lease.scope,
                            "scope_type": lease.scope_type.to_string(),
                            "ttl_seconds": lease.ttl_seconds,
                            "expires_at": lease.expires_at.to_rfc3339(),
                            "next_action": format!("Lease granted on '{}'. Proceed with edits, then call lock.release when finished.", lease.scope)
                        });
                        ToolCallResult::json(&result_payload)
                    }
                    Ok(LeaseAcquireResult::Renewed { lease }) => {
                        let result_payload = json!({
                            "status": "renewed",
                            "lease_id": lease.id,
                            "scope": lease.scope,
                            "ttl_seconds": lease.ttl_seconds,
                            "expires_at": lease.expires_at.to_rfc3339(),
                            "next_action": format!("Lease renewed on '{}'. Continue your task.", lease.scope)
                        });
                        ToolCallResult::json(&result_payload)
                    }
                    Ok(LeaseAcquireResult::Conflict { conflict }) => {
                        let result_payload = json!({
                            "status": "conflict",
                            "message": format!("Resource '{}' is currently leased by another agent.", conflict.scope),
                            "conflict": {
                                "holder_agent": conflict.holder_agent,
                                "holder_session_id": conflict.holder_session_id,
                                "goal": conflict.goal,
                                "remaining_seconds": conflict.remaining_seconds,
                                "expires_at": conflict.expires_at.to_rfc3339(),
                            },
                            "next_action": format!("Wait for lease to expire ({}s remaining), or switch to another task.", conflict.remaining_seconds)
                        });
                        ToolCallResult::json(&result_payload)
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to acquire lease: {}", e)),
                }
            }

            "lock.release" => {
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let scope = arguments.get("scope").and_then(|v| v.as_str());

                match self.db.release_leases(session_id, scope) {
                    Ok(count) => {
                        let result_payload = json!({
                            "status": "released",
                            "released_count": count,
                            "scope": scope,
                            "next_action": "Lease released. Other agents may now modify this resource."
                        });
                        ToolCallResult::json(&result_payload)
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to release lease: {}", e)),
                }
            }

            "lock.status" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());

                match self.db.list_active_leases(project_id) {
                    Ok(leases) => {
                        let now = Utc::now();
                        let items: Vec<_> = leases
                            .into_iter()
                            .map(|l| {
                                let rem = l.remaining_seconds(now);
                                json!({
                                    "id": l.id,
                                    "scope": l.scope,
                                    "scope_type": l.scope_type.to_string(),
                                    "agent": l.agent,
                                    "session_id": l.session_id,
                                    "goal": l.goal,
                                    "remaining_seconds": rem,
                                    "expires_at": l.expires_at.to_rfc3339(),
                                })
                            })
                            .collect();

                        let result_payload = json!({
                            "active_leases": items,
                            "count": items.len(),
                            "next_action": "Use lock.acquire to lease a file/symbol before modifying it, or lock.release to yield."
                        });
                        ToolCallResult::json(&result_payload)
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to list active leases: {}", e)),
                }
            }

            "decision.record" => {
                let topic = match arguments.get("topic").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return ToolCallResult::error("Missing required parameter: topic"),
                };
                let decision = match arguments.get("decision").and_then(|v| v.as_str()) {
                    Some(d) => d,
                    None => return ToolCallResult::error("Missing required parameter: decision"),
                };
                let reason = match arguments.get("reason").and_then(|v| v.as_str()) {
                    Some(r) => r,
                    None => return ToolCallResult::error("Missing required parameter: reason"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let agent = arguments
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent");
                let dissent = arguments.get("dissent").and_then(|v| v.as_str());

                let alternatives: Vec<String> = arguments
                    .get("alternatives")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                let symbols: Vec<String> = arguments
                    .get("symbols")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                let evidence: Vec<String> = arguments
                    .get("evidence")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                match self.db.record_decision(DecisionRecordArgs {
                    project_id,
                    topic,
                    decision,
                    reason,
                    alternatives: &alternatives,
                    dissent,
                    agent,
                    symbols: &symbols,
                    evidence: &evidence,
                }) {
                    Ok(rec) => {
                        let payload = json!({
                            "status": "recorded",
                            "decision": rec,
                            "next_action": format!("Decision for '{}' recorded. Call decision.get(\"{}\") to retrieve active policy and dissent.", rec.topic, rec.topic)
                        });
                        ToolCallResult::json(&payload)
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to record decision: {}", e)),
                }
            }

            "decision.get" => {
                let topic = match arguments.get("topic").and_then(|v| v.as_str()) {
                    Some(t) => t,
                    None => return ToolCallResult::error("Missing required parameter: topic"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());

                match self.db.get_decision(project_id, topic) {
                    Ok(res) => {
                        let next_action = match &res.active {
                            Some(act) => format!("Follow active decision for '{}': {}. Proceed with implementation.", act.topic, act.decision),
                            None => format!("No active decision found for '{}'. Use decision.record to establish policy.", topic),
                        };
                        let payload = json!({
                            "topic": res.topic,
                            "active": res.active,
                            "history": res.history,
                            "next_action": next_action
                        });
                        ToolCallResult::json(&payload)
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to get decision: {}", e)),
                }
            }

            "baton.pass" => {
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let to_session = arguments.get("to_session").and_then(|v| v.as_str());
                let note = arguments.get("note").and_then(|v| v.as_str());
                let agent = arguments
                    .get("agent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent");

                match automemory_core::baton::pass_baton(
                    &self.db, project_id, session_id, to_session, agent, note,
                ) {
                    Ok(baton) => {
                        let payload = json!({
                            "status": "passed",
                            "baton": baton,
                            "next_action": "Tell the receiving agent to call baton.pickup with its session_id, then memory.context for full context."
                        });
                        ToolCallResult::json(&payload)
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to pass baton: {}", e)),
                }
            }

            "baton.pickup" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let session_id = arguments.get("session_id").and_then(|v| v.as_str());

                match self.db.latest_baton(project_id, session_id) {
                    Ok(Some(baton)) => {
                        let payload = json!({
                            "baton": baton,
                            "next_action": "Call memory.context for full project context, then continue with the packet's next step."
                        });
                        ToolCallResult::json(&payload)
                    }
                    Ok(None) => ToolCallResult::error("No handoff baton found for this session"),
                    Err(e) => ToolCallResult::error(format!("Failed to pick up baton: {}", e)),
                }
            }

            "task.create" => {
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let title = match arguments.get("title").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: title"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let spec = arguments.get("spec").and_then(|v| v.as_str()).unwrap_or("");
                let symbols: Vec<String> = arguments
                    .get("symbols")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                match self
                    .db
                    .create_task(session_id, project_id, title, spec, &symbols)
                {
                    Ok(task) => ToolCallResult::json(&json!({
                        "status": "created",
                        "task": task,
                        "next_action": "Another agent can now claim it with task.next."
                    })),
                    Err(e) => ToolCallResult::error(format!("Failed to create task: {}", e)),
                }
            }

            "task.next" => {
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());

                match self.db.claim_next_task(project_id, session_id) {
                    Ok(Some(task)) => {
                        let next_action = if task.symbols.is_empty() {
                            "Implement the spec, then call task.submit with the result.".to_string()
                        } else {
                            format!("Call lock.acquire for {:?}, implement the spec, then call task.submit.", task.symbols)
                        };
                        ToolCallResult::json(&json!({ "task": task, "next_action": next_action }))
                    }
                    Ok(None) => ToolCallResult::error("No open tasks for this project"),
                    Err(e) => ToolCallResult::error(format!("Failed to claim task: {}", e)),
                }
            }

            "task.submit" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: id"),
                };
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let result = arguments.get("result").and_then(|v| v.as_str());

                match self.db.submit_task(id, session_id, result) {
                    Ok(true) => ToolCallResult::json(&json!({
                        "status": "submitted",
                        "id": id,
                        "next_action": "Call lock.release for symbols you leased, then baton.pass to hand context on."
                    })),
                    Ok(false) => {
                        ToolCallResult::error("Submit refused: task is not claimed by this session")
                    }
                    Err(e) => ToolCallResult::error(format!("Failed to submit task: {}", e)),
                }
            }

            "review.submit" => {
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let diff = match arguments.get("diff").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: diff"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let task_id = arguments.get("task_id").and_then(|v| v.as_str());
                let title = arguments
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("diff review");
                let test_command = arguments.get("test_command").and_then(|v| v.as_str());

                match self.db.submit_review(
                    project_id,
                    task_id,
                    session_id,
                    title,
                    diff,
                    test_command,
                ) {
                    Ok(review) => ToolCallResult::json(&json!({
                        "status": "queued",
                        "review": review,
                        "next_action": "Another agent reviews it with review.next (never the author)."
                    })),
                    Err(e) => ToolCallResult::error(format!("Failed to queue review: {}", e)),
                }
            }

            "review.next" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let session_id = arguments.get("session_id").and_then(|v| v.as_str());

                match self.db.next_review(project_id, session_id) {
                    Ok(Some(review)) => ToolCallResult::json(&json!({
                        "review": review,
                        "next_action": "Run its test_command, then post the verdict with review.post (tests_passed decides)."
                    })),
                    Ok(None) => ToolCallResult::error("No pending reviews for this session"),
                    Err(e) => ToolCallResult::error(format!("Failed to fetch review: {}", e)),
                }
            }

            "review.post" => {
                let id = match arguments.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: id"),
                };
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let approved = arguments
                    .get("approved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let tests_passed = match arguments.get("tests_passed").and_then(|v| v.as_bool()) {
                    Some(t) => t,
                    None => return ToolCallResult::error(
                        "Missing required parameter: tests_passed (test results judge the diff)",
                    ),
                };
                let verdict = arguments.get("verdict").and_then(|v| v.as_str());

                match self
                    .db
                    .post_review(id, session_id, approved, tests_passed, verdict)
                {
                    Ok(Some(review)) => ToolCallResult::json(&json!({
                        "review": review,
                        "next_action": "If rejected, the author fixes and resubmits with review.submit."
                    })),
                    Ok(None) => ToolCallResult::error(
                        "Post refused: review is decided, missing, or your own diff",
                    ),
                    Err(e) => ToolCallResult::error(format!("Failed to post review: {}", e)),
                }
            }

            "room.pending" => {
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());
                let session_id = arguments.get("session_id").and_then(|v| v.as_str());

                match self.db.pending_rooms(project_id, session_id) {
                    Ok(rooms) => ToolCallResult::json(&json!({
                        "rooms": rooms,
                        "next_action": "Join a room with room.join, or post your turn with room.post."
                    })),
                    Err(e) => ToolCallResult::error(format!("Failed to list rooms: {}", e)),
                }
            }

            "room.join" => {
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let project_id = arguments.get("project_id").and_then(|v| v.as_str());

                if let Some(room_id) = arguments.get("room_id").and_then(|v| v.as_str()) {
                    match self.db.join_room(room_id, session_id) {
                        Ok(Some(room)) => ToolCallResult::json(&json!({
                            "room": room,
                            "next_action": "Post with room.post when turn_holder is your session."
                        })),
                        Ok(None) => ToolCallResult::error("Room not found"),
                        Err(e) => ToolCallResult::error(format!("Failed to join room: {}", e)),
                    }
                } else {
                    let topic = arguments
                        .get("topic")
                        .and_then(|v| v.as_str())
                        .unwrap_or("untitled");
                    match self.db.create_room(project_id, topic, session_id) {
                        Ok(room) => ToolCallResult::json(&json!({
                            "room": room,
                            "next_action": "Invite a second agent via room.join, then open with room.post kind=propose and evidence."
                        })),
                        Err(e) => ToolCallResult::error(format!("Failed to create room: {}", e)),
                    }
                }
            }

            "room.post" => {
                let room_id = match arguments.get("room_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: room_id"),
                };
                let session_id = match arguments.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: session_id"),
                };
                let kind = match arguments
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<automemory_core::models::PostKind>().ok())
                {
                    Some(k) => k,
                    None => {
                        return ToolCallResult::error(
                            "Missing or invalid parameter: kind (propose, critique, revise)",
                        )
                    }
                };
                let body = match arguments.get("body").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: body"),
                };
                let evidence: Vec<String> = arguments
                    .get("evidence")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                match self
                    .db
                    .post_to_room(room_id, session_id, kind, body, &evidence)
                {
                    Ok(post) => {
                        let room = self.db.get_room(room_id).ok().flatten();
                        let next_action = match room.as_ref().map(|r| r.state) {
                            Some(automemory_core::models::RoomState::Decided) => {
                                "Room decided. Record the outcome with decision.record."
                            }
                            _ => "Turn rotated. The next holder continues with room.post.",
                        };
                        ToolCallResult::json(&json!({
                            "post": post,
                            "room": room,
                            "next_action": next_action
                        }))
                    }
                    Err(e) => ToolCallResult::error(format!("Post refused: {}", e)),
                }
            }

            "room.status" => {
                let room_id = match arguments.get("room_id").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolCallResult::error("Missing required parameter: room_id"),
                };

                match self.db.get_room(room_id) {
                    Ok(Some(room)) => {
                        let posts = self.db.room_posts(room_id).unwrap_or_default();
                        ToolCallResult::json(&json!({
                            "room": room,
                            "posts": posts,
                            "next_action": "Post with room.post when turn_holder is your session."
                        }))
                    }
                    Ok(None) => ToolCallResult::error("Room not found"),
                    Err(e) => ToolCallResult::error(format!("Failed to get room: {}", e)),
                }
            }

            unknown => ToolCallResult::error(format!("Unknown tool: {}", unknown)),
        }
    }
}
