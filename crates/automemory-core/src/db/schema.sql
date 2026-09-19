-- AutoMemory Pro Core Database Schema

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    display_name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    capture_policy TEXT NOT NULL,
    context_budget TEXT NOT NULL,
    document_config TEXT NOT NULL,
    sync_config TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS project_paths (
    project_id TEXT NOT NULL,
    local_path TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (project_id, local_path),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    agent_adapter TEXT NOT NULL,
    agent_instance_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    project_id TEXT,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tool_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    session_id TEXT NOT NULL,
    task_id TEXT,
    project_id TEXT,
    agent_adapter TEXT NOT NULL,
    tool_name TEXT,
    tool_category TEXT,
    tool_input TEXT,
    tool_input_hash TEXT,
    tool_output TEXT,
    tool_output_hash TEXT,
    status TEXT NOT NULL,
    exit_code INTEGER,
    duration_ms INTEGER,
    artifacts TEXT,
    sensitivity TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    processed INTEGER NOT NULL DEFAULT 0,
    raw_payload TEXT
);

CREATE INDEX IF NOT EXISTS idx_tool_events_processed ON tool_events(processed, timestamp);
CREATE INDEX IF NOT EXISTS idx_tool_events_session ON tool_events(session_id);
CREATE INDEX IF NOT EXISTS idx_tool_events_project ON tool_events(project_id);

CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    project_id TEXT,
    memory_type TEXT NOT NULL,
    content TEXT NOT NULL,
    summary TEXT NOT NULL,
    confidence REAL NOT NULL,
    importance REAL NOT NULL,
    freshness REAL NOT NULL,
    status TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_recalled_at TEXT,
    tags TEXT NOT NULL,
    entities TEXT NOT NULL,
    metadata TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memories_scope_project ON memories(scope, project_id);
CREATE INDEX IF NOT EXISTS idx_memories_status_type ON memories(status, memory_type);
CREATE INDEX IF NOT EXISTS idx_memories_confidence ON memories(confidence DESC);
CREATE INDEX IF NOT EXISTS idx_memories_updated ON memories(updated_at DESC);

-- FTS5 Full-Text Search Table
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id UNINDEXED,
    content,
    summary,
    tags,
    tokenize='unicode61'
);

-- Triggers to synchronize memories_fts
CREATE TRIGGER IF NOT EXISTS trg_memories_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(id, content, summary, tags)
    VALUES (new.id, new.content, new.summary, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS trg_memories_update AFTER UPDATE ON memories BEGIN
    DELETE FROM memories_fts WHERE id = old.id;
    INSERT INTO memories_fts(id, content, summary, tags)
    VALUES (new.id, new.content, new.summary, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS trg_memories_delete AFTER DELETE ON memories BEGIN
    DELETE FROM memories_fts WHERE id = old.id;
END;

CREATE TABLE IF NOT EXISTS memory_versions (
    id TEXT PRIMARY KEY,
    memory_id TEXT NOT NULL,
    version_num INTEGER NOT NULL,
    content TEXT NOT NULL,
    summary TEXT NOT NULL,
    reason TEXT,
    evidence_event_ids TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(memory_id) REFERENCES memories(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_memory_versions_mem ON memory_versions(memory_id, version_num);

CREATE TABLE IF NOT EXISTS memory_links (
    source_memory_id TEXT NOT NULL,
    target_memory_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (source_memory_id, target_memory_id, relation_type)
);

CREATE TABLE IF NOT EXISTS success_traces (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    title TEXT NOT NULL,
    category TEXT NOT NULL,
    status TEXT NOT NULL,
    preconditions TEXT NOT NULL,
    steps TEXT NOT NULL,
    reproducibility_score REAL NOT NULL,
    caveats TEXT NOT NULL,
    source_task_id TEXT,
    evidence_event_ids TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_traces_project ON success_traces(project_id, status);

CREATE TABLE IF NOT EXISTS failure_records (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    task_title TEXT NOT NULL,
    category TEXT,
    failed_tool TEXT,
    error_message TEXT NOT NULL,
    root_cause_hypothesis TEXT,
    fix_applied TEXT,
    verification_result TEXT,
    status TEXT NOT NULL,
    evidence_event_ids TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_failures_project ON failure_records(project_id, status);

CREATE TABLE IF NOT EXISTS preferences (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    scope TEXT NOT NULL,
    project_id TEXT,
    locked INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prefs_unique ON preferences(key, scope, COALESCE(project_id, ''));

CREATE TABLE IF NOT EXISTS tracked_documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    summary TEXT,
    last_scanned_at TEXT NOT NULL,
    is_deleted INTEGER NOT NULL DEFAULT 0,
    UNIQUE(project_id, relative_path)
);

CREATE TABLE IF NOT EXISTS document_revisions (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    revision_num INTEGER NOT NULL,
    content TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(document_id) REFERENCES tracked_documents(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS review_proposals (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    proposal_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    description TEXT NOT NULL,
    suggested_action TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    decided_at TEXT
);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    details TEXT NOT NULL,
    timestamp TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp DESC);
