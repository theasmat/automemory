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

    if agent.eq_ignore_ascii_case("all") {
        return connect_all(&home, &cwd, &exe_path, global);
    }
    match super::adapter::find_adapter(agent) {
        Some(adapter) => {
            if !matches!(adapter.id, "agy" | "claude" | "codex") {
                println!(
                    "note: '{}' is experimental (MCP config only, untested in v1).\n",
                    adapter.id
                );
            }
            super::adapter::connect_via(adapter, &home, &cwd, &exe_path, global)
        }
        None => {
            println!("Unknown agent: '{}'. Supported: agy, claude, claude-desktop, cursor, windsurf, continue, cline, roo, zed, codex, opencode, goose, kilo, aider, warp, gemini-cli, devin, copilot, openclaw, hermes, all", agent);
            Ok(())
        }
    }
}

/// Reads a JSON config. A missing or blank file is an empty object; a file
/// that exists but does not parse is an error, so we never overwrite (and
/// destroy) a config the user or the agent is still editing.
pub(crate) fn read_json_or_empty(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "refusing to modify {}: not valid JSON ({e}). Fix or move the file, then re-run connect.",
            path.display()
        )
    })
}

/// Writes via a temp file and rename so a reader never sees a half-written config.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn codebase_memory_config() -> serde_json::Value {
    serde_json::json!({
        "command": "codebase-memory-mcp",
        "args": []
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
    let mut val = read_json_or_empty(&target_file)?;

    if val.get("mcpServers").is_none() {
        val["mcpServers"] = serde_json::json!({});
    }

    val["mcpServers"]["automemory"] = automemory_mcp_config(exe_path);
    val["mcpServers"]["codebase-memory"] = codebase_memory_config();

    write_atomic(target_file, &serde_json::to_string_pretty(&val)?)?;
    Ok(())
}

// ── Antigravity / Gemini ────────────────────────────────────────────────────
pub(crate) fn connect_agy(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    if global {
        let gemini_config = home.join(".gemini").join("config");
        fs::create_dir_all(&gemini_config)?;

        let mcp_path = gemini_config.join("mcp_config.json");
        merge_mcp_config(&mcp_path, exe_path)?;

        let global_skill_dir = gemini_config.join("skills").join("automemory");
        let _ = fs::create_dir_all(&global_skill_dir);
        let _ = fs::write(
            global_skill_dir.join("SKILL.md"),
            automemory_skill_content(),
        );

        let plugin_skill_dir = gemini_config
            .join("plugins")
            .join("automemory")
            .join("skills")
            .join("automemory");
        let _ = fs::create_dir_all(&plugin_skill_dir);
        let _ = fs::write(
            plugin_skill_dir.join("SKILL.md"),
            automemory_skill_content(),
        );

        println!(
            "✓ [GLOBAL] Connected to Antigravity / Gemini at {}",
            mcp_path.display()
        );
        println!(
            "  🧠 Installed global skill: {}",
            global_skill_dir.join("SKILL.md").display()
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
        // Antigravity loads hooks from .agents/hooks.json AND from plugin folders; registering
        // both ran every hook twice. Keep the workspace file only (and drop an older copy).
        let _ = fs::remove_file(plugin_dir.join("hooks.json"));

        // 5. Skills (Workspace & Plugin)
        let ws_skill_dir = agents_dir.join("skills").join("automemory");
        fs::create_dir_all(&ws_skill_dir)?;
        fs::write(ws_skill_dir.join("SKILL.md"), automemory_skill_content())?;

        let plugin_skill_dir = plugin_dir.join("skills").join("automemory");
        fs::create_dir_all(&plugin_skill_dir)?;
        fs::write(
            plugin_skill_dir.join("SKILL.md"),
            automemory_skill_content(),
        )?;

        // 6. Rules & Guidelines (AGENTS.md, GEMINI.md, .agents/rules/automemory.md)
        let rules_dir = agents_dir.join("rules");
        fs::create_dir_all(&rules_dir)?;
        fs::write(rules_dir.join("automemory.md"), automemory_rules_content())?;

        let agents_md = workspace_root.join("AGENTS.md");
        if !agents_md.exists() {
            let _ = fs::write(&agents_md, automemory_rules_content());
        }
        let gemini_md = workspace_root.join("GEMINI.md");
        if !gemini_md.exists() {
            let _ = fs::write(&gemini_md, automemory_rules_content());
        }

        // 7. Git post-commit hook
        let _ = install_git_post_commit_hook(workspace_root, exe_path);

        println!(
            "✓ [WORKSPACE] Connected to Antigravity at {}",
            workspace_root.display()
        );
        println!("  🪝 Registered hooks in: .agents/hooks.json");
        println!("  📦 Created plugin in:   .agents/plugins/automemory/");
        println!("  🧠 Installed skill in:   .agents/skills/automemory/SKILL.md");
        println!("  📜 Generated rules in:   AGENTS.md, GEMINI.md, .agents/rules/");
        println!("  ⚓ Git post-commit hook: active");
    }

    Ok(())
}

// ── Claude Code / Codex native hooks ────────────────────────────────────────
/// Substring identifying hook commands written by `connect`; used to replace
/// our entries idempotently and to strip them on `disconnect`.
pub(crate) const HOOK_MARKER: &str = " hook --agent ";

/// (hook event name, matcher, `automemory hook --event` value)
type HookSpec = (&'static str, Option<&'static str>, &'static str);

pub(crate) const CLAUDE_HOOKS: &[HookSpec] = &[
    ("SessionStart", Some("startup|resume|clear|compact"), "boot"),
    ("UserPromptSubmit", None, "prompt"),
    ("PreToolUse", Some("*"), "pre-tool"),
    ("PostToolUse", Some("*"), "post-tool"),
    ("PostToolUseFailure", Some("*"), "post-tool-fail"),
    ("Stop", None, "stop"),
    ("SessionEnd", None, "session-end"),
];

pub(crate) const CODEX_HOOKS: &[HookSpec] = &[
    ("SessionStart", Some("startup|resume|clear|compact"), "boot"),
    ("UserPromptSubmit", None, "prompt"),
    ("PreToolUse", Some("*"), "pre-tool"),
    ("PostToolUse", Some("*"), "post-tool"),
    ("Stop", None, "stop"),
    ("SessionEnd", None, "session-end"),
];

fn is_automemory_group(group: &serde_json::Value) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .is_some_and(|hs| {
            hs.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .is_some_and(|c| c.contains(HOOK_MARKER) && c.contains("--project auto"))
            })
        })
}

