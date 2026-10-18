//! `automemory disconnect <agent>`: remove exactly what `connect` wrote.
//!
//! Safety contract (each op is conservative by design):
//! - MCP JSON files: only the `mcpServers.automemory` key is removed. The
//!   `codebase-memory` entry and everything else is left intact, and files
//!   themselves are never deleted.
//! - `.agents/hooks.json`: only the top-level `automemory` key is removed;
//!   the file is deleted only if nothing remains.
//! - `.cursorrules`: deleted only on byte-exact match with our template.
//! - YAML blocks: removed only on exact shape match, else reported manual.
//! - Anything unrecognized prints manual removal steps instead of guessing.

use anyhow::Result;
use automemory_core::ports::AgentAdapter;
use std::fs;
use std::path::{Path, PathBuf};

use super::adapter::find_adapter;
use super::connect::{get_vscode_user_dir, CURSOR_RULES};

/// One removal operation derived from an agent's connect paths.
enum Removal {
    /// Strip `mcpServers.automemory` from a JSON MCP config file.
    McpServerJson(PathBuf),
    /// Delete `.cursorrules` on exact template match.
    CursorRules(PathBuf),
    /// Strip our appended block from a Goose YAML config.
    GooseYaml(PathBuf),
    /// Strip our appended block from an Aider YAML config.
    AiderYaml(PathBuf),
    /// Strip our entry from a Continue.dev JSON config array.
    ContinueJson(PathBuf),
    /// Strip `context_servers.automemory` from a Zed JSON config.
    ZedJson(PathBuf),
    /// Strip our groups from a Claude Code / Codex hooks JSON (`hooks.<Event>[]`).
    NativeHooks(PathBuf),
    /// Strip the `automemory` key from an Antigravity hooks.json (other hooks stay).
    AgyHooks(PathBuf),
    /// Delete a folder we created (skill or legacy plugin) when only our files are in it.
    OwnedDir(PathBuf),
    /// Delete a file only if it still holds exactly what `connect` wrote.
    OwnedFile(PathBuf, String),
    /// Remove our block from `.git/hooks/post-commit`.
    GitHook(PathBuf),
    /// Remove our marked block from an agent instruction file (the rest of the file stays).
    RulesBlock(PathBuf, &'static str),
    /// Strip `[mcp_servers.automemory]` tables from a Codex `config.toml`.
    CodexToml(PathBuf),
}

/// Mirror of each `connect_*` target path in `connect.rs`. Workspace paths
/// resolve under `cwd`, global paths under `home`, same as connect.
fn targets(agent: &str, home: &Path, cwd: &Path, global: bool) -> Result<Vec<Removal>> {
    let ws = |parts: &[&str]| {
        let mut p = cwd.to_path_buf();
        for part in parts {
            p = p.join(part);
        }
        p
    };
    #[cfg(target_os = "macos")]
    let code_settings = |home: &Path, ext: &str| {
        home.join("Library/Application Support/Code/User/globalStorage")
            .join(ext)
            .join("settings/cline_mcp_settings.json")
    };
    #[cfg(target_os = "windows")]
    let code_settings = |home: &Path, ext: &str| {
        home.join("AppData/Roaming/Code/User/globalStorage")
            .join(ext)
            .join("settings/cline_mcp_settings.json")
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let code_settings = |home: &Path, ext: &str| {
        home.join(".config/Code/User/globalStorage")
            .join(ext)
            .join("settings/cline_mcp_settings.json")
    };

    let ops = match agent {
        "agy" => {
            if global {
                let g = home.join(".gemini");
                vec![
                    Removal::McpServerJson(g.join("config/mcp_config.json")),
                    Removal::AgyHooks(g.join("config/hooks.json")),
                    Removal::OwnedDir(g.join("config/skills/automemory")),
                    Removal::OwnedDir(g.join("antigravity-cli/skills/automemory")),
                    Removal::OwnedDir(g.join("config/plugins/automemory")),
                ]
            } else {
                vec![
                    Removal::McpServerJson(ws(&[".agents", "mcp_config.json"])),
                    Removal::AgyHooks(ws(&[".agents", "hooks.json"])),
                    Removal::OwnedDir(ws(&[".agents", "skills", "automemory"])),
                    Removal::OwnedDir(ws(&[".agents", "plugins", "automemory"])),
                    Removal::OwnedFile(
                        ws(&[".agents", "rules", "automemory.md"]),
                        super::connect::automemory_rules_content().to_string(),
                    ),
                    Removal::OwnedFile(
                        ws(&["AGENTS.md"]),
                        super::connect::automemory_rules_content().to_string(),
                    ),
                    Removal::OwnedFile(
                        ws(&["GEMINI.md"]),
                        super::connect::automemory_rules_content().to_string(),
                    ),
                ]
            }
        }
        "claude" => vec![
            Removal::McpServerJson(if global {
                home.join(".claude.json")
            } else {
                ws(&[".mcp.json"])
            }),
            Removal::NativeHooks(if global {
                home.join(".claude/settings.json")
            } else {
                ws(&[".claude", "settings.local.json"])
            }),
            Removal::OwnedFile(
                ws(&["CLAUDE.md"]),
                super::connect::automemory_rules_content().to_string(),
            ),
        ],
        "claude-desktop" => {
            #[cfg(target_os = "macos")]
            let p = home.join("Library/Application Support/Claude/claude_desktop_config.json");
            #[cfg(target_os = "windows")]
            let p = home.join("AppData/Roaming/Claude/claude_desktop_config.json");
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let p = home.join(".config/Claude/claude_desktop_config.json");
            vec![Removal::McpServerJson(p)]
        }
        "cursor" => {
            let mut ops = vec![Removal::McpServerJson(if global {
                #[cfg(target_os = "macos")]
                let p = home.join("Library/Application Support/Cursor/User/globalStorage/mcp.json");
                #[cfg(target_os = "windows")]
                let p = home.join("AppData/Roaming/Cursor/User/globalStorage/mcp.json");
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                let p = home.join(".config/Cursor/User/globalStorage/mcp.json");
                p
            } else {
                ws(&[".cursor", "mcp.json"])
            })];
            if !global {
                ops.push(Removal::CursorRules(ws(&[".cursorrules"])));
            }
            ops
        }
        "windsurf" => vec![Removal::McpServerJson(if global {
            home.join(".codeium").join("windsurf/mcp_config.json")
        } else {
            ws(&[".windsurf", "mcp.json"])
        })],
        "cline" => vec![Removal::McpServerJson(if global {
            code_settings(home, "saoudrizwan.claude-dev")
        } else {
            ws(&[".cline", "mcp.json"])
        })],
        "roo" => vec![Removal::McpServerJson(if global {
            code_settings(home, "rooveterinaryinc.roo-cline")
        } else {
            ws(&[".roo", "mcp.json"])
        })],
        "kilo" => vec![Removal::McpServerJson(if global {
            code_settings(home, "kilocode.kilo-code")
        } else {
            ws(&[".kilo", "mcp.json"])
        })],
        "continue" => vec![Removal::ContinueJson(if global {
            home.join(".continue/config.json")
        } else {
            ws(&[".continue", "config.json"])
        })],
        "zed" => vec![Removal::ZedJson(if global {
            home.join(".config/zed/settings.json")
        } else {
            ws(&[".zed", "settings.json"])
        })],
        "codex" => {
            let dir = if global {
                home.join(".codex")
            } else {
                ws(&[".codex"])
            };
            vec![
                // Legacy JSON config written by older versions.
                Removal::McpServerJson(dir.join("config.json")),
                Removal::CodexToml(dir.join("config.toml")),
                Removal::NativeHooks(dir.join("hooks.json")),
            ]
        }
        "opencode" => vec![Removal::McpServerJson(if global {
            home.join(".opencode/config.json")
        } else {
            ws(&[".opencode", "config.json"])
        })],
        "goose" => vec![Removal::GooseYaml(if global {
            home.join(".config/goose/config.yaml")
        } else {
            ws(&[".goose", "config.yaml"])
        })],
        "aider" => vec![Removal::AiderYaml(if global {
            home.join(".aider.conf.yml")
        } else {
            ws(&[".aider.conf.yml"])
        })],
        "warp" => vec![Removal::McpServerJson(if global {
            home.join(".warp/mcp.json")
        } else {
            ws(&[".warp", "mcp.json"])
        })],
        "gemini-cli" => vec![Removal::McpServerJson(if global {
            home.join(".gemini/settings.json")
        } else {
            ws(&[".gemini", "settings.json"])
        })],
        "devin" => vec![Removal::McpServerJson(if global {
            home.join(".devin/mcp.json")
        } else {
            ws(&[".devin", "mcp.json"])
        })],
        "copilot" => vec![Removal::McpServerJson(if global {
            get_vscode_user_dir(home).join("mcp.json")
        } else {
            ws(&[".vscode", "mcp.json"])
        })],
        "openclaw" => vec![Removal::McpServerJson(if global {
            home.join(".openclaw/mcp.json")
        } else {
            ws(&[".openclaw", "mcp.json"])
        })],
        "hermes" => vec![Removal::McpServerJson(if global {
            home.join(".hermes/mcp.json")
        } else {
            ws(&[".hermes", "mcp.json"])
        })],
        other => anyhow::bail!("Unknown agent: '{other}'. See `automemory adapter list`."),
    };
    Ok(ops)
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn write_json(path: &Path, val: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(val)?)?;
    Ok(())
}

/// Removes `mcpServers.automemory` (only our key) and prunes the emptied
/// parent object. Returns true when the file changed.
fn strip_mcp_server(path: &Path) -> Result<bool> {
    let mut val = match read_json(path) {
        Some(v) => v,
        None => return Ok(false),
    };
    let changed = val
        .get_mut("mcpServers")
        .and_then(|servers| servers.as_object_mut())
        .map(|servers| servers.remove("automemory").is_some())
        .unwrap_or(false);
    if !changed {
        return Ok(false);
    }
    if val
        .get("mcpServers")
        .and_then(|s| s.as_object())
        .map(|s| s.is_empty())
        .unwrap_or(false)
    {
        if let Some(obj) = val.as_object_mut() {
            obj.remove("mcpServers");
        }
    }
    write_json(path, &val)?;
    Ok(true)
}

/// Removes our appended Goose block only on exact shape match:
/// `- name: automemory` + `type: stdio` + `cmd:` mentioning automemory +
/// `args:` + `- mcp`, directly under an `extensions:` header.
fn strip_goose_block(path: &Path) -> Result<bool> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| {
        let t = l.trim_start();
        t.starts_with("- name:") && t.contains("automemory")
    });
    let Some(start) = start else {
        return Ok(false);
    };
    let body: Vec<&str> = lines[start + 1..].iter().take(4).copied().collect();
    let shape_ok = body.len() == 4
        && body[0].trim_start().starts_with("type: stdio")
        && body[1].trim_start().starts_with("cmd:")
        && body[1].contains("automemory")
        && body[2].trim_start().starts_with("args:")
        && body[3].trim() == "- mcp";
    if !shape_ok {
        return Ok(false);
    }
    // Our block is exactly 5 lines (item + type/cmd/args/- mcp); the scan
    // below only advances past deeper-indented user lines that belong to
    // our own item, stopping at the next list entry, dedent, or EOF.
    let mut end = start + 5;
    while end < lines.len() {
        let t = lines[end];
        let trimmed = t.trim_start();
        let indent = t.len() - trimmed.len();
        let item_indent = lines[start].len() - lines[start].trim_start().len();
        if trimmed.starts_with("- ") && indent <= item_indent {
            break;
        }
        if !t.trim().is_empty() && indent < item_indent {
            break;
        }
        end += 1;
    }
    let mut kept: Vec<&str> = lines[..start].to_vec();
    kept.extend_from_slice(&lines[end..]);
    // Drop the `extensions:` header only when no list items remain under it
    // (siblings before or after our removed block both count).
    if start > 0 && kept.get(start - 1).map(|l| l.trim()) == Some("extensions:") {
        let items_remain = kept[start..]
            .iter()
            .chain(kept[..start - 1].iter())
            .any(|l| l.starts_with(' ') && l.trim_start().starts_with("- "));
        if !items_remain {
            kept.remove(start - 1);
        }
    }
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(true)
}

