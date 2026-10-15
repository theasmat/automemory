use crate::db::Database;
use crate::extraction::{DistillationReport, TrajectoryDistiller};
use crate::guardrails::rules::GuardrailRule;
use crate::hooks::{CanonicalEvent, FailureMode, HandlerOutput, PipelineCtx};
use crate::models::{LeaseAcquireArgs, LeaseAcquireResult, Memory, PostKind, RoomState, TrustTier};
use crate::retrieval::VectorEngine;
use crate::sync::{GitSync, VaultManager};
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Versioned port contract (PRD §7.4). Every swappable module implements
/// [`Port`]; the host refuses `api_version` mismatches instead of
/// misbehaving (see [`check_api_version`]).
pub trait Port {
    fn port_name(&self) -> &'static str;
    fn api_version(&self) -> &'static str;
}

pub fn check_api_version(port_name: &str, actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(anyhow!(
            "port {port_name}: api_version mismatch (plugin {actual} != host {expected})"
        ))
    }
}

pub const CODE_INTEL_API_VERSION: &str = "1";

/// Code intelligence (PRD §7.1 CodeIntel): symbol existence, content hash,
/// callers. Powers the verifier's future `changed` status.
pub trait CodeIntel: Port {
    fn symbol_exists(&self, project_id: Option<&str>, symbol: &str) -> bool;
    fn symbol_hash(&self, project_id: Option<&str>, symbol: &str) -> Option<String>;
    fn callers(&self, project_id: Option<&str>, symbol: &str) -> Vec<String>;
}

/// Default implementation over the local graph index. An empty (never
/// indexed) graph answers negative — fail-open, never a false claim.
pub struct GraphCodeIntel<'a> {
    db: &'a Database,
}

impl<'a> GraphCodeIntel<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl Port for GraphCodeIntel<'_> {
    fn port_name(&self) -> &'static str {
        "CodeIntel"
    }
    fn api_version(&self) -> &'static str {
        CODE_INTEL_API_VERSION
    }
}

impl CodeIntel for GraphCodeIntel<'_> {
    fn symbol_exists(&self, project_id: Option<&str>, symbol: &str) -> bool {
        self.db
            .query_graph_entities(project_id, None, Some(symbol), None, 1)
            .is_ok_and(|hits| !hits.is_empty())
    }

    /// Index-time content hash of the entity (or its file). The indexer
    /// records per-file sha256; symbol-level hashes arrive with a real
    /// symbol extractor (today the index is file-granular).
    fn symbol_hash(&self, project_id: Option<&str>, symbol: &str) -> Option<String> {
        let hits = self
            .db
            .query_graph_entities(project_id, None, Some(symbol), None, 5)
            .unwrap_or_default();
        for entity in &hits {
            if let Some(hash) = entity.metadata.get("sha256").and_then(|v| v.as_str()) {
                return Some(hash.to_string());
            }
        }
        let path = hits.first().map(|e| e.path.clone())?;
        self.db
            .query_graph_entities(project_id, Some("file"), None, Some(&path), 1)
            .unwrap_or_default()
            .into_iter()
            .find_map(|e| {
                e.metadata
                    .get("sha256")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
    }

    fn callers(&self, project_id: Option<&str>, symbol: &str) -> Vec<String> {
        let targets = self
            .db
            .query_graph_entities(project_id, None, Some(symbol), None, 5)
            .unwrap_or_default();
        let mut out = Vec::new();
        for t in targets {
            for edge in self.db.get_incoming_graph_edges(&t.id).unwrap_or_default() {
                // Current indexer records imports; call-level edges arrive
                // with a richer indexer. Importers are still dependents.
                if !matches!(
                    edge.relation_type.as_str(),
                    "calls" | "references" | "imports"
                ) {
                    continue;
                }
                let name = self
                    .db
                    .get_graph_entity_by_id(&edge.source_id)
                    .ok()
                    .flatten()
                    .map(|e| e.name)
                    .unwrap_or(edge.source_id);
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    }
}

pub const HOOK_HANDLER_API_VERSION: &str = "1";

/// Environment handed to every built-in handler call: connection, scope,
/// and the project's autopilot mode for this invocation.
pub struct HandlerEnv<'a> {
    pub db: &'a Database,
    pub project_id: Option<&'a str>,
    pub mode: crate::models::AutopilotMode,
}

/// A pipeline stage (PRD §7.1 HookHandler). Built-ins implement this;
/// external commands cover the same contract over stdio (see run_exec).
pub trait HookHandler {
    fn handler_id(&self) -> &str;
    fn handle(&self, ctx: &PipelineCtx<'_>, env: &HandlerEnv<'_>) -> HandlerOutput;
}

pub const TRUST_POLICY_API_VERSION: &str = "1";

/// Source-to-tier mapping (PRD §7.1 TrustPolicy). The builtin mirrors
/// `Memory::trust_tier`; stricter org policies implement this trait.
pub trait TrustPolicy: Port {
    fn tier(&self, memory: &Memory) -> TrustTier;
}

pub struct BuiltinTrustPolicy;

impl Port for BuiltinTrustPolicy {
    fn port_name(&self) -> &'static str {
        "TrustPolicy"
    }
    fn api_version(&self) -> &'static str {
        TRUST_POLICY_API_VERSION
    }
}