fn shell_quote_exe(exe_path: &str) -> String {
    if exe_path.contains(char::is_whitespace) {
        format!("\"{exe_path}\"")
    } else {
        exe_path.to_string()
    }
}

/// Merges our hooks into a Claude Code / Codex style JSON file
/// (`{"hooks": {Event: [{matcher, hooks: [{type, command, timeout}]}]}}`).
/// Existing user hooks and other keys are preserved; our own entries are replaced.
pub(crate) fn merge_native_hooks(
    path: &Path,
    exe_path: &str,
    agent: &str,
    specs: &[HookSpec],
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut root: serde_json::Value = read_json_or_empty(path)?;
    if !root.is_object() {
        anyhow::bail!(
            "refusing to modify {}: top-level JSON is not an object",
            path.display()
        );
    }
    if !root.get("hooks").is_some_and(|h| h.is_object()) {
        root["hooks"] = serde_json::json!({});
    }
    let exe = shell_quote_exe(exe_path);
    for (event, matcher, cli_event) in specs {
        let hooks = &mut root["hooks"];
        let mut groups: Vec<serde_json::Value> = hooks
            .get(*event)
            .and_then(|g| g.as_array())
            .cloned()
            .unwrap_or_default();
        groups.retain(|g| !is_automemory_group(g));
        let mut group = serde_json::json!({
            "hooks": [{
                "type": "command",
                "command": format!("{exe} hook --agent {agent} --event {cli_event} --project auto"),
                "timeout": 5
            }]
        });
        if let Some(m) = matcher {
            group["matcher"] = serde_json::json!(m);
        }
        groups.push(group);
        hooks[*event] = serde_json::Value::Array(groups);
    }
    write_atomic(path, &serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// Removes only our hook groups. Returns true when the file changed.
/// Empty event arrays and an empty `hooks` object are pruned; the file is
/// never deleted.
pub(crate) fn strip_native_hooks(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let Ok(mut root) = serde_json::from_str::<serde_json::Value>(&fs::read_to_string(path)?) else {
        return Ok(false);
    };
    let Some(hooks) = root.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return Ok(false);
    };
    let mut changed = false;
    for groups in hooks.values_mut() {
        if let Some(arr) = groups.as_array_mut() {
            let before = arr.len();
            arr.retain(|g| !is_automemory_group(g));
            changed |= arr.len() != before;
        }
    }
    hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    if hooks.is_empty() {
        root.as_object_mut().map(|o| o.remove("hooks"));
    }
    if changed {
        write_atomic(path, &serde_json::to_string_pretty(&root)?)?;
    }
    Ok(changed)
}

/// Codex reads MCP servers from `config.toml` (`[mcp_servers.<name>]`), not JSON.
/// Appends our table only when absent so user comments and ordering survive.
pub(crate) fn merge_codex_toml(path: &Path, exe_path: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if text.contains("[mcp_servers.automemory]") {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
    let exe = serde_json::to_string(exe_path)?; // JSON string == valid TOML basic string
    text.push_str(&format!(
        "[mcp_servers.automemory]\ncommand = {exe}\nargs = [\"mcp\"]\n\n[mcp_servers.automemory.env]\nAUTOMEMORY_PROJECT_NAME = \"auto\"\n"
    ));
    fs::write(path, text)?;
    Ok(())
}

/// Removes `[mcp_servers.automemory]` and its sub-tables from a Codex TOML file.
pub(crate) fn strip_codex_toml(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    let mut skipping = false;
    let mut changed = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            skipping = t == "[mcp_servers.automemory]" || t.starts_with("[mcp_servers.automemory.");
        }
        if skipping {
            changed = true;
        } else {
            out.push(line);
        }
    }
    if changed {
        let mut joined = out.join("\n");
        joined = joined.trim_end().to_string();
        if !joined.is_empty() {
            joined.push('\n');
        }
        fs::write(path, joined)?;
    }
    Ok(changed)
}

