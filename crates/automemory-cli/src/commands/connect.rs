use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub fn execute(agent: &str, global: bool) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| cwd.join("bin").join("automemory"))
        .to_string_lossy()
        .to_string();

    let scope_label = if global { "GLOBAL" } else { "WORKSPACE" };
    println!(
        "Connecting AutoMemory [{}] for agent: '{}'...\n",
        scope_label, agent
    );

    match agent.to_lowercase().as_str() {
        "agy" | "antigravity" | "gemini" => {
            connect_agy(&home, &cwd, &exe_path, global)?;
        }
        "claude" | "claudecode" => {
            connect_claude(&home, &cwd, &exe_path, global)?;
        }
        "cursor" => {
            connect_cursor(&cwd, &home, &exe_path, global)?;
        }
        "windsurf" | "codeium" => {
            connect_windsurf(&home, &cwd, &exe_path, global)?;
        }
        "cline" => {
            connect_cline(&home, &cwd, &exe_path, global)?;
        }
        "roo" | "roocode" | "roo-cline" => {
            connect_roo(&home, &cwd, &exe_path, global)?;
        }
        "continue" | "vscode" => {
            connect_continue(&home, &cwd, &exe_path, global)?;
        }
        "zed" => {
            connect_zed(&home, &cwd, &exe_path, global)?;
        }
        "codex" | "openai" => {
            connect_codex(&home, &cwd, &exe_path, global)?;
        }
        "all" => {
            connect_all(&home, &cwd, &exe_path, global)?;
        }
        other => {
            println!("Unknown agent host: '{}'. Supported: agy, claude, cursor, windsurf, continue, cline, roo, zed, codex, all", other);
            return Ok(());
        }
    }

    Ok(())
}

fn codebase_memory_config() -> serde_json::Value {
    serde_json::json!({
        "command": "npx",
        "args": ["-y", "@codebase-memory/mcp@latest"]
    })
}

fn automemory_mcp_config(exe_path: &str) -> serde_json::Value {
    serde_json::json!({
        "command": exe_path,
        "args": ["mcp"],
        "env": {
            "AUTOMEMORY_PROJECT_NAME": "auto"
        }
    })
}

