use anyhow::{anyhow, Result};
use automemory_core::hooks::CanonicalEvent;
use automemory_core::ports::{AgentAdapter, AGENT_ADAPTER_API_VERSION};
use std::path::{Path, PathBuf};

use super::connect;

pub type ConnectFn = fn(&Path, &Path, &str, bool) -> Result<()>;

/// One registry entry per supported agent surface. The `connect` column is
/// the same installer `automemory connect` uses; `adapter update` re-runs it
/// to refresh hook/event mappings. `agy`, `claude` and `codex` install native hooks;
/// every other surface joins over MCP (the degrade ladder, PRD §7.4).
pub struct ConnectAdapter {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub events: &'static [CanonicalEvent],
    pub connect: ConnectFn,
}

impl AgentAdapter for ConnectAdapter {
    fn adapter_id(&self) -> &str {
        self.id
    }
    fn aliases(&self) -> &[&str] {
        self.aliases
    }
    fn hook_events(&self) -> &[CanonicalEvent] {
        self.events
    }
    fn connect(
        &self,
        home: &Path,
        workspace_root: &Path,
        exe_path: &str,
        global: bool,
    ) -> Result<()> {
        (self.connect)(home, workspace_root, exe_path, global)
    }
}

const AGY_EVENTS: &[CanonicalEvent] = &[
    CanonicalEvent::SessionStart,
    CanonicalEvent::PreToolUse,
    CanonicalEvent::PostToolUse,
    CanonicalEvent::Stop,
];
const CLAUDE_EVENTS: &[CanonicalEvent] = &[
    CanonicalEvent::SessionStart,
    CanonicalEvent::UserPromptSubmit,
    CanonicalEvent::PreToolUse,
    CanonicalEvent::PostToolUse,
    CanonicalEvent::Stop,
    CanonicalEvent::SessionEnd,
];
const MCP_ONLY: &[CanonicalEvent] = &[];

pub static ADAPTERS: &[ConnectAdapter] = &[
    ConnectAdapter {
        id: "agy",
        aliases: &["antigravity", "gemini"],
        events: AGY_EVENTS,
        connect: connect::connect_agy,
    },
    ConnectAdapter {
        id: "claude",
        aliases: &["claudecode"],
        events: CLAUDE_EVENTS,
        connect: connect::connect_claude,
    },
    ConnectAdapter {
        id: "claude-desktop",
        aliases: &["claudedesktop"],
        events: MCP_ONLY,
        connect: connect::connect_claude_desktop,
    },
    ConnectAdapter {
        id: "cursor",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_cursor_unified,
    },
    ConnectAdapter {
        id: "windsurf",
        aliases: &["codeium"],
        events: MCP_ONLY,
        connect: connect::connect_windsurf,
    },
    ConnectAdapter {
        id: "cline",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_cline,
    },
    ConnectAdapter {
        id: "roo",
        aliases: &["roocode", "roo-cline"],
        events: MCP_ONLY,
        connect: connect::connect_roo,
    },
    ConnectAdapter {
        id: "continue",
        aliases: &["vscode"],
        events: MCP_ONLY,
        connect: connect::connect_continue,
    },
    ConnectAdapter {
        id: "zed",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_zed,
    },
    ConnectAdapter {
        id: "codex",
        aliases: &["openai"],
        events: CLAUDE_EVENTS,
        connect: connect::connect_codex,
    },
    ConnectAdapter {
        id: "opencode",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_opencode,
    },
    ConnectAdapter {
        id: "goose",
        aliases: &["block-goose"],
        events: MCP_ONLY,
        connect: connect::connect_goose,
    },
    ConnectAdapter {
        id: "kilo",
        aliases: &["kilocode", "kilo-code"],
        events: MCP_ONLY,
        connect: connect::connect_kilo,
    },
    ConnectAdapter {
        id: "aider",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_aider,
    },
    ConnectAdapter {
        id: "warp",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_warp,
    },
    ConnectAdapter {
        id: "gemini-cli",
        aliases: &["geminicli"],
        events: MCP_ONLY,
        connect: connect::connect_gemini_cli,
    },
    ConnectAdapter {
        id: "devin",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_devin,
    },
    ConnectAdapter {
        id: "copilot",
        aliases: &["github-copilot", "ghcopilot"],
        events: MCP_ONLY,
        connect: connect::connect_copilot,
    },
    ConnectAdapter {
        id: "openclaw",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_openclaw,
    },
    ConnectAdapter {
        id: "hermes",
        aliases: &[],
        events: MCP_ONLY,
        connect: connect::connect_hermes,
    },
];

pub fn find_adapter(name: &str) -> Option<&'static ConnectAdapter> {
    let lower = name.to_lowercase();
    ADAPTERS
        .iter()
        .find(|a| a.id == lower || a.aliases.iter().any(|alias| *alias == lower))
}

/// Connects through the port contract so replacements slot in here.
pub fn connect_via(
    adapter: &dyn AgentAdapter,
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    automemory_core::ports::check_api_version(
        adapter.adapter_id(),
        AGENT_ADAPTER_API_VERSION,
        AGENT_ADAPTER_API_VERSION,
    )?;
    adapter.connect(home, workspace_root, exe_path, global)
}

fn exe_context() -> (PathBuf, PathBuf, String) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| cwd.join("bin").join("automemory"))
        .to_string_lossy()
        .to_string();
    (home, cwd, exe_path)
}

/// Lists every registered adapter, its aliases, and its hook coverage.
pub fn list() -> Result<()> {
    println!("Adapters ({}):", ADAPTERS.len());
    for a in ADAPTERS {
        let events: Vec<&str> = a.hook_events().iter().map(|e| e.as_str()).collect();
        println!(
            "  {:<15} aliases=[{}] hooks={}",
            a.adapter_id(),
            a.aliases().join(","),
            if events.is_empty() {
                "mcp-only".to_string()
            } else {
                events.join(",")
            }
        );
    }
    Ok(())
}

/// Re-runs an adapter's installer to refresh its hook/event mappings.
pub fn update(agent: &str, global: bool) -> Result<()> {
    let adapter = find_adapter(agent)
        .ok_or_else(|| anyhow!("Unknown agent: '{agent}'. See `automemory adapter list`."))?;
    let (home, cwd, exe_path) = exe_context();
    connect_via(adapter, &home, &cwd, &exe_path, global)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_all_twenty_agents() {
        assert_eq!(ADAPTERS.len(), 20);
        let mut ids: Vec<&str> = ADAPTERS.iter().map(|a| a.adapter_id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 20);
        // agy, claude and codex install native hooks; the rest degrade to MCP-only.
        let hooked: Vec<&ConnectAdapter> = ADAPTERS
            .iter()
            .filter(|a| !a.hook_events().is_empty())
            .collect();
        let mut ids: Vec<&str> = hooked.iter().map(|a| a.adapter_id()).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["agy", "claude", "codex"]);
    }

    #[test]
    fn aliases_resolve_to_canonical_ids() {
        assert_eq!(find_adapter("gemini").unwrap().adapter_id(), "agy");
        assert_eq!(find_adapter("CLAUDE").unwrap().adapter_id(), "claude");
        assert_eq!(find_adapter("ghcopilot").unwrap().adapter_id(), "copilot");
        assert!(find_adapter("nope").is_none());
    }
}