// ── Claude Code & Claude Desktop ────────────────────────────────────────────
pub(crate) fn connect_claude(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
        let settings = home.join(".claude").join("settings.json");
        merge_native_hooks(&settings, exe_path, "claude", CLAUDE_HOOKS)?;
        println!("  🪝 Registered hooks in: {}", settings.display());
    } else {
        let ws_mcp = workspace_root.join(".mcp.json");
        merge_mcp_config(&ws_mcp, exe_path)?;

        let claude_md = workspace_root.join("CLAUDE.md");
        if !claude_md.exists() {
            let _ = fs::write(&claude_md, automemory_rules_content());
        }
        let _ = install_git_post_commit_hook(workspace_root, exe_path);

        println!("✓ [WORKSPACE] Connected to Claude at {}", ws_mcp.display());
        // Machine-specific binary path: keep out of the committed settings.json.
        let settings = workspace_root.join(".claude").join("settings.local.json");
        merge_native_hooks(&settings, exe_path, "claude", CLAUDE_HOOKS)?;
        println!("  🪝 Registered hooks in: {}", settings.display());
        println!("  📝 Guidelines configured in CLAUDE.md");
        println!("  ⚓ Git post-commit hook: active");
    }

    Ok(())
}

// ── Cursor ──────────────────────────────────────────────────────────────────
/// Exact guidelines text written by `connect_cursor`. `disconnect` deletes
/// `.cursorrules` only on byte-exact match, never user-edited content.
pub(crate) const CURSOR_RULES: &str = r#"# AutoMemory Persistent Intelligence Rules
- You have persistent cognitive memory via `automemory` MCP server (`memory.context`, `memory.recall`, `memory.save`, `trace.save`, `memory.proposals.summary`, `memory.proposals.decide_all`).
- You have deep AST code intelligence via `codebase-memory` MCP server (`index_repository`, `query_graph`, `trace_path`).
- At the start of a task, call `memory.context` to load prior decisions, learnings, and golden trajectories.
- Zero-SQL Rule: Never run raw sqlite3 queries on internal databases. Use the official MCP tools and CLI commands.
"#;
pub(crate) fn connect_cursor_unified(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    connect_cursor(workspace_root, home, exe_path, global)
}

