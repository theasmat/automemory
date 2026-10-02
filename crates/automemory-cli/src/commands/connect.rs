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

    match agent.to_lowercase().as_str() {
        "agy" | "antigravity" => {
            if global {
                connect_agy_global(&home, &exe_path)?;
            } else {
                connect_agy_workspace(&cwd, &exe_path)?;
            }
        }
        "claude" | "claudecode" => {
            connect_claude(&home, &cwd, &exe_path)?;
        }
        "cursor" => {
            connect_cursor(&cwd, &home, &exe_path, global)?;
        }
        "windsurf" | "codeium" => {
            connect_windsurf(&home, &exe_path)?;
        }
        "cline" | "roo" | "roocode" => {
            connect_cline(&home, &exe_path)?;
        }
        "codex" | "openai" => {
            connect_codex(&cwd, &exe_path)?;
        }
        "gemini" => {
            connect_agy_global(&home, &exe_path)?;
        }
        other => {
            println!("Unknown agent host: '{}'. Supported: agy, claude, cursor, windsurf, cline, codex, gemini", other);
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

fn connect_agy_workspace(workspace_root: &Path, exe_path: &str) -> Result<()> {
    let agents_dir = workspace_root.join(".agents");
    fs::create_dir_all(&agents_dir)?;

    // 1. Workspace .agents/mcp_config.json
    let mcp_config_path = agents_dir.join("mcp_config.json");
    let mut mcp_val = if mcp_config_path.exists() {
        serde_json::from_str(&fs::read_to_string(&mcp_config_path)?).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if mcp_val.get("mcpServers").is_none() {
        mcp_val["mcpServers"] = serde_json::json!({});
    }

    // AutoMemory Pro MCP
    mcp_val["mcpServers"]["automemory"] = serde_json::json!({
        "command": exe_path,
        "args": ["mcp"],
        "env": {
            "AUTOMEMORY_PROJECT_NAME": "auto"
        }
    });

    // Codebase Memory Companion MCP
    mcp_val["mcpServers"]["codebase-memory"] = codebase_memory_config();

    fs::write(&mcp_config_path, serde_json::to_string_pretty(&mcp_val)?)?;

    // 2. Root .mcp.json
    let root_mcp_path = workspace_root.join(".mcp.json");
    fs::write(&root_mcp_path, serde_json::to_string_pretty(&mcp_val)?)?;

    // 3. Workspace .agents/hooks.json
    let hooks_path = agents_dir.join("hooks.json");
    let hooks_val = serde_json::json!({
        "automemory-pro": {
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

    // 4. Workspace Plugin
    let plugin_dir = agents_dir.join("plugins").join("automemory");
    fs::create_dir_all(&plugin_dir)?;
    fs::write(plugin_dir.join("plugin.json"), serde_json::to_string_pretty(&serde_json::json!({"name": "automemory"}))?)?;
    fs::write(plugin_dir.join("mcp_config.json"), serde_json::to_string_pretty(&mcp_val)?)?;
    fs::write(plugin_dir.join("hooks.json"), serde_json::to_string_pretty(&hooks_val)?)?;

    println!("✓ Successfully connected Dual-Engine Suite to Antigravity workspace at {}", workspace_root.display());
    println!("  🧠 Registered AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Registered Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    println!("  🪝 Registered lifecycle hooks in: .agents/hooks.json");
    println!("  📦 Created workspace plugin in: .agents/plugins/automemory/");
    Ok(())
}

fn connect_agy_global(home: &Path, exe_path: &str) -> Result<()> {
    let gemini_config = home.join(".gemini").join("config");
    fs::create_dir_all(&gemini_config)?;

    let mcp_path = gemini_config.join("mcp_config.json");
    let mut mcp_val = if mcp_path.exists() {
        serde_json::from_str(&fs::read_to_string(&mcp_path)?).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if mcp_val.get("mcpServers").is_none() {
        mcp_val["mcpServers"] = serde_json::json!({});
    }
    mcp_val["mcpServers"]["automemory"] = serde_json::json!({
        "command": exe_path,
        "args": ["mcp"],
        "env": {
            "AUTOMEMORY_PROJECT_NAME": "auto"
        }
    });
    mcp_val["mcpServers"]["codebase-memory"] = codebase_memory_config();

    fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_val)?)?;

    println!("✓ Successfully registered Dual-Engine Suite in global Antigravity config at {}", mcp_path.display());
    println!("  🧠 AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    Ok(())
}

fn connect_claude(home: &Path, workspace_root: &Path, exe_path: &str) -> Result<()> {
    let claude_config = home.join(".claude.json");
    let mut val = if claude_config.exists() {
        serde_json::from_str(&fs::read_to_string(&claude_config)?).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if val.get("mcpServers").is_none() {
        val["mcpServers"] = serde_json::json!({});
    }

    val["mcpServers"]["automemory"] = serde_json::json!({
        "command": exe_path,
        "args": ["mcp"]
    });
    val["mcpServers"]["codebase-memory"] = codebase_memory_config();

    fs::write(&claude_config, serde_json::to_string_pretty(&val)?)?;

    // Also write to workspace .mcp.json
    let ws_mcp = workspace_root.join(".mcp.json");
    fs::write(&ws_mcp, serde_json::to_string_pretty(&val)?)?;

    println!("✓ Successfully connected Dual-Engine Suite to Claude Code at {}", claude_config.display());
    println!("  🧠 AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    Ok(())
}

fn merge_mcp_config(target_file: &Path, exe_path: &str) -> Result<()> {
    if let Some(parent) = target_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut val = if target_file.exists() {
        serde_json::from_str(&fs::read_to_string(target_file)?).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if val.get("mcpServers").is_none() {
        val["mcpServers"] = serde_json::json!({});
    }

    val["mcpServers"]["automemory"] = serde_json::json!({
        "command": exe_path,
        "args": ["mcp"],
        "env": {
            "AUTOMEMORY_PROJECT_NAME": "auto"
        }
    });
    val["mcpServers"]["codebase-memory"] = codebase_memory_config();

    fs::write(target_file, serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

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

    // Also create .cursorrules in workspace with AutoMemory context guidelines
    let cursorrules_path = workspace_root.join(".cursorrules");
    let rules_content = r#"# AutoMemory Pro & Codebase Memory Companion Rules
- You have persistent cognitive memory via `automemory` MCP server (`memory.context`, `memory.recall`, `memory.save`, `trace.save`).
- You have deep Tree-Sitter AST structural code intelligence via `codebase-memory` MCP server (`index_repository`, `query_graph`, `trace_path`, `search_graph`).
- At the start of a task, call `memory.context` to load prior decisions, learnings, and golden trajectories.
- Before modifying complex architectures, use `codebase-memory` to trace callers and call graphs.
"#;
    let _ = fs::write(&cursorrules_path, rules_content);

    println!("✓ Successfully connected Dual-Engine Suite to Cursor at {}", target_path.display());
    println!("  🧠 AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    println!("  📝 Created/updated .cursorrules for persistent memory & AST context");
    Ok(())
}

fn connect_windsurf(home: &Path, exe_path: &str) -> Result<()> {
    let windsurf_config = home.join(".codeium").join("windsurf").join("mcp_config.json");
    merge_mcp_config(&windsurf_config, exe_path)?;

    println!("✓ Successfully connected Dual-Engine Suite to Windsurf at {}", windsurf_config.display());
    println!("  🧠 AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    Ok(())
}

fn connect_cline(home: &Path, exe_path: &str) -> Result<()> {
    // 1. Cline MCP settings
    #[cfg(target_os = "macos")]
    let cline_path = home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(target_os = "windows")]
    let cline_path = home.join("AppData/Roaming/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let cline_path = home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json");

    merge_mcp_config(&cline_path, exe_path)?;

    // 2. Roo Code MCP settings (if folder exists or create it)
    #[cfg(target_os = "macos")]
    let roo_path = home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(target_os = "windows")]
    let roo_path = home.join("AppData/Roaming/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let roo_path = home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json");

    if let Some(parent) = roo_path.parent() {
        if parent.exists() {
            let _ = merge_mcp_config(&roo_path, exe_path);
        }
    }

    println!("✓ Successfully connected Dual-Engine Suite to Cline / Roo Code at {}", cline_path.display());
    println!("  🧠 AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    Ok(())
}

fn connect_codex(workspace_root: &Path, exe_path: &str) -> Result<()> {
    let codex_config = workspace_root.join(".codex").join("config.json");
    merge_mcp_config(&codex_config, exe_path)?;

    println!("✓ Successfully connected Dual-Engine Suite to Codex CLI at {}", codex_config.display());
    println!("  🧠 AutoMemory Pro (Agent Memory & Trajectory Distiller)");
    println!("  🌳 Codebase Memory Companion (Tree-Sitter AST & Call Graph)");
    Ok(())
}