impl TrustPolicy for BuiltinTrustPolicy {
    fn tier(&self, memory: &Memory) -> TrustTier {
        memory.trust_tier()
    }
}

pub const MEMORY_STORE_API_VERSION: &str = "1";

/// Persistence seam (PRD §7.1 MemoryStore). The default is the SQLite WAL
/// store; teams can back it with another layout without touching callers.
pub trait MemoryStore: Send + Sync {
    fn save_memory(&self, memory: &Memory) -> Result<()>;
    fn get_memory_by_id(&self, id: &str) -> Result<Option<Memory>>;
    fn search_memories_fts(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>>;
    fn list_active_memories(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<Memory>>;
}

impl MemoryStore for Database {
    fn save_memory(&self, memory: &Memory) -> Result<()> {
        Database::save_memory(self, memory)
    }
    fn get_memory_by_id(&self, id: &str) -> Result<Option<Memory>> {
        Database::get_memory_by_id(self, id)
    }
    fn search_memories_fts(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        Database::search_memories_fts(self, query, project_id, limit)
    }
    fn list_active_memories(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<Memory>> {
        self.list_memories(
            project_id,
            Some(crate::models::MemoryStatus::Active),
            None,
            limit,
        )
    }
}

pub const RETRIEVER_API_VERSION: &str = "1";

/// Ranking seam (PRD §7.1 Retriever). The default ranks FTS5 output;
/// rerankers and hybrid strategies implement this trait.
pub trait Retriever: Send + Sync {
    fn retrieve(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>>;
}

pub struct FtsRetriever<'a> {
    store: &'a dyn MemoryStore,
}

impl<'a> FtsRetriever<'a> {
    pub fn new(store: &'a dyn MemoryStore) -> Self {
        Self { store }
    }
}

impl Retriever for FtsRetriever<'_> {
    fn retrieve(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        self.store.search_memories_fts(query, project_id, limit)
    }
}

pub const EMBEDDING_API_VERSION: &str = "1";

/// Text-to-vector seam (PRD §7.1 EmbeddingProvider). The default is the
/// local projection engine; API providers implement this trait.
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

pub struct LocalEmbeddings {
    engine: VectorEngine,
}

impl LocalEmbeddings {
    pub fn new() -> Self {
        Self {
            engine: VectorEngine,
        }
    }
}

impl Default for LocalEmbeddings {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingProvider for LocalEmbeddings {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.engine.embed_text(text)
    }
}

pub const GUARDRAIL_RULE_API_VERSION: &str = "1";

/// Single check seam (PRD §7.1 GuardrailRule). The evaluator consults these
/// before its built-in AST checks; org policies add their own.
pub trait RuleMatcher: Send + Sync {
    fn rule_id(&self) -> &str;
    fn matches_text(&self, text: &str) -> bool;
}

impl RuleMatcher for GuardrailRule {
    fn rule_id(&self) -> &str {
        &self.id
    }
    fn matches_text(&self, text: &str) -> bool {
        Regex::new(&self.pattern)
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    }
}

pub const LOCK_STRATEGY_API_VERSION: &str = "1";

/// Lease-granting seam (PRD §7.1 LockStrategy). The default is advisory
/// file/glob/symbol leases; enforced or worktree strategies replace it.
pub trait LockStrategy: Send + Sync {
    fn acquire(&self, args: LeaseAcquireArgs<'_>) -> Result<LeaseAcquireResult>;
}

pub struct AdvisoryLocks<'a> {
    db: &'a Database,
}

impl<'a> AdvisoryLocks<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
}

impl LockStrategy for AdvisoryLocks<'_> {
    fn acquire(&self, args: LeaseAcquireArgs<'_>) -> Result<LeaseAcquireResult> {
        self.db.acquire_lease(args)
    }
}

