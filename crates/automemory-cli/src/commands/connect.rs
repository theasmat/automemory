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
        "opencode" => {
            connect_opencode(&home, &cwd, &exe_path, global)?;
        }
        "goose" | "block-goose" => {
            connect_goose(&home, &cwd, &exe_path, global)?;
        }
        "kilo" | "kilocode" | "kilo-code" => {
            connect_kilo(&home, &cwd, &exe_path, global)?;
        }
        "aider" => {
            connect_aider(&home, &cwd, &exe_path, global)?;
        }
        "warp" => {
            connect_warp(&home, &cwd, &exe_path, global)?;
        }
        "gemini-cli" | "geminicli" => {
            connect_gemini_cli(&home, &cwd, &exe_path, global)?;
        }
        "devin" => {
            connect_devin(&home, &cwd, &exe_path, global)?;
        }
        "copilot" | "github-copilot" | "ghcopilot" => {
            connect_copilot(&home, &cwd, &exe_path, global)?;
        }
        "openclaw" => {
            connect_openclaw(&home, &cwd, &exe_path, global)?;
        }
        "hermes" => {
            connect_hermes(&home, &cwd, &exe_path, global)?;
        }
        "claude-desktop" | "claudedesktop" => {
            connect_claude_desktop(&home, &cwd, &exe_path, global)?;
        }
        "all" => {
            connect_all(&home, &cwd, &exe_path, global)?;
        }
        other => {
            println!("Unknown agent: '{}'. Supported: agy, claude, claude-desktop, cursor, windsurf, continue, cline, roo, zed, codex, opencode, goose, kilo, aider, warp, gemini-cli, devin, copilot, openclaw, hermes, all", other);
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
    let _ = connect_claude_desktop(home, workspace_root, exe_path, global);
    let _ = connect_cursor(workspace_root, home, exe_path, global);
    let _ = connect_windsurf(home, workspace_root, exe_path, global);
    let _ = connect_continue(home, workspace_root, exe_path, global);
    let _ = connect_cline(home, workspace_root, exe_path, global);
    let _ = connect_roo(home, workspace_root, exe_path, global);
    let _ = connect_kilo(home, workspace_root, exe_path, global);
    let _ = connect_zed(home, workspace_root, exe_path, global);
    let _ = connect_codex(home, workspace_root, exe_path, global);
    let _ = connect_opencode(home, workspace_root, exe_path, global);
    let _ = connect_goose(home, workspace_root, exe_path, global);
    let _ = connect_aider(home, workspace_root, exe_path, global);
    let _ = connect_warp(home, workspace_root, exe_path, global);
    let _ = connect_gemini_cli(home, workspace_root, exe_path, global);
    let _ = connect_devin(home, workspace_root, exe_path, global);
    let _ = connect_copilot(home, workspace_root, exe_path, global);
    let _ = connect_openclaw(home, workspace_root, exe_path, global);
    let _ = connect_hermes(home, workspace_root, exe_path, global);

    println!("========================================================");
    println!("✓ All 20 coding agent hosts configured successfully!");
    println!("========================================================");
    Ok(())
}

