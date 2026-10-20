-- AutoMemory Core Database Schema

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
    spec TEXT NOT NULL DEFAULT '',
    symbols TEXT NOT NULL DEFAULT '[]',
    owner_session TEXT,
    result TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_status_created ON tasks(status, created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_owner ON tasks(owner_session);

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
CREATE INDEX IF NOT EXISTS idx_tool_events_timestamp ON tool_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_tool_events_proj_time ON tool_events(project_id, timestamp);

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
    category TEXT NOT NULL DEFAULT 'general',
    title TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL,
    remote_synced_hash TEXT,
    summary TEXT,
    l0_abstract TEXT,
    l1_overview TEXT,
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

-- Local Semantic Vector Embeddings
CREATE TABLE IF NOT EXISTS memory_embeddings (
    memory_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL DEFAULT 384,
    model TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(memory_id) REFERENCES memories(id) ON DELETE CASCADE
);

-- Codebase Structural Knowledge Graph
CREATE TABLE IF NOT EXISTS graph_entities (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    entity_type TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_graph_entities_proj ON graph_entities(project_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_graph_entities_path ON graph_entities(path);
CREATE INDEX IF NOT EXISTS idx_graph_entities_name ON graph_entities(name);

CREATE TABLE IF NOT EXISTS graph_edges (
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (source_id, target_id, relation_type),
    FOREIGN KEY(source_id) REFERENCES graph_entities(id) ON DELETE CASCADE,
    FOREIGN KEY(target_id) REFERENCES graph_entities(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(target_id);

CREATE TABLE IF NOT EXISTS custom_guardrail_rules (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    pattern TEXT NOT NULL,
    is_critical INTEGER NOT NULL DEFAULT 0,
    action TEXT NOT NULL,
    description TEXT NOT NULL,
    mitigation_hint TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_custom_guardrails_proj ON custom_guardrail_rules(project_id, enabled);

-- Procedural Skills (Consolidated Reusable Playbooks)
CREATE TABLE IF NOT EXISTS procedural_skills (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    trigger_patterns TEXT NOT NULL,
    preconditions TEXT NOT NULL,
    steps TEXT NOT NULL,
    caveats TEXT NOT NULL,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    source_trace_ids TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_procedural_skills_proj ON procedural_skills(project_id);

-- Multi-Agent Coordination Leases and Resource Locks
CREATE TABLE IF NOT EXISTS leases (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    agent TEXT NOT NULL,
    session_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    scope_type TEXT NOT NULL DEFAULT 'file',
    goal TEXT NOT NULL,
    ttl_seconds INTEGER NOT NULL DEFAULT 300,
    acquired_at TEXT NOT NULL,
    heartbeat_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_leases_scope ON leases(project_id, scope);
CREATE INDEX IF NOT EXISTS idx_leases_expires ON leases(expires_at);
CREATE INDEX IF NOT EXISTS idx_leases_session ON leases(session_id);

-- Architecture Decisions (Binding records with alternatives, reasons, and dissent)
CREATE TABLE IF NOT EXISTS decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    topic TEXT NOT NULL,
    decision TEXT NOT NULL,
    reason TEXT NOT NULL,
    alternatives TEXT NOT NULL DEFAULT '[]',
    dissent TEXT,
    agent TEXT NOT NULL,
    symbols TEXT NOT NULL DEFAULT '[]',
    evidence TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'active',
    superseded_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_decisions_topic ON decisions(project_id, topic);
CREATE INDEX IF NOT EXISTS idx_decisions_status ON decisions(status);

-- Agent-to-Agent Handoff Batons (<400-token packets built from the ledger)
CREATE TABLE IF NOT EXISTS batons (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    from_session TEXT NOT NULL,
    to_session TEXT,
    agent TEXT NOT NULL,
    packet TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_batons_pickup ON batons(project_id, to_session, created_at);

-- Cross-review diff queue (tests are the judge, not votes)
CREATE TABLE IF NOT EXISTS reviews (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    task_id TEXT,
    author_session TEXT NOT NULL,
    title TEXT NOT NULL,
    diff TEXT NOT NULL,
    test_command TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    reviewer_session TEXT,
    tests_passed INTEGER,
    verdict TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reviews_queue ON reviews(project_id, status, created_at);

-- Planning Room: propose → critique → revise → decided, turn-gated posts
CREATE TABLE IF NOT EXISTS rooms (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    topic TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'open',
    members TEXT NOT NULL DEFAULT '[]',
    turn_holder TEXT,
    round INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS room_posts (
    id TEXT PRIMARY KEY,
    room_id TEXT NOT NULL,
    author_session TEXT NOT NULL,
    kind TEXT NOT NULL,
    body TEXT NOT NULL,
    evidence TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    FOREIGN KEY(room_id) REFERENCES rooms(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_rooms_turn ON rooms(project_id, state, turn_holder);
CREATE INDEX IF NOT EXISTS idx_room_posts_room ON room_posts(room_id, created_at);

-- Automatic locking: pre-command workspace snapshots, per-write coverage, watcher alerts
CREATE TABLE IF NOT EXISTS workspace_snapshots (
    session_id TEXT PRIMARY KEY,
    project_id TEXT,
    taken_at TEXT NOT NULL,
    in_flight INTEGER NOT NULL DEFAULT 1,
    data TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS lock_coverage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    project_id TEXT,
    session_id TEXT NOT NULL,
    path TEXT NOT NULL,
    source TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_lock_coverage_ts ON lock_coverage(ts);

CREATE TABLE IF NOT EXISTS lock_alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    project_id TEXT,
    path TEXT NOT NULL,
    holder_agent TEXT NOT NULL,
    holder_session TEXT NOT NULL,
    message TEXT NOT NULL
);

-- Notes for a session's agent that its hooks cannot print immediately (Antigravity's
-- PostToolUse output may only be {}); delivered on its next PreInvocation.
CREATE TABLE IF NOT EXISTS session_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    ts TEXT NOT NULL,
    text TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_session_notes_session ON session_notes(session_id);

-- Identical hook invocations (a hook registered twice) answer once
CREATE TABLE IF NOT EXISTS hook_claims (
    key TEXT PRIMARY KEY,
    ts INTEGER NOT NULL,
    output TEXT
);

-- Stable project identity across machines (see sync::identity)
CREATE TABLE IF NOT EXISTS project_keys (
    project_id TEXT PRIMARY KEY,
    project_key TEXT NOT NULL UNIQUE
);

-- Vectors per (memory, embedding model): switching model never mixes vector spaces
CREATE TABLE IF NOT EXISTS memory_vectors (
    memory_id TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    embedding BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (memory_id, model),
    FOREIGN KEY(memory_id) REFERENCES memories(id) ON DELETE CASCADE
);

-- Sync watermarks + applied delta files (incremental op-log sync)
CREATE TABLE IF NOT EXISTS sync_kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Continuity: what previous sessions worked out (goal, ruled-out options, open questions, next steps)
CREATE TABLE IF NOT EXISTS brief_items (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    reason TEXT,
    ref_id TEXT,
    origin TEXT NOT NULL,
    author TEXT NOT NULL,
    session TEXT,
    state TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_brief_project ON brief_items(project_id, status, updated_at DESC);