pub const DISTILLER_API_VERSION: &str = "1";

/// Session-end extraction seam (PRD §7.1 Distiller). The default distills
/// trajectories into traces; retrospective synthesizers replace it.
pub trait Distiller: Send + Sync {
    fn distill_session(&self, session_id: &str) -> Result<DistillationReport>;
    fn distill_events(
        &self,
        events: &[crate::models::NormalizedEvent],
        session_id: Option<&str>,
    ) -> Result<DistillationReport>;
}

impl Distiller for TrajectoryDistiller<'_> {
    fn distill_session(&self, session_id: &str) -> Result<DistillationReport> {
        TrajectoryDistiller::distill_session(self, session_id)
    }
    fn distill_events(
        &self,
        events: &[crate::models::NormalizedEvent],
        session_id: Option<&str>,
    ) -> Result<DistillationReport> {
        TrajectoryDistiller::distill_events(self, events, session_id)
    }
}

pub const SYNC_BACKEND_API_VERSION: &str = "1";

/// Transport seam (PRD §7.1 SyncBackend). The default moves the vault
/// through git; other transports implement this trait.
pub trait SyncBackend: Send + Sync {
    fn backend_id(&self) -> &'static str;
    fn push(&self, dir: &Path, remote: Option<&str>, branch: Option<&str>) -> Result<()>;
    fn pull(&self, dir: &Path, remote: Option<&str>, branch: Option<&str>) -> Result<()>;
}

pub struct GitTransport;

impl SyncBackend for GitTransport {
    fn backend_id(&self) -> &'static str {
        "git"
    }
    fn push(&self, dir: &Path, remote: Option<&str>, branch: Option<&str>) -> Result<()> {
        GitSync::push(dir, remote, branch)
    }
    fn pull(&self, dir: &Path, remote: Option<&str>, branch: Option<&str>) -> Result<()> {
        GitSync::pull(dir, remote, branch)
    }
}

pub const EXPORTER_API_VERSION: &str = "1";

/// Mirror/pointer seam (PRD §7.1 Exporter). The default writes the
/// text-first vault; markdown mirrors implement this trait.
pub trait Exporter: Send + Sync {
    fn export(&self) -> Result<()>;
}

pub struct VaultExporter<'a> {
    db: &'a Database,
    vault_path: PathBuf,
}

impl<'a> VaultExporter<'a> {
    pub fn new(db: &'a Database, vault_path: PathBuf) -> Self {
        Self { db, vault_path }
    }
}

impl Exporter for VaultExporter<'_> {
    fn export(&self) -> Result<()> {
        VaultManager::new(self.db, &self.vault_path).export_to_vault()
    }
}