/// Removes our Aider block (marker `# AutoMemory MCP server` through the
/// `args: [mcp]` line) plus one preceding blank line.
fn strip_aider_block(path: &Path) -> Result<bool> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == "# AutoMemory MCP server");
    let Some(start) = start else {
        return Ok(false);
    };
    let mut end = start + 1;
    while end < lines.len() && !lines[end].trim_start().starts_with("args:") {
        end += 1;
    }
    if end >= lines.len() || !lines[end].contains("mcp") {
        return Ok(false);
    }
    end += 1;
    let mut kept: Vec<&str> = lines[..start.saturating_sub(1)].to_vec();
    // Keep the preceding line when it is not blank (don't eat user content).
    if start > 0 && !lines[start - 1].trim().is_empty() {
        kept.push(lines[start - 1]);
    }
    kept.extend_from_slice(&lines[end..]);
    let mut out = kept.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(true)
}

/// Removes array entries whose transport command mentions automemory.
fn strip_continue_entry(path: &Path) -> Result<bool> {
    let mut val = match read_json(path) {
        Some(v) => v,
        None => return Ok(false),
    };
    let servers = val
        .get_mut("experimental")
        .and_then(|e| e.get_mut("modelContextProtocolServers"))
        .and_then(|s| s.as_array_mut());
    let Some(servers) = servers else {
        return Ok(false);
    };
    let before = servers.len();
    servers.retain(|s| {
        s.get("transport")
            .and_then(|t| t.get("command"))
            .and_then(|c| c.as_str())
            .map(|cmd| !cmd.contains("automemory"))
            .unwrap_or(true)
    });
    if servers.len() == before {
        return Ok(false);
    }
    write_json(path, &val)?;
    Ok(true)
}

