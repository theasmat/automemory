use crate::db::Database;
use crate::hooks::{CanonicalEvent, FailureMode, HandlerOutput, PipelineCtx};
use crate::models::{Memory, TrustTier};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

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
        api_version: "1",
        status: PortStatus::Concrete,
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
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "SQLite WAL",
    },
    PortInfo {
        id: "Retriever",
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "hybrid FTS + vector RRF",
    },
    PortInfo {
        id: "EmbeddingProvider",
        api_version: "1",
        status: PortStatus::Concrete,
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
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "rule structs + AST evaluator",
    },
    PortInfo {
        id: "LockStrategy",
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "file/glob/symbol leases",
    },
    PortInfo {
        id: "Distiller",
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "trajectory distiller",
    },
    PortInfo {
        id: "SyncBackend",
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "vault, git, cloud",
    },
    PortInfo {
        id: "Runner",
        api_version: "1",
        status: PortStatus::Deferred,
        default_impl: "headless agent launch (vendor terms)",
    },
    PortInfo {
        id: "RoomProtocol",
        api_version: "1",
        status: PortStatus::Concrete,
        default_impl: "propose/critique/revise machine",
    },
    PortInfo {
        id: "Notifier",
        api_version: "1",
        status: PortStatus::Deferred,
        default_impl: "human-turn ping",
    },
    PortInfo {
        id: "Exporter",
        api_version: "1",
        status: PortStatus::Concrete,
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
}