pub const RUNNER_API_VERSION: &str = "1";

/// Headless-launch seam (PRD §7.1 Runner). PRD specifies None in v1, so the
/// only in-tree implementation refuses with the reason: vendor terms for
/// automated use are unverified (§16). This trait is the contract a real
/// runner (`claude -p`, `codex exec`) implements.
pub trait Runner: Send + Sync {
    fn launch(&self, prompt: &str, workdir: &Path) -> Result<String>;
}

pub struct NoRunner;

impl Runner for NoRunner {
    fn launch(&self, _prompt: &str, _workdir: &Path) -> Result<String> {
        Err(anyhow!(
            "headless runner not configured: automated agent launch is gated on vendor terms (§16)"
        ))
    }
}

pub const ROOM_PROTOCOL_API_VERSION: &str = "1";

/// Turn-format seam (PRD §7.1 RoomProtocol). The default is the
/// propose/critique/revise machine; review-only or vote rooms replace it.
pub trait RoomProtocol: Send + Sync {
    fn supported_kinds(&self) -> Vec<PostKind>;
    fn validate_turn(
        &self,
        state: RoomState,
        kind: PostKind,
        turn_holder: Option<&str>,
        author: &str,
        evidence_empty: bool,
    ) -> Result<()>;
}

pub struct PcrProtocol;

impl RoomProtocol for PcrProtocol {
    fn supported_kinds(&self) -> Vec<PostKind> {
        vec![PostKind::Propose, PostKind::Critique, PostKind::Revise]
    }
    fn validate_turn(
        &self,
        state: RoomState,
        kind: PostKind,
        turn_holder: Option<&str>,
        author: &str,
        evidence_empty: bool,
    ) -> Result<()> {
        let expected = PostKind::expected_in(state).ok_or_else(|| anyhow!("room is decided"))?;
        if kind != expected {
            return Err(anyhow!("room expects {expected}, not {kind}"));
        }
        if turn_holder != Some(author) {
            return Err(anyhow!("not your turn"));
        }
        if kind == PostKind::Propose && evidence_empty {
            return Err(anyhow!("propose requires evidence"));
        }
        Ok(())
    }
}

pub const NOTIFIER_API_VERSION: &str = "1";

/// Human-ping seam (PRD §7.1 Notifier). The default writes to stderr so it
/// never corrupts hook/MCP stdout; desktop toasts replace it.
pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str);
}

pub struct StderrNotifier;

impl Notifier for StderrNotifier {
    fn notify(&self, title: &str, body: &str) {
        eprintln!("[AutoMemory] {title}: {body}");
    }
}

pub const AGENT_ADAPTER_API_VERSION: &str = "1";

/// Agent-surface seam (PRD §7.1 AgentAdapter). Connect implementations live
/// in the CLI (one entry per agent in the adapter registry); this trait is
/// the contract they satisfy.
pub trait AgentAdapter: Send + Sync {
    fn adapter_id(&self) -> &str;
    fn aliases(&self) -> &[&str];
    fn hook_events(&self) -> &[CanonicalEvent];
    fn connect(
        &self,
        home: &Path,
        workspace_root: &Path,
        exe_path: &str,
        global: bool,
    ) -> Result<()>;
}

/// How each PRD §7.1 port is provided today. `Native` = versioned trait
/// with a default impl; `Concrete` = working default, trait not yet cut;
/// `Deferred` = explicitly gated (see PRD §14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortStatus {
    Native,
    Concrete,
    Deferred,
}

