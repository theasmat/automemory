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
            connect_cursor(&cwd, &exe_path)?;
        }
        other => {
            println!("Unknown agent host: '{}'. Supported: agy, claude, cursor", other);
            return Ok(());
        }
    }

    Ok(())
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

    mcp_val["mcpServers"]["automemory"] = serde_json::json!({
        "command": exe_path,
        "args": ["mcp"],
        "env": {
            "AUTOMEMORY_PROJECT_NAME": "auto"
        }
    });
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

    println!("✓ Successfully connected AutoMemory Pro to Antigravity workspace at {}", workspace_root.display());
    println!("  - Registered MCP server in: .agents/mcp_config.json & .mcp.json");
    println!("  - Registered lifecycle hooks in: .agents/hooks.json");
    println!("  - Created workspace plugin in: .agents/plugins/automemory/");
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
    fs::write(&mcp_path, serde_json::to_string_pretty(&mcp_val)?)?;

    println!("✓ Successfully registered AutoMemory Pro in global Antigravity config at {}", mcp_path.display());
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

    fs::write(&claude_config, serde_json::to_string_pretty(&val)?)?;

    // Also write to workspace .mcp.json
    let ws_mcp = workspace_root.join(".mcp.json");
    fs::write(&ws_mcp, serde_json::to_string_pretty(&val)?)?;

    println!("✓ Successfully connected AutoMemory Pro to Claude Code config at {}", claude_config.display());
    Ok(())
}

fn connect_cursor(workspace_root: &Path, exe_path: &str) -> Result<()> {
    let cursor_dir = workspace_root.join(".cursor");
    fs::create_dir_all(&cursor_dir)?;
    let cursor_mcp = cursor_dir.join("mcp.json");

    let mut val = if cursor_mcp.exists() {
        serde_json::from_str(&fs::read_to_string(&cursor_mcp)?).unwrap_or_else(|_| serde_json::json!({}))
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

    fs::write(&cursor_mcp, serde_json::to_string_pretty(&val)?)?;
    println!("✓ Successfully connected AutoMemory Pro to Cursor at {}", cursor_mcp.display());
    Ok(())
}