/// Removes `context_servers.automemory` from a Zed settings file.
fn strip_zed_entry(path: &Path) -> Result<bool> {
    let mut val = match read_json(path) {
        Some(v) => v,
        None => return Ok(false),
    };
    let changed = val
        .get_mut("context_servers")
        .and_then(|c| c.as_object_mut())
        .map(|c| c.remove("automemory").is_some())
        .unwrap_or(false);
    if !changed {
        return Ok(false);
    }
    write_json(path, &val)?;
    Ok(true)
}

fn apply(op: &Removal) -> Result<bool> {
    match op {
        Removal::McpServerJson(p) => strip_mcp_server(p),
        Removal::CursorRules(p) => match fs::read_to_string(p) {
            Ok(content) if content == CURSOR_RULES => {
                fs::remove_file(p)?;
                Ok(true)
            }
            Ok(_) => {
                println!(
                    "  ⚠ Kept edited .cursorrules (not our template): {}",
                    p.display()
                );
                Ok(false)
            }
            Err(_) => Ok(false),
        },
        Removal::GooseYaml(p) => strip_goose_block(p),
        Removal::AiderYaml(p) => strip_aider_block(p),
        Removal::ContinueJson(p) => strip_continue_entry(p),
        Removal::ZedJson(p) => strip_zed_entry(p),
        Removal::NativeHooks(p) => super::connect::strip_native_hooks(p),
        Removal::AgyHooks(p) => super::connect::strip_agy_hooks(p),
        Removal::OwnedDir(p) => Ok(remove_owned_dir(p)),
        Removal::OwnedFile(p, expected) => match fs::read_to_string(p) {
            Ok(content) if &content == expected => {
                fs::remove_file(p)?;
                Ok(true)
            }
            Ok(_) => {
                println!("  ⚠ Kept edited file (not our template): {}", p.display());
                Ok(false)
            }
            Err(_) => Ok(false),
        },
        Removal::GitHook(root) => super::connect::remove_git_post_commit_hook(root),
        Removal::RulesBlock(root, path) => {
            let t = super::rules_export::TARGETS.iter().find(|t| t.path == *path).expect("known target");
            super::rules_export::remove_one(root, t)
        }
        Removal::CodexToml(p) => super::connect::strip_codex_toml(p),
    }
}