pub struct PortInfo {
    pub id: &'static str,
    pub api_version: &'static str,
    pub status: PortStatus,
    pub default_impl: &'static str,
}

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        id: "AgentAdapter",
        api_version: AGENT_ADAPTER_API_VERSION,
        status: PortStatus::Native,
        default_impl: "connect templates (20 agents)",
    },
    PortInfo {
        id: "HookHandler",
        api_version: HOOK_HANDLER_API_VERSION,
        status: PortStatus::Native,
        default_impl: "lock-check, guardrail",
    },
    PortInfo {
        id: "CodeIntel",
        api_version: CODE_INTEL_API_VERSION,
        status: PortStatus::Native,
        default_impl: "GraphCodeIntel",
    },
    PortInfo {
        id: "MemoryStore",
        api_version: MEMORY_STORE_API_VERSION,
        status: PortStatus::Native,
        default_impl: "SQLite WAL",
    },
    PortInfo {
        id: "Retriever",
        api_version: RETRIEVER_API_VERSION,
        status: PortStatus::Native,
        default_impl: "hybrid FTS + vector RRF",
    },
    PortInfo {
        id: "EmbeddingProvider",
        api_version: EMBEDDING_API_VERSION,
        status: PortStatus::Native,
        default_impl: "local projection engine",
    },
    PortInfo {
        id: "Verifier",
        api_version: "1",
        status: PortStatus::Native,
        default_impl: "DecayVerifier",
    },
    PortInfo {
        id: "TrustPolicy",
        api_version: TRUST_POLICY_API_VERSION,
        status: PortStatus::Native,
        default_impl: "BuiltinTrustPolicy",
    },
    PortInfo {
        id: "GuardrailRule",
        api_version: GUARDRAIL_RULE_API_VERSION,
        status: PortStatus::Native,
        default_impl: "rule structs + AST evaluator",
    },
    PortInfo {
        id: "LockStrategy",
        api_version: LOCK_STRATEGY_API_VERSION,
        status: PortStatus::Native,
        default_impl: "file/glob/symbol leases",
    },
    PortInfo {
        id: "Distiller",
        api_version: DISTILLER_API_VERSION,
        status: PortStatus::Native,
        default_impl: "trajectory distiller",
    },
    PortInfo {
        id: "SyncBackend",
        api_version: SYNC_BACKEND_API_VERSION,
        status: PortStatus::Native,
        default_impl: "vault, git, cloud",
    },
    PortInfo {
        id: "Runner",
        api_version: RUNNER_API_VERSION,
        status: PortStatus::Deferred,
        default_impl: "headless agent launch (vendor terms)",
    },
    PortInfo {
        id: "RoomProtocol",
        api_version: ROOM_PROTOCOL_API_VERSION,
        status: PortStatus::Native,
        default_impl: "propose/critique/revise machine",
    },
    PortInfo {
        id: "Notifier",
        api_version: NOTIFIER_API_VERSION,
        status: PortStatus::Native,
        default_impl: "human-turn ping",
    },
    PortInfo {
        id: "Exporter",
        api_version: EXPORTER_API_VERSION,
        status: PortStatus::Native,
        default_impl: "vault export, agent rule files",
    },
];