pub(crate) fn connect_cursor(
    workspace_root: &Path,
    home: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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

    // Create .cursorrules in workspace (exact template match required to
    // remove it again in `disconnect`; see CURSOR_RULES).
    let cursorrules_path = workspace_root.join(".cursorrules");
    let _ = fs::write(&cursorrules_path, CURSOR_RULES);
    let _ = install_git_post_commit_hook(workspace_root, exe_path);

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Cursor at {}",
        scope_tag,
        target_path.display()
    );
    println!("  📝 Created/updated .cursorrules guidelines");
    println!("  ⚓ Git post-commit hook: active");
    Ok(())
}

// ── Windsurf / Codeium Cascade ──────────────────────────────────────────────
pub(crate) fn connect_windsurf(
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
pub(crate) fn connect_cline(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_roo(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_continue(
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

    let mut val = read_json_or_empty(&target_path)?;

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

    write_atomic(&target_path, &serde_json::to_string_pretty(&val)?)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Continue.dev at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── Zed Editor ──────────────────────────────────────────────────────────────
pub(crate) fn connect_zed(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    let target_path = if global {
        home.join(".config").join("zed").join("settings.json")
    } else {
        workspace_root.join(".zed").join("settings.json")
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut val = read_json_or_empty(&target_path)?;

    if val.get("context_servers").is_none() {
        val["context_servers"] = serde_json::json!({});
    }

    val["context_servers"]["automemory"] = serde_json::json!({
        "command": {
            "path": exe_path,
            "args": ["mcp"]
        }
    });

    write_atomic(&target_path, &serde_json::to_string_pretty(&val)?)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Zed editor at {}",
        scope_tag,
        target_path.display()
    );
    Ok(())
}

// ── OpenAI Codex CLI ────────────────────────────────────────────────────────
pub(crate) fn connect_codex(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    let dir = if global {
        home.join(".codex")
    } else {
        workspace_root.join(".codex")
    };
    let config_toml = dir.join("config.toml");
    merge_codex_toml(&config_toml, exe_path)?;
    let hooks_json = dir.join("hooks.json");
    merge_native_hooks(&hooks_json, exe_path, "codex", CODEX_HOOKS)?;

    let scope_tag = if global { "[GLOBAL]" } else { "[WORKSPACE]" };
    println!(
        "✓ {} Connected to Codex CLI at {}",
        scope_tag,
        config_toml.display()
    );
    println!("  🪝 Registered hooks in: {}", hooks_json.display());
    println!("  ⚠️  Codex skips new hooks until trusted: run `codex`, open /hooks, review and trust them.");
    if !global {
        println!("     Project-level .codex/ is only loaded for trusted projects.");
    }
    Ok(())
}

// ── Connect All Agents ──────────────────────────────────────────────────────
pub(crate) fn connect_all(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
    println!("========================================================");
    println!("       AutoMemory Multi-Agent Universal Connector       ");
    println!("========================================================");

    // v1 supports three agents. The other connectors write MCP config only and are
    // untested, so they are connected one at a time on request (`connect <name>`).
    let mut failures = Vec::new();
    for (name, result) in [
        ("agy", connect_agy(home, workspace_root, exe_path, global)),
        (
            "claude",
            connect_claude(home, workspace_root, exe_path, global),
        ),
        (
            "codex",
            connect_codex(home, workspace_root, exe_path, global),
        ),
    ] {
        if let Err(e) = result {
            eprintln!("✗ {name}: {e}");
            failures.push(name);
        }
    }

    println!("========================================================");
    if failures.is_empty() {
        println!("✓ Connected the supported agents: agy, claude, codex.");
    }
    println!("  Experimental (MCP only, untested): run `automemory connect <name>` for any of");
    println!("  claude-desktop, cursor, windsurf, continue, cline, roo, kilo, zed, opencode,");
    println!("  goose, aider, warp, gemini-cli, devin, copilot, openclaw, hermes.");
    println!("========================================================");
    if !failures.is_empty() {
        anyhow::bail!("failed to connect: {}", failures.join(", "));
    }
    Ok(())
}

// ── OpenCode ─────────────────────────────────────────────────────────────────
pub(crate) fn connect_opencode(
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
pub(crate) fn connect_goose(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_kilo(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_aider(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_warp(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_gemini_cli(
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

    let mut val = read_json_or_empty(&target_path)?;

    if val.get("mcpServers").is_none() {
        val["mcpServers"] = serde_json::json!({});
    }
    val["mcpServers"]["automemory"] = automemory_mcp_config(exe_path);

    write_atomic(&target_path, &serde_json::to_string_pretty(&val)?)?;

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
pub(crate) fn connect_devin(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_copilot(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_openclaw(
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
pub(crate) fn connect_hermes(
    home: &Path,
    workspace_root: &Path,
    exe_path: &str,
    global: bool,
) -> Result<()> {
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
pub(crate) fn connect_claude_desktop(
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
pub(crate) fn get_vscode_user_dir(home: &Path) -> PathBuf {
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

pub(crate) fn automemory_skill_content() -> &'static str {
    r#"---
name: automemory
description: >
  Autonomous multi-agent memory, zero-staleness blackboard, cross-worktree advisory leases,
  and automatic session distillation. Use whenever working on codebases with AutoMemory enabled
  to automatically recall context, lease files before editing, sync decisions, record lessons,
  and update memories.
argument-hint: "[context|status|lease|record]"
license: MIT
---

# AutoMemory Autonomous Agent Skill

You are operating with **AutoMemory**—the autonomous cognitive memory, safety guardrail, and multi-agent coordination layer. AutoMemory automatically tracks your actions via out-of-process hooks, prevents collisions across git worktrees, and distills durable memories and success traces.

Follow this autonomous protocol for all tasks in this repository.

---

## 1. Task Initialization & Context Boot
At the start of any new task, feature, or refactoring:
1. **Load Active Project Context**:
   - Call MCP tool `memory.constitution` or `memory.context` to retrieve hot architectural invariants, active rules, and past gotchas.
   - Call `decision.get` for relevant domains (e.g. `architecture`, `database`, `concurrency`) to ensure your implementation complies with active decisions.
   - Call `doc.list` if the task involves specifications, plans, or PRDs.
2. **Check for Inbound Handoff**:
   - If continuing work from another agent, call `baton.pickup` with your session ID to receive the prior agent's token-budgeted handoff packet.

---

## 2. Resource Leases (Multi-Agent & Worktree Safety)
Before modifying or refactoring shared files in multi-agent or parallel worktree environments:
1. **Acquire an Advisory Lease**:
   - Call `lock.acquire` with:
     - `scope`: relative file path (e.g. `src/auth.c`), glob (`src/models/*.rs`), or symbol (`auth::login`).
     - `goal`: concise description of what you are changing and why.
     - `ttl_seconds`: estimated duration (e.g. 300 to 600s).
   - If granted: proceed with your edits.
   - If conflict: inspect `holder_agent`, `goal`, and `remaining_seconds`. Choose another task or wait for expiration.
2. **Release Upon Completion**:
   - When file edits and tests are complete, call `lock.release` to free the resource for peer agents.

---

## 3. Autonomous Architectural Decisions & Checkpoints
When you establish a key architectural policy, choose between alternative libraries/approaches, or resolve a major design choice:
1. **Record the Decision**:
   - Call `decision.record` with:
     - `topic`: domain name (e.g. `storage-engine`, `auth-flow`).
     - `decision`: clear 1-line statement of the chosen policy.
     - `reason`: justification and rationale.
     - `alternatives`: list of rejected options.
   - This propagates immediately with **0ms staleness** to all parallel agents and worktrees.
2. **Milestone Memories**:
   - While AutoMemory's hooks automatically distill sessions upon test success and git commits, you can explicitly preserve critical domain insights or patterns using `memory.save(category="architecture|lesson|pattern", ...)`.

---

## 4. Failure Recovery & Error Diagnosis
When encountering an unexpected build error, test failure, or runtime issue:
1. Diagnose the root cause using codebase evidence.
2. Apply the verified fix.
3. If the bug was subtle or dangerous, call `failure.save` or `memory.save` with `category: "lesson"` to prevent future agents from repeating the same mistake.

---

## 5. Task Handoff & Completion
When finishing your assigned task or when reaching context window limits:
1. Release all active file leases (`lock.release`).
2. If another agent or subsequent session will continue the work, call `baton.pass` with:
   - `addressed_to`: name of receiving agent or role.
   - `context`: concise summary of work done and immediate next steps.
   - `artifacts`: list of files modified or created.
"#
}

pub(crate) fn automemory_rules_content() -> &'static str {
    r#"# AutoMemory Multi-Agent Coordination Guidelines

This workspace is integrated with **AutoMemory**.
- **Context Awareness**: At the start of tasks, call `memory.context` or `memory.constitution` to load project invariants and active decisions.
- **Mutual Exclusion**: Before modifying shared files, call `lock.acquire(scope="<file>", goal="<goal>")` to prevent collisions with parallel agents in other worktrees. Release with `lock.release` when done.
- **Architectural Policy**: Record non-trivial architectural decisions with `decision.record` so all parallel agents see them with 0ms latency.
- **Handoffs**: Pass context to peer workers using `baton.pass`.
- **Pre-Tool Safety**: Pre-tool hooks intercept destructive operations (e.g. recursive root deletions, force pushes). Never attempt raw destructive shell commands.
"#
}

pub(crate) fn install_git_post_commit_hook(workspace_root: &Path, exe_path: &str) -> Result<()> {
    let git_dir = if let Some(canonical) =
        automemory_core::sync::GitSync::resolve_git_canonical_root(workspace_root)
    {
        canonical.join(".git")
    } else {
        workspace_root.join(".git")
    };

    if !git_dir.exists() {
        return Ok(());
    }

    let hooks_dir = git_dir.join("hooks");
    let _ = fs::create_dir_all(&hooks_dir);
    let post_commit_path = hooks_dir.join("post-commit");

    let script = format!(
        r#"#!/bin/sh
# AutoMemory Git Post-Commit Hook (auto-updates memory on commit)
AUTOMEMORY_EXE="{}"
if [ -x "$AUTOMEMORY_EXE" ]; then
    "$AUTOMEMORY_EXE" hook --agent git --event post-commit --project auto >/dev/null 2>&1 || true
fi
"#,
        exe_path
    );

    fs::write(&post_commit_path, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&post_commit_path, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}