fn describe(op: &Removal) -> String {
    match op {
        Removal::McpServerJson(p) => format!("MCP server entry in {}", p.display()),
        Removal::CursorRules(p) => format!(".cursorrules at {}", p.display()),
        Removal::GooseYaml(p) => format!("Goose block in {}", p.display()),
        Removal::AiderYaml(p) => format!("Aider block in {}", p.display()),
        Removal::ContinueJson(p) => format!("Continue entry in {}", p.display()),
        Removal::ZedJson(p) => format!("Zed entry in {}", p.display()),
        Removal::NativeHooks(p) => format!("AutoMemory hooks in {}", p.display()),
        Removal::AgyHooks(p) => format!("Antigravity hooks in {}", p.display()),
        Removal::OwnedDir(p) => format!("folder {}", p.display()),
        Removal::OwnedFile(p, _) => format!("generated file {}", p.display()),
        Removal::GitHook(p) => format!("git post-commit block in {}", p.display()),
        Removal::RulesBlock(root, path) => format!("AutoMemory block in {}", root.join(path).display()),
        Removal::CodexToml(p) => format!("Codex MCP table in {}", p.display()),
    }
}

/// A folder we created: only deleted when every file in it is one `connect` writes, so a user's
/// own files in the same place are never lost.
fn remove_owned_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    if dir.file_name().is_some_and(|n| n == "automemory") && dir.parent().is_some_and(|p| p.ends_with("plugins")) {
        return super::connect::remove_owned_plugin_dir(dir);
    }
    let ours = |name: &str| matches!(name, "SKILL.md");
    let only_ours = fs::read_dir(dir)
        .map(|rd| rd.flatten().all(|e| ours(&e.file_name().to_string_lossy())))
        .unwrap_or(false);
    if only_ours {
        fs::remove_dir_all(dir).is_ok()
    } else {
        println!("  ⚠ Kept {} (it holds files AutoMemory did not write)", dir.display());
        false
    }
}