/// External plugin contract (PRD §7.4): least-privilege manifest parsed
/// from TOML and validated before the host wires anything up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub ports: Vec<String>,
    #[serde(default)]
    pub capabilities: PluginCapabilities,
    #[serde(default)]
    pub hooks: PluginHooks,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginCapabilities {
    #[serde(default)]
    pub read_ledger: bool,
    #[serde(default)]
    pub write_memory: bool,
    #[serde(default)]
    pub block_tools: bool,
    #[serde(default)]
    pub network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHooks {
    #[serde(default)]
    pub subscribe: Vec<CanonicalEvent>,
    #[serde(default = "default_hook_priority")]
    pub priority: u8,
    #[serde(default = "default_hook_budget_ms")]
    pub latency_budget_ms: u64,
    #[serde(default)]
    pub on_failure: FailureMode,
}

fn default_hook_priority() -> u8 {
    50
}
fn default_hook_budget_ms() -> u64 {
    100
}

impl Default for PluginHooks {
    fn default() -> Self {
        Self {
            subscribe: Vec::new(),
            priority: default_hook_priority(),
            latency_budget_ms: default_hook_budget_ms(),
            on_failure: FailureMode::default(),
        }
    }
}

impl PluginManifest {
    pub fn from_toml_str(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| anyhow!("invalid plugin manifest: {e}"))
    }

    /// Every claimed port must exist in [`PORTS`]; version matching happens
    /// at load time via [`check_api_version`].
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(anyhow!("plugin name is empty"));
        }
        if self.version.trim().is_empty() {
            return Err(anyhow!("plugin version is empty"));
        }
        if self.ports.is_empty() {
            return Err(anyhow!("plugin claims no ports"));
        }
        for port in &self.ports {
            if !PORTS.iter().any(|p| p.id == port) {
                return Err(anyhow!("unknown port: {port}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphEdge, GraphEntity};
    use chrono::Utc;

    #[test]
    fn api_version_gate_refuses_mismatch() {
        assert!(check_api_version("CodeIntel", "1", CODE_INTEL_API_VERSION).is_ok());
        assert!(check_api_version("CodeIntel", "2", CODE_INTEL_API_VERSION).is_err());
    }

    #[test]
    fn empty_graph_answers_negative() {
        let db = Database::open_in_memory().unwrap();
        let ci = GraphCodeIntel::new(&db);
        assert!(!ci.symbol_exists(None, "anything"));
        assert_eq!(ci.symbol_hash(None, "anything"), None);
        assert!(ci.callers(None, "anything").is_empty());
    }

    #[test]
    fn graph_backend_resolves_symbols_and_importers() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();
        db.batch_insert_graph_entities(&[
            GraphEntity {
                id: "e-auth".to_string(),
                project_id: None,
                entity_type: "function".to_string(),
                name: "auth.session.createSession".to_string(),
                path: "src/auth/session.ts".to_string(),
                line_start: Some(1),
                line_end: Some(10),
                metadata: serde_json::json!({ "sha256": "abc123" }),
                created_at: now,
                updated_at: now,
            },
            GraphEntity {
                id: "e-login".to_string(),
                project_id: None,
                entity_type: "function".to_string(),
                name: "auth.login".to_string(),
                path: "src/auth/login.ts".to_string(),
                line_start: Some(1),
                line_end: Some(5),
                metadata: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            },
        ])
        .unwrap();
        db.batch_insert_graph_edges(&[GraphEdge {
            source_id: "e-login".to_string(),
            target_id: "e-auth".to_string(),
            relation_type: "imports".to_string(),
            confidence: 1.0,
            created_at: now,
        }])
        .unwrap();

        let ci = GraphCodeIntel::new(&db);
        assert!(
            check_api_version(ci.port_name(), ci.api_version(), CODE_INTEL_API_VERSION).is_ok()
        );
        assert!(ci.symbol_exists(None, "createSession"));
        assert!(!ci.symbol_exists(None, "no.such.symbol"));
        assert_eq!(
            ci.callers(None, "createSession"),
            vec!["auth.login".to_string()]
        );
        assert_eq!(
            ci.symbol_hash(None, "createSession"),
            Some("abc123".to_string())
        );
        assert_eq!(ci.symbol_hash(None, "no.such.symbol"), None);
    }

    #[test]
    fn port_inventory_covers_all_sixteen() {
        assert_eq!(PORTS.len(), 16);
        let mut ids: Vec<&str> = PORTS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 16);
        assert!(PORTS
            .iter()
            .any(|p| p.id == "HookHandler" && p.status == PortStatus::Native));
        assert!(PORTS
            .iter()
            .any(|p| p.id == "Runner" && p.status == PortStatus::Deferred));
    }

    #[test]
    fn builtin_trust_policy_matches_inherent_tier() {
        use crate::models::{MemoryScope, MemorySourceKind, MemoryStatus, MemoryType};
        use chrono::Utc;
        use uuid::Uuid;
        let policy = BuiltinTrustPolicy;
        let mem = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: None,
            memory_type: MemoryType::Fact,
            content: "c".to_string(),
            summary: "s".to_string(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        assert_eq!(policy.tier(&mem), mem.trust_tier());
    }

    #[test]
    fn plugin_manifest_parses_and_validates() {
        let manifest = PluginManifest::from_toml_str(
            r#"
            name = "symbol-verifier"
            version = "0.3.1"
            api_version = "1"
            ports = ["Verifier"]

            [capabilities]
            read_ledger = true

            [hooks]
            subscribe = ["PostCommit"]
            priority = 50
            latency_budget_ms = 20
            on_failure = "open"
            "#,
        )
        .unwrap();
        assert_eq!(manifest.hooks.priority, 50);
        assert!(manifest.validate().is_ok());

        let bad = PluginManifest::from_toml_str(
            "name = \"x\"\nversion = \"1\"\napi_version = \"1\"\nports = [\"Nope\"]\n",
        )
        .unwrap();
        assert!(bad.validate().is_err());
    }

    fn seed_memory(db: &Database, summary: &str) -> crate::models::Memory {
        use crate::models::{MemoryScope, MemorySourceKind, MemoryStatus, MemoryType};
        let mem = crate::models::Memory {
            id: uuid::Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: None,
            memory_type: MemoryType::Fact,
            content: format!("content for {summary}"),
            summary: summary.to_string(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory(&mem).unwrap();
        mem
    }

    #[test]
    fn memory_store_trait_roundtrips_through_database() {
        let db = Database::open_in_memory().unwrap();
        let store: &dyn MemoryStore = &db;
        let mem = seed_memory(&db, "store seam memory");
        let fetched = store.get_memory_by_id(&mem.id).unwrap().unwrap();
        assert_eq!(fetched.summary, "store seam memory");
        let hits = store.search_memories_fts("seam", None, 5).unwrap();
        assert!(hits.iter().any(|(m, _)| m.id == mem.id));
        let listed = store.list_active_memories(None, 5).unwrap();
        assert!(listed.iter().any(|m| m.id == mem.id));
    }

    #[test]
    fn retriever_delegates_to_injected_store() {
        struct StubStore {
            mems: Vec<crate::models::Memory>,
        }
        impl MemoryStore for StubStore {
            fn save_memory(&self, _m: &crate::models::Memory) -> Result<()> {
                Ok(())
            }
            fn get_memory_by_id(&self, _id: &str) -> Result<Option<crate::models::Memory>> {
                Ok(None)
            }
            fn search_memories_fts(
                &self,
                _q: &str,
                _p: Option<&str>,
                _l: usize,
            ) -> Result<Vec<(crate::models::Memory, f64)>> {
                Ok(self.mems.clone().into_iter().map(|m| (m, 1.0)).collect())
            }
            fn list_active_memories(
                &self,
                _p: Option<&str>,
                _l: usize,
            ) -> Result<Vec<crate::models::Memory>> {
                Ok(self.mems.clone())
            }
        }
        let db = Database::open_in_memory().unwrap();
        let mem = seed_memory(&db, "stub seam memory");
        let stub = StubStore { mems: vec![mem] };
        let retriever = FtsRetriever::new(&stub);
        let hits = retriever.retrieve("anything", None, 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.summary, "stub seam memory");
    }

    #[test]
    fn local_embeddings_are_deterministic_vectors() {
        let provider = LocalEmbeddings::new();
        let a = provider.embed("jwt session tokens").unwrap();
        let b = provider.embed("jwt session tokens").unwrap();
        assert_eq!(a.len(), 384);
        assert_eq!(a, b);
    }

    #[test]
    fn rule_matcher_covers_builtin_and_custom_rules() {
        struct DenyAll;
        impl RuleMatcher for DenyAll {
            fn rule_id(&self) -> &str {
                "org-deny-all"
            }
            fn matches_text(&self, _text: &str) -> bool {
                true
            }
        }
        let builtin = crate::guardrails::rules::GuardrailRule::default_rules()
            .into_iter()
            .find(|r| r.id == "rule_raw_db_drop")
            .unwrap();
        assert_eq!(builtin.rule_id(), "rule_raw_db_drop");
        assert!(builtin.matches_text("DROP DATABASE prod"));
        assert!(!builtin.matches_text("SELECT 1"));
        let custom = DenyAll;
        assert!(custom.matches_text("anything at all"));
    }

    #[test]
    fn advisory_locks_delegate_acquire_renew_conflict() {
        use crate::models::{LeaseAcquireArgs, LeaseAcquireResult, LeaseScopeType};
        let db = Database::open_in_memory().unwrap();
        let strategy = AdvisoryLocks::new(&db);
        fn args(session: &str) -> LeaseAcquireArgs<'_> {
            LeaseAcquireArgs {
                project_id: None,
                agent: "agent",
                session_id: session,
                scope: "src/a.ts",
                scope_type: LeaseScopeType::File,
                goal: "edit",
                ttl_seconds: 300,
            }
        }
        assert!(matches!(
            strategy.acquire(args("s1")).unwrap(),
            LeaseAcquireResult::Acquired { .. }
        ));
        assert!(matches!(
            strategy.acquire(args("s2")).unwrap(),
            LeaseAcquireResult::Conflict { .. }
        ));
    }

    #[test]
    fn distiller_trait_delegates_to_trajectory_default() {
        let db = Database::open_in_memory().unwrap();
        let distiller = TrajectoryDistiller::new(&db);
        let report = distiller.distill_session("no-such-session").unwrap();
        assert_eq!(report.total_raw_events, 0);
    }

    #[test]
    fn git_transport_reports_backend_id_and_surfaces_errors() {
        let backend = GitTransport;
        assert_eq!(backend.backend_id(), "git");
        let dir = std::env::temp_dir().join("automemory-no-repo-here");
        assert!(backend.push(&dir, None, None).is_err());
    }

    #[test]
    fn vault_exporter_writes_text_first_vault() {
        let db = Database::open_in_memory().unwrap();
        seed_memory(&db, "export seam memory");
        let dir =
            std::env::temp_dir().join(format!("automemory-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        VaultExporter::new(&db, dir.clone()).export().unwrap();
        let found = walk_for_memories(&dir);
        assert!(
            found,
            "exported vault should contain memory records under {dir:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn walk_for_memories(dir: &std::path::Path) -> bool {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let entries = std::fs::read_dir(&d).unwrap();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if text.contains("export seam memory") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    #[test]
    fn no_runner_refuses_with_vendor_terms_reason() {
        let runner = NoRunner;
        let err = runner
            .launch("do work", std::path::Path::new("/tmp"))
            .unwrap_err();
        assert!(err.to_string().contains("vendor terms"));
    }

    #[test]
    fn pcr_protocol_accepts_machine_order_and_rejects_other() {
        use crate::models::{PostKind, RoomState};
        let protocol = PcrProtocol;
        assert_eq!(protocol.supported_kinds().len(), 3);
        assert!(protocol
            .validate_turn(RoomState::Open, PostKind::Propose, Some("a"), "a", false)
            .is_ok());
        assert!(protocol
            .validate_turn(RoomState::Open, PostKind::Critique, Some("a"), "a", false)
            .is_err());
        assert!(protocol
            .validate_turn(RoomState::Open, PostKind::Propose, Some("b"), "a", false)
            .is_err());
        assert!(protocol
            .validate_turn(RoomState::Open, PostKind::Propose, Some("a"), "a", true)
            .is_err());
    }

    #[test]
    fn stderr_notifier_never_fails() {
        StderrNotifier.notify("test-title", "test-body");
    }
}