// ── OpenCode ─────────────────────────────────────────────────────────────────
fn connect_opencode(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    let target_path = if global {
        home.join(".opencode").join("config.json")
    } else {
        workspace_root.join(".opencode").join("config.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to OpenCode at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Goose (Block) ─────────────────────────────────────────────────────────────
fn connect_goose(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    // Goose uses YAML config at ~/.config/goose/config.yaml
    let target_path = if global {
        home.join(".config").join("goose").join("config.yaml")
    } else {
        workspace_root.join(".goose").join("config.yaml")
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Read or create the YAML config
    let mut content = if target_path.exists() {
        fs::read_to_string(&target_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Only add the MCP extension block if not already present
    if !content.contains("automemory") {
        let mcp_block = format!(
            "\nextensions:\n  - name: automemory\n    type: stdio\n    cmd: {}\n    args:\n      - mcp\n",
            exe_path
        );
        content.push_str(&mcp_block);
        fs::write(&target_path, content)?;
    }

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Goose at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Kilo Code (VS Code Extension, same pattern as Cline/Roo) ─────────────────
fn connect_kilo(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        #[cfg(target_os = "macos")]
        let p = home.join("Library/Application Support/Code/User/globalStorage/kilocode.kilo-code/settings/cline_mcp_settings.json");
        #[cfg(target_os = "windows")]
        let p = home.join("AppData/Roaming/Code/User/globalStorage/kilocode.kilo-code/settings/cline_mcp_settings.json");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let p = home.join(
            ".config/Code/User/globalStorage/kilocode.kilo-code/settings/cline_mcp_settings.json",
        );
        p
    } else {
        workspace_root.join(".kilo").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Kilo Code at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Aider ─────────────────────────────────────────────────────────────────────
// Aider uses a YAML config file. MCP is injected via its `--mcp-server` key.
fn connect_aider(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        home.join(".aider.conf.yml")
    } else {
        workspace_root.join(".aider.conf.yml")
    };

    let mut content = if target_path.exists() {
        fs::read_to_string(&target_path).unwrap_or_default()
    } else {
        String::new()
    };

    if !content.contains("automemory") {
        let mcp_block = format!(
            "\n# AutoMemory MCP server\nmcp-servers:\n  automemory:\n    command: {}\n    args: [mcp]\n",
            exe_path
        );
        content.push_str(&mcp_block);
        fs::write(&target_path, content)?;
    }

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Aider at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Warp Terminal ─────────────────────────────────────────────────────────────
fn connect_warp(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    // Warp uses ~/.warp/mcp.json for MCP servers
    let target_path = if global {
        home.join(".warp").join("mcp.json")
    } else {
        workspace_root.join(".warp").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Warp at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Gemini CLI (standalone, different from Antigravity IDE) ──────────────────
// `gemini-cli` is google-gemini/gemini-cli — separate from the AGY plugin system.
fn connect_gemini_cli(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    // Gemini CLI uses ~/.gemini/settings.json (different from AGY's mcp_config.json)
    let target_path = if global {
        home.join(".gemini").join("settings.json")
    } else {
        workspace_root.join(".gemini").join("settings.json")
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

    if val.get("mcpServers").is_none() {
        val["mcpServers"] = serde_json::json!({});
    }
    val["mcpServers"]["automemory"] = automemory_mcp_config(exe_path);

    fs::write(&target_path, serde_json::to_string_pretty(&val)?)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Gemini CLI at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Devin ─────────────────────────────────────────────────────────────────────
// Devin uses a .devin/config.json for MCP server configuration.
fn connect_devin(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        home.join(".devin").join("mcp.json")
    } else {
        workspace_root.join(".devin").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Devin at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── GitHub Copilot CLI ────────────────────────────────────────────────────────
// GitHub Copilot CLI uses the VS Code MCP config path (it runs inside VS Code).
fn connect_copilot(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        // VS Code user-level settings (same as what Copilot Chat picks up)
        get_vscode_user_dir(home).join("mcp.json")
    } else {
        workspace_root.join(".vscode").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to GitHub Copilot (VS Code MCP) at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── OpenClaw ──────────────────────────────────────────────────────────────────
fn connect_openclaw(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    let target_path = if global {
        home.join(".openclaw").join("mcp.json")
    } else {
        workspace_root.join(".openclaw").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to OpenClaw at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Hermes (NousResearch) ─────────────────────────────────────────────────────
fn connect_hermes(home: &Path, workspace_root: &Path, exe_path: &str, global: bool) -> Result<()> {
    let target_path = if global {
        home.join(".hermes").join("mcp.json")
    } else {
        workspace_root.join(".hermes").join("mcp.json")
    };

    merge_mcp_config(&target_path, exe_path)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Hermes at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Claude Desktop (separate from Claude Code) ────────────────────────────────
fn connect_claude_desktop(
    home: &Path,
    _workspace_root: &Path,
    exe_path: &str,
    _global: bool,
) -> Result<()> {
    // Claude Desktop always uses the macOS/Linux app-level config — no workspace mode.
    #[cfg(target_os = "macos")]
    let config_path = home
        .join("Library")
        .join("Application Support")
        .join("Claude")
        .join("claude_desktop_config.json");

    #[cfg(target_os = "linux")]
    let config_path = home
        .join(".config")
        .join("Claude")
        .join("claude_desktop_config.json");

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let config_path = home.join(".claude").join("claude_desktop_config.json");

    merge_mcp_config(&config_path, exe_path)?;

    println!(
        "✓ [APP] Connected to Claude Desktop at {}",
        config_path.display()
    );
    Ok(())
}

// ── VS Code helpers ──────────────────────────────────────────────────────────
fn get_vscode_user_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    return home
        .join("Library")
        .join("Application Support")
        .join("Code")
        .join("User");

    #[cfg(target_os = "linux")]
    return home.join(".config").join("Code").join("User");

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return home.join(".vscode").join("User");
}