fn merge_mcp_config(target_file: &Path, exe_path: &str) -> Result<()> {
    if let Some(parent) = target_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut val = if target_file.exists() {
        serde_json::from_str(&fs::read_to_string(target_file)?)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if val.get("mcpServers").is_none() {
        val["mcpServers"] = serde_json::json!({});
    }

    val["mcpServers"]["automemory"] = automemory_mcp_config(exe_path);
    val["mcpServers"]["codebase-memory"] = codebase_memory_config();

    fs::write(target_file, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

// ── Antigravity / Gemini ────────────────────────────────────────────────────
fn connect_agy(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    if global {
        let gemini_config = home.join(".gemini").join("config");
        fs::create_dir_all(&gemini_config)?;

        let mcp_path = gemini_config.join("mcp_config.json");
        merge_mcp_config(&mcp_path, exe_path)?;

        println!(
            "✓ [GLOBAL] Connected to Antigravity / Gemini at {}",
            mcp_path.display()
        );
    } else {
        let agents_dir = workspace_root.join(".agents");
        fs::create_dir_all(&agents_dir)?;

        // 1. .agents/mcp_config.json
        let mcp_config_path = agents_dir.join("mcp_config.json");
        merge_mcp_config(&mcp_config_path, exe_path)?;

        // 2. Root .mcp.json
        let root_mcp_path = workspace_root.join(".mcp.json");
        merge_mcp_config(&root_mcp_path, exe_path)?;

        // 3. .agents/hooks.json
        let hooks_path = agents_dir.join("hooks.json");
        let hooks_val = serde_json::json!({
            "automemory": {
                "enabled": true,
                "PreInvocation": [
                    {
                        "type": "command",
                        "command": format!("{} hook --agent agy --event boot --project auto", exe_path),
                        "timeout": 5
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [
                            {
                                "type": "command",
                                "command": format!("{} hook --agent agy --event pre-tool --project auto", exe_path),
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "PostToolUse": [
                    {
                        "matcher": "*",
                        "hooks": [
                            {
                                "type": "command",
                                "command": format!("{} hook --agent agy --event post-tool --project auto", exe_path),
                                "timeout": 5
                            }
                        ]
                    }
                ],
                "Stop": [
                    {
                        "type": "command",
                        "command": format!("{} hook --agent agy --event stop --project auto", exe_path),
                        "timeout": 5
                    }
                ]
            }
        });
        fs::write(&hooks_path, serde_json::to_string_pretty(&hooks_val)?)?;

        // 4. Plugin manifest
        let plugin_dir = agents_dir.join("plugins").join("automemory");
        fs::create_dir_all(&plugin_dir)?;
        fs::write(
            plugin_dir.join("plugin.json"),
            serde_json::to_string_pretty(&serde_json::json!({"name": "automemory"}))?,
        )?;
        merge_mcp_config(&plugin_dir.join("mcp_config.json"), exe_path)?;
        fs::write(
            plugin_dir.join("hooks.json"),
            serde_json::to_string_pretty(&hooks_val)?,
        )?;

        println!(
            "✓ [WORKSPACE] Connected to Antigravity at {}",
            workspace_root.display()
        );
        println!("  🪝 Registered hooks in: .agents/hooks.json");
        println!("  📦 Created plugin in:   .agents/plugins/automemory/");
    }

    Ok(())
}

// ── Claude Code & Claude Desktop ────────────────────────────────────────────
fn connect_claude(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    if global {
        let claude_config = home.join(".claude.json");
        merge_mcp_config(&claude_config, exe_path)?;

        #[cfg(target_os = "macos")]
        let desktop_config =
            home.join("Library/Application Support/Claude/claude_desktop_config.json");
        #[cfg(target_os = "windows")]
        let desktop_config = home.join("AppData/Roaming/Claude/claude_desktop_config.json");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let desktop_config = home.join(".config/Claude/claude_desktop_config.json");

        if let Some(parent) = desktop_config.parent() {
            if parent.exists() {
                let _ = merge_mcp_config(&desktop_config, exe_path);
                println!(
                    "✓ [GLOBAL] Connected to Claude Desktop at {}",
                    desktop_config.display()
                );
            }
        }

        println!(
            "✓ [GLOBAL] Connected to Claude Code at {}",
            claude_config.display()
        );
    } else {
        let ws_mcp = workspace_root.join(".mcp.json");
        merge_mcp_config(&ws_mcp, exe_path)?;
        println!("✓ [WORKSPACE] Connected to Claude at {}", ws_mcp.display());
    }

    Ok(())
}

// ── Cursor ──────────────────────────────────────────────────────────────────
fn connect_cursor(workspace_root: &Path, home: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        #[cfg(target_os = "macos")]
        let p = home.join("Library/Application Support/Cursor/User/globalStorage/mcp.json");
        #[cfg(target_os = "windows")]
        let p = home.join("AppData/Roaming/Cursor/User/globalStorage/mcp.json");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let p = home.join(".config/Cursor/User/globalStorage/mcp.json");
        p
    } else {
        workspace_root.join(".cursor").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    // Create .cursorrules in workspace
    let cursorrules_path = workspace_root.join(".cursorrules");
    let rules_content = r#"# AutoMemory Persistent Intelligence Rules
- You have persistent cognitive memory via `automemory` MCP server (`memory.context`, `memory.recall`, `memory.save`, `trace.save`, `memory.proposals.summary`, `memory.proposals.decide_all`).
- You have deep AST code intelligence via `codebase-memory` MCP server (`index_repository`, `query_graph`, `trace_path`).
- At the start of a task, call `memory.context` to load prior decisions, learnings, and golden trajectories.
- Zero-SQL Rule: Never run raw sqlite3 queries on internal databases. Use the official MCP tools and CLI commands.
"#;
    let _ = fs::write(&cursorrules_path, rules_content);

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Cursor at {}",
        scope_tag,
        target_path.display()
    );
    println!("  📝 Created/updated .cursorrules guidelines");
    Ok(())
}

// ── Windsurf / Codeium Cascade ──────────────────────────────────────────────
fn connect_windsurf(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    let target_path = if global {
        home.join(".codeium")
            .join("windsurf")
            .join("mcp_config.json")
    } else {
        workspace_root.join(".windsurf").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Windsurf at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Cline ───────────────────────────────────────────────────────────────────
fn connect_cline(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        #[cfg(target_os = "macos")]
        let p = home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
        #[cfg(target_os = "windows")]
        let p = home.join("AppData/Roaming/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let p = home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
        p
    } else {
        workspace_root.join(".cline").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Cline at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Roo Code ────────────────────────────────────────────────────────────────
fn connect_roo(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        #[cfg(target_os = "macos")]
        let p = home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
        #[cfg(target_os = "windows")]
        let p = home.join("AppData/Roaming/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let p = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
        p
    } else {
        workspace_root.join(".roo").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Roo Code at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Continue.dev / VS Code ──────────────────────────────────────────────────
fn connect_continue(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    let target_path = if global {
        home.join(".continue").join("config.json")
    } else {
        workspace_root.join(".continue").join("config.json")
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut val = if target_path.exists() {
        serde_json::from_str(&fs::read_to_string(&target_path)?)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if val.get("experimental").is_none() {
        val["experimental"] = serde_json::json!({});
    }
    if val["experimental"]
        .get("modelContextProtocolServers")
        .is_none()
    {
        val["experimental"]["modelContextProtocolServers"] = serde_json::json!([]);
    }

    if let Some(servers) = val["experimental"]["modelContextProtocolServers"].as_array_mut() {
        let has_automemory = servers.iter().any(|s| {
            s.get("transport")
                .and_then(|t| t.get("command"))
                .and_then(|c| c.as_str())
                .map(|cmd| cmd.contains("automemory"))
                .unwrap_or(false)
        });

        if !has_automemory {
            servers.push(serde_json::json!({
                "transport": {
                    "type": "stdio",
                    "command": exe_path,
                    "args": ["mcp"]
                }
            }));
        }
    }

    fs::write(&target_path, serde_json::to_string_pretty(&val)?)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Continue.dev at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Zed Editor ──────────────────────────────────────────────────────────────
fn connect_zed(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        home.join(".config").join("zed").join("settings.json")
    } else {
        workspace_root.join(".zed").join("settings.json")
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut val = if target_path.exists() {
        serde_json::from_str(&fs::read_to_string(&target_path)?)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if val.get("context_servers").is_none() {
        val["context_servers"] = serde_json::json!({});
    }

    val["context_servers"]["automemory"] = serde_json::json!({
        "command": {
            "path": exe_path,
            "args": ["mcp"]
        }
    });

    fs::write(&target_path, serde_json::to_string_pretty(&val)?)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Zed editor at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── OpenAI Codex CLI ────────────────────────────────────────────────────────
fn connect_codex(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        home.join(".codex").join("config.json")
    } else {
        workspace_root.join(".codex").join("config.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Codex CLI at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Connect All Agents ──────────────────────────────────────────────────────
fn connect_all(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    println!("========================================================");
    println!("       AutoMemory Multi-Agent Universal Connector       ");
    println!("========================================================");

    let _ = connect_agy(home, workspace_root, exe_path, global);
    let _ = connect_claude(home, workspace_root, exe_path, global);
    let _ = connect_cursor(workspace_root, home, exe_path, global);
    let _ = connect_windsurf(home, workspace_root, exe_path, global);
    let _ = connect_continue(home, workspace_root, exe_path, global);
    let _ = connect_cline(home, workspace_root, exe_path, global);
    let _ = connect_roo(home, workspace_root, exe_path, global);
    let _ = connect_zed(home, workspace_root, exe_path, global);
    let _ = connect_codex(home, workspace_root, exe_path, global);

    println!("========================================================");
    println!("✓ All 9 coding agent hosts configured successfully!");
    println!("========================================================");
    Ok(())
}