/// Whether the operation still has something of ours to remove (read-only).
fn present(op: &Removal) -> bool {
    let has = |p: &Path| fs::read_to_string(p).is_ok_and(|t| t.contains("automemory"));
    match op {
        Removal::McpServerJson(p)
        | Removal::NativeHooks(p)
        | Removal::AgyHooks(p)
        | Removal::ContinueJson(p)
        | Removal::ZedJson(p)
        | Removal::GooseYaml(p)
        | Removal::AiderYaml(p)
        | Removal::CodexToml(p) => has(p),
        Removal::OwnedDir(p) => p.is_dir(),
        Removal::CursorRules(p) => p.is_file(),
        Removal::OwnedFile(p, expected) => fs::read_to_string(p).is_ok_and(|t| &t == expected),
        Removal::RulesBlock(root, path) => fs::read_to_string(root.join(path)).is_ok_and(|t| t.contains("<!-- automemory:begin")),
        Removal::GitHook(root) => {
            let git = automemory_core::sync::GitSync::resolve_git_canonical_root(root).unwrap_or(root.clone());
            fs::read_to_string(git.join(".git/hooks/post-commit")).is_ok_and(|t| t.contains("AutoMemory") || t.contains("automemory post-commit"))
        }
    }
}

/// Everything AutoMemory has registered for any agent, in the current workspace and globally.
/// Backs `doctor` and the tests that prove a full disconnect leaves nothing behind.
pub fn inventory(home: &Path, cwd: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for is_global in [false, true] {
        for a in super::adapter::ADAPTERS.iter() {
            for op in targets(a.id, home, cwd, is_global).unwrap_or_default() {
                if present(&op) {
                    found.push(format!("{} [{}]: {}", a.id, if is_global { "global" } else { "workspace" }, describe(&op)));
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn scopes(global: bool, everywhere: bool) -> Vec<bool> {
    if everywhere { vec![false, true] } else { vec![global] }
}

/// Removes exactly what `connect` wrote for one agent, or for every agent with `all`. Third-party
/// entries (e.g. a separately configured `codebase-memory` server) are left intact. `dry_run`
/// lists what is registered without changing anything.
pub fn execute(agent: &str, global: bool, everywhere: bool, dry_run: bool) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    run(&home, &cwd, agent, global, everywhere, dry_run)
}

fn run(home: &Path, cwd: &Path, agent: &str, global: bool, everywhere: bool, dry_run: bool) -> Result<()> {
    let ids: Vec<String> = if agent.eq_ignore_ascii_case("all") {
        super::adapter::ADAPTERS.iter().map(|a| a.id.to_string()).collect()
    } else {
        let Some(a) = find_adapter(agent) else {
            anyhow::bail!("Unknown agent: '{agent}'. Use a name from `automemory adapter list`, or `all`.");
        };
        vec![a.adapter_id().to_string()]
    };
    let all = ids.len() > 1;
    let verb = if dry_run { "Registered" } else { "Removing" };
    let mut count = 0;
    for is_global in scopes(global, everywhere) {
        let scope = if is_global { "GLOBAL" } else { "WORKSPACE" };
        println!("{verb} AutoMemory [{scope}] for: {}\n", if all { "all agents".to_string() } else { ids[0].clone() });
        let mut ops = Vec::new();
        for id in &ids {
            ops.extend(targets(id, home, cwd, is_global)?);
        }
        if all && !is_global {
            ops.push(Removal::GitHook(cwd.to_path_buf()));
        }
        if !is_global {
            for t in super::rules_export::TARGETS {
                if all || ids.iter().any(|i| i == t.agent) {
                    ops.push(Removal::RulesBlock(super::rules_export::repo_root(cwd), t.path));
                }
            }
        }
        for op in ops {
            if dry_run {
                if present(&op) {
                    println!("  • {}", describe(&op));
                    count += 1;
                }
                continue;
            }
            match apply(&op) {
                Ok(true) => {
                    println!("  ✓ Removed {}", describe(&op));
                    count += 1;
                }
                Ok(false) => {}
                Err(e) => println!("  ⚠ Skipped {}: {e}", describe(&op)),
            }
        }
        println!();
    }
    if dry_run {
        println!("{count} AutoMemory registration(s) found.");
    } else if count == 0 {
        println!("Nothing AutoMemory-owned found.");
    } else {
        println!("✓ {count} change(s). Local memory data (~/.automemory) left intact; delete that folder to remove it too.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "automemory-disconnect-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn mcp_strip_keeps_other_servers_and_prunes_empty_parent() {
        let dir = sandbox("mcp");
        let path = dir.join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers": {"automemory": {"command": "x"}, "other": {"command": "y"}}}"#,
        )
        .unwrap();
        assert!(strip_mcp_server(&path).unwrap());
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(val["mcpServers"].get("automemory").is_none());
        assert_eq!(val["mcpServers"]["other"]["command"], "y");

        fs::write(&path, r#"{"mcpServers": {"automemory": {"command": "x"}}}"#).unwrap();
        assert!(strip_mcp_server(&path).unwrap());
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(val.get("mcpServers").is_none());

        // Missing file and foreign-only file are no-ops, never errors.
        assert!(!strip_mcp_server(&dir.join("nope.json")).unwrap());
        fs::write(&path, r#"{"mcpServers": {"other": {}}}"#).unwrap();
        assert!(!strip_mcp_server(&path).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hooks_key_strip_deletes_file_only_when_empty() {
        let dir = sandbox("hooks");
        let path = dir.join("hooks.json");
        fs::write(&path, r#"{"automemory": {"enabled": true}, "user": {}}"#).unwrap();
        assert!(super::super::connect::strip_agy_hooks(&path).unwrap());
        assert!(path.exists());
        fs::write(&path, r#"{"automemory": {"enabled": true}}"#).unwrap();
        assert!(super::super::connect::strip_agy_hooks(&path).unwrap());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn goose_block_removed_only_on_exact_shape() {
        let dir = sandbox("goose");
        let path = dir.join("config.yaml");
        // Ours alone.
        fs::write(
            &path,
            "other: 1\n\nextensions:\n  - name: automemory\n    type: stdio\n    cmd: /bin/automemory\n    args:\n      - mcp\n",
        )
        .unwrap();
        assert!(strip_goose_block(&path).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("automemory"));
        assert!(text.contains("other: 1"));

        // Mixed with a user extension: header stays, ours goes.
        fs::write(
            &path,
            "extensions:\n  - name: mine\n    type: foo\n  - name: automemory\n    type: stdio\n    cmd: /bin/automemory\n    args:\n      - mcp\n",
        )
        .unwrap();
        assert!(strip_goose_block(&path).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("mine"));
        assert!(text.contains("extensions:"));
        assert!(text.contains("type: foo"));
        assert!(!text.contains("automemory"));

        // Wrong shape: untouched, caller reports manual.
        fs::write(&path, "extensions:\n  - name: automemory\n    type: http\n").unwrap();
        assert!(!strip_goose_block(&path).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn aider_block_removed_by_marker() {
        let dir = sandbox("aider");
        let path = dir.join(".aider.conf.yml");
        fs::write(
            &path,
            "model: gpt\n\n# AutoMemory MCP server\nmcp-servers:\n  automemory:\n    command: /bin/automemory\n    args: [mcp]\n",
        )
        .unwrap();
        assert!(strip_aider_block(&path).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text, "model: gpt\n");
        assert!(!strip_aider_block(&path).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn continue_and_zed_entries_stripped() {
        let dir = sandbox("json");
        let cpath = dir.join("config.json");
        fs::write(
            &cpath,
            r#"{"experimental": {"modelContextProtocolServers": [{"transport": {"command": "/bin/automemory"}}, {"transport": {"command": "other"}}]}}"#,
        )
        .unwrap();
        assert!(strip_continue_entry(&cpath).unwrap());
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&cpath).unwrap()).unwrap();
        assert_eq!(
            val["experimental"]["modelContextProtocolServers"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let zpath = dir.join("settings.json");
        fs::write(
            &zpath,
            r#"{"context_servers": {"automemory": {}, "other": {}}, "theme": "x"}"#,
        )
        .unwrap();
        assert!(strip_zed_entry(&zpath).unwrap());
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&zpath).unwrap()).unwrap();
        assert!(val["context_servers"].get("automemory").is_none());
        assert_eq!(val["context_servers"]["other"], serde_json::json!({}));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn targets_cover_all_registered_adapters() {
        let home = std::env::temp_dir().join("automemory-disconnect-home");
        let cwd = std::env::temp_dir().join("automemory-disconnect-cwd");
        for a in super::super::adapter::ADAPTERS {
            let ws = targets(a.adapter_id(), &home, &cwd, false).unwrap();
            let gl = targets(a.adapter_id(), &home, &cwd, true).unwrap();
            assert!(!ws.is_empty(), "workspace targets for {}", a.adapter_id());
            assert!(!gl.is_empty(), "global targets for {}", a.adapter_id());
        }
        assert!(targets("nope", &home, &cwd, false).is_err());
    }

    #[test]
    fn native_hooks_roundtrip_preserves_user_hooks() {
        use super::super::connect::{merge_native_hooks, CLAUDE_HOOKS};
        let dir = sandbox("native-hooks");
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{"model":"x","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"my-lint"}]}]}}"#,
        )
        .unwrap();
        merge_native_hooks(&path, "/opt/am bin/automemory", "claude", CLAUDE_HOOKS).unwrap();
        merge_native_hooks(&path, "/opt/am bin/automemory", "claude", CLAUDE_HOOKS).unwrap(); // idempotent
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let pre = val["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "user hook kept, ours added once");
        assert!(pre[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .starts_with("\"/opt/am bin/automemory\" hook --agent claude --event pre-tool"));
        assert_eq!(val["model"], "x");

        assert!(super::super::connect::strip_native_hooks(&path).unwrap());
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(val["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert!(val["hooks"].get("Stop").is_none());
        assert_eq!(val["model"], "x");
    }

    #[test]
    fn codex_toml_append_and_strip_keep_user_content() {
        use super::super::connect::{merge_codex_toml, strip_codex_toml};
        let dir = sandbox("codex-toml");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "model = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n",
        )
        .unwrap();
        merge_codex_toml(&path, "/bin/automemory").unwrap();
        merge_codex_toml(&path, "/bin/automemory").unwrap(); // idempotent
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("[mcp_servers.automemory]").count(), 1);
        let parsed: toml::Table = text.parse().expect("valid TOML");
        assert_eq!(
            parsed["mcp_servers"]["automemory"]["args"][0].as_str(),
            Some("mcp")
        );
        assert!(strip_codex_toml(&path).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("automemory"));
        assert!(text.contains("[mcp_servers.other]") && text.contains("model = \"o3\""));
    }

    #[test]
    fn connect_refuses_to_overwrite_unparseable_config() {
        use super::super::connect::{merge_native_hooks, read_json_or_empty, CLAUDE_HOOKS};
        let dir = sandbox("broken-json");
        let path = dir.join("settings.json");
        let broken = r#"{"important": true,"#; // e.g. mid-write by the agent
        fs::write(&path, broken).unwrap();
        let err = merge_native_hooks(&path, "/bin/automemory", "claude", CLAUDE_HOOKS).unwrap_err();
        assert!(err.to_string().contains("not valid JSON"), "{err}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            broken,
            "file must be untouched"
        );

        // Missing and blank files are fine: they start as empty objects.
        assert_eq!(
            read_json_or_empty(&dir.join("nope.json")).unwrap(),
            serde_json::json!({})
        );
        fs::write(&path, "  \n").unwrap();
        assert_eq!(read_json_or_empty(&path).unwrap(), serde_json::json!({}));
    }

    fn write(p: &Path, text: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn git_repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(dir).status().unwrap().success());
    }

    #[test]
    fn antigravity_connect_is_single_registration_and_keeps_user_hooks() {
        use super::super::connect::connect_agy;
        let dir = sandbox("agy-ws");
        let (home, ws) = (dir.join("home"), dir.join("ws"));
        fs::create_dir_all(&home).unwrap();
        git_repo(&ws);
        write(&ws.join(".agents/hooks.json"), r#"{"mine": {"enabled": true}}"#);
        write(&ws.join(".agents/plugins/automemory/plugin.json"), r#"{"name":"automemory"}"#);
        write(&ws.join(".agents/plugins/automemory/hooks.json"), "{}");
        connect_agy(&home, &ws, "/bin/automemory", false).unwrap();
        connect_agy(&home, &ws, "/bin/automemory", false).unwrap(); // idempotent

        let hooks: serde_json::Value = serde_json::from_str(&fs::read_to_string(ws.join(".agents/hooks.json")).unwrap()).unwrap();
        assert!(hooks["mine"]["enabled"] == true, "user's hook kept");
        for ev in ["PreInvocation", "PreToolUse", "PostToolUse", "Stop"] {
            assert!(hooks["automemory"][ev].is_array(), "{ev}");
        }
        assert!(!ws.join(".agents/plugins/automemory").exists(), "legacy duplicate plugin removed");
        assert!(!ws.join(".mcp.json").exists(), "Antigravity does not write Claude's file");
        assert!(ws.join(".agents/mcp_config.json").exists() && ws.join(".agents/skills/automemory/SKILL.md").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn antigravity_global_connect_covers_cli_app_and_ide_and_prevents_duplicates() {
        use super::super::connect::connect_agy;
        let dir = sandbox("agy-global");
        let (home, ws) = (dir.join("home"), dir.join("ws"));
        fs::create_dir_all(home.join(".gemini/antigravity-cli")).unwrap(); // the CLI is installed
        git_repo(&ws);
        connect_agy(&home, &ws, "/bin/automemory", true).unwrap();
        assert!(home.join(".gemini/config/hooks.json").exists());
        assert!(home.join(".gemini/config/mcp_config.json").exists());
        assert!(home.join(".gemini/config/skills/automemory/SKILL.md").exists(), "App and IDE skill path");
        assert!(home.join(".gemini/antigravity-cli/skills/automemory/SKILL.md").exists(), "CLI skill path");

        // A workspace connect now must not add a second copy of the hooks.
        connect_agy(&home, &ws, "/bin/automemory", false).unwrap();
        assert!(!ws.join(".agents/hooks.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn disconnect_all_leaves_no_registration_and_no_user_content_lost() {
        use super::super::connect::{connect_agy, connect_claude, connect_codex};
        let dir = sandbox("all");
        let (home, ws) = (dir.join("home"), dir.join("ws"));
        git_repo(&ws);
        write(&ws.join("AGENTS.md"), "# my own agent notes\n");
        write(&ws.join(".git/hooks/post-commit"), "#!/bin/sh\necho user hook\n");
        write(&home.join(".claude/settings.json"), r#"{"model":"x","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"my-lint"}]}]}}"#);
        write(&home.join(".gemini/config/hooks.json"), r#"{"mine": {"enabled": true}}"#);
        for global in [false, true] {
            connect_agy(&home, &ws, "/bin/automemory", global).unwrap();
            connect_claude(&home, &ws, "/bin/automemory", global).unwrap();
            connect_codex(&home, &ws, "/bin/automemory", global).unwrap();
        }
        assert!(inventory(&home, &ws).len() >= 8, "{:?}", inventory(&home, &ws));
        let hook = fs::read_to_string(ws.join(".git/hooks/post-commit")).unwrap();
        assert!(hook.contains("echo user hook") && hook.contains("automemory post-commit"), "user hook kept, ours added");

        run(&home, &ws, "all", false, true, false).unwrap();

        assert_eq!(inventory(&home, &ws), Vec::<String>::new(), "nothing of ours is left");
        assert_eq!(fs::read_to_string(ws.join("AGENTS.md")).unwrap(), "# my own agent notes\n");
        assert_eq!(fs::read_to_string(ws.join(".git/hooks/post-commit")).unwrap(), "#!/bin/sh\necho user hook\n");
        let claude: serde_json::Value = serde_json::from_str(&fs::read_to_string(home.join(".claude/settings.json")).unwrap()).unwrap();
        assert_eq!(claude["model"], "x");
        assert_eq!(claude["hooks"]["PreToolUse"].as_array().unwrap().len(), 1, "user's Claude hook kept");
        let agy: serde_json::Value = serde_json::from_str(&fs::read_to_string(home.join(".gemini/config/hooks.json")).unwrap()).unwrap();
        assert!(agy.get("automemory").is_none() && agy["mine"]["enabled"] == true);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dry_run_lists_without_changing_anything() {
        use super::super::connect::connect_agy;
        let dir = sandbox("dry");
        let (home, ws) = (dir.join("home"), dir.join("ws"));
        git_repo(&ws);
        connect_agy(&home, &ws, "/bin/automemory", false).unwrap();
        let before = inventory(&home, &ws);
        run(&home, &ws, "all", false, true, true).unwrap();
        assert_eq!(inventory(&home, &ws), before);
        let _ = fs::remove_dir_all(&dir);
    }
}
