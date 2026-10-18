//! Guided learning, onboarding next step, interactive tour, and demo data.

use anyhow::{bail, Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::onboarding;
use automemory_core::setup::{self, LEARN_TOPICS};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Prints the single best next onboarding step and command.
pub fn next() -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .context("Failed to open database. Try: run `automemory init` first.")?;
    let steps = setup::checklist(&db, &home, &cwd);
    if let Some(step) = setup::next_step(&steps) {
        let reason = step.why.trim_end_matches('.');
        println!("{}. Try: `{}`", reason, step.command);
    } else {
        println!("All onboarding checklist steps are complete!");
    }
    Ok(())
}

/// Lists available topics or explains a specific topic with copy-paste commands.
pub fn learn(topic: Option<&str>) -> Result<()> {
    match topic {
        Some(t) => {
            let key = t.trim().to_lowercase();
            if let Some(found) = LEARN_TOPICS.iter().find(|item| item.id == key || item.title.to_lowercase() == key) {
                println!("\n=== {} ===\n{}\n", found.title, found.content);
                Ok(())
            } else {
                println!("Unknown topic: '{t}'.\n\nAvailable topics:");
                for item in LEARN_TOPICS {
                    println!("  {:<15} - {}", item.id, item.summary);
                }
                println!("\nTry: `automemory learn memory`");
                Ok(())
            }
        }
        None => {
            println!("=== AutoMemory Guided Topics ===\n");
            for item in LEARN_TOPICS {
                println!("  {:<15} - {}", item.id, item.summary);
            }
            println!("\nLearn more with: `automemory learn <topic>`");
            Ok(())
        }
    }
}

/// Seeds or removes a sample project (3 decisions, 5 notes, 1 pending suggestion).
pub fn demo(remove: bool) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .context("Failed to open database. Try: run `automemory init` first.")?;

    if remove {
        onboarding::remove_demo(&db)?;
        println!("✓ Demo project and all sample data removed. Zero demo rows remain.");
    } else {
        onboarding::seed_demo(&db)?;
        println!("✓ Demo project seeded with 3 decisions, 5 notes, and 1 pending suggestion.");
        println!("  Sample items carry metadata.demo = true and are excluded from sync, export-rules, and recall.");
        println!("  Run `automemory demo --remove` at any time to remove all sample data.");
    }
    Ok(())
}

/// Runs the interactive or automated 8-step guided tour in an isolated sandbox.
pub fn tour(
    auto: bool,
    resume: bool,
    reset: bool,
    step_filter: Option<usize>,
    no_color: bool,
) -> Result<()> {
    let is_auto = auto || !std::io::stdin().is_terminal();

    // Isolated sandbox directories in temp
    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let sandbox_root = std::env::temp_dir().join(format!("am-tour-sandbox-{user}"));
    let home = sandbox_root.join("home");
    let project = sandbox_root.join("project");

    if reset {
        if sandbox_root.exists() {
            let _ = std::fs::remove_dir_all(&sandbox_root);
        }
        println!("✓ Tour sandbox and progress reset.");
        if step_filter.is_none() && !resume {
            return Ok(());
        }
    }

    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&project)?;

    if !project.join(".git").exists() {
        let _ = Command::new("git").args(["init", "-q", "-b", "main"]).current_dir(&project).output();
        let _ = std::fs::write(project.join("README.md"), "# Tour Sandbox Project\n");
        let _ = Command::new("git").args(["add", "-A"]).current_dir(&project).output();
        let _ = Command::new("git")
            .args(["-c", "user.name=tour", "-c", "user.email=tour@example.com", "commit", "-qm", "Initial commit"])
            .current_dir(&project)
            .output();
    }

    let state_file = sandbox_root.join("tour_state.json");
    let current_step = if let Some(s) = step_filter {
        s
    } else if resume {
        read_saved_step(&state_file) + 1
    } else {
        1
    };

    let exe = std::env::current_exe().context("Failed to get current executable path")?;

    println!("============================================================");
    println!("             AutoMemory — Guided Tour (Sandbox)             ");
    println!("============================================================");
    println!("Learn AutoMemory in an isolated throwaway environment.");
    println!("Nothing in your real ~/.automemory or repository will change.\n");

    let total_steps = 8;
    let start = current_step.clamp(1, total_steps);
    let end = if step_filter.is_some() { start } else { total_steps };

    for s in start..=end {
        run_tour_step(s, &exe, &home, &project, is_auto, no_color)?;
        save_step(&state_file, s);
    }

    if end == total_steps {
        println!("\n============================================================");
        println!("✓ You have completed the AutoMemory Guided Tour!");
        println!("Run `automemory next` anytime in your workspace to get started.");
        println!("============================================================\n");
        let _ = std::fs::remove_dir_all(&sandbox_root);
    }

    Ok(())
}

fn run_tour_step(
    step: usize,
    exe: &Path,
    home: &Path,
    project: &Path,
    is_auto: bool,
    _no_color: bool,
) -> Result<()> {
    match step {
        1 => {
            prompt_or_auto(
                is_auto,
                "automemory doctor",
                "Step 1/8 · Welcome & Diagnostics\nInspect system health, database status, and getting started checklist.",
            );
            let (ok, stdout, _) = run_cmd(exe, home, project, &["doctor"], None);
            println!("{stdout}");
            if !ok {
                bail!("Step 1 failed: doctor exited with non-zero code. Try: run `automemory tour --reset`");
            }
            println!("✓ done: System diagnostics verified.\n");
        }
        2 => {
            prompt_or_auto(
                is_auto,
                "automemory note \"We use pnpm\"",
                "Step 2/8 · Remember Something\nTeach AutoMemory a project preference. All connected agents will share it.",
            );
            let (ok, stdout, _) = run_cmd(exe, home, project, &["note", "We use pnpm"], None);
            println!("{stdout}");
            let (_, list_out, _) = run_cmd(exe, home, project, &["memory", "list"], None);
            println!("Listing memories:\n{list_out}");
            if !ok || !list_out.contains("pnpm") {
                bail!("Step 2 failed: note not found in memory list. Try: `automemory note \"We use pnpm\"`");
            }
            println!("✓ done: Note saved and verified in memory.\n");
        }
        3 => {
            prompt_or_auto(
                is_auto,
                "automemory decision record --topic Storage --decision \"SQLite WAL\" --reason \"Fast local ACID\"",
                "Step 3/8 · Record an Architectural Decision\nDecisions outrank notes and are presented to agents first.",
            );
            let (ok, stdout, _) = run_cmd(
                exe,
                home,
                project,
                &[
                    "decision",
                    "record",
                    "--topic",
                    "Storage",
                    "--decision",
                    "SQLite WAL",
                    "--reason",
                    "Fast local ACID",
                ],
                None,
            );
            println!("{stdout}");
            if !ok {
                bail!("Step 3 failed: decision record failed. Try: `automemory decision list`");
            }
            println!("✓ done: Architectural decision stored in the ledger.\n");
        }
        4 => {
            prompt_or_auto(
                is_auto,
                "automemory export-rules",
                "Step 4/8 · Instruction Files (What Agents See)\nExport verified decisions and notes directly into AGENTS.md.",
            );
            let (ok, stdout, _) = run_cmd(exe, home, project, &["export-rules"], None);
            println!("{stdout}");
            let agents_md = project.join("AGENTS.md");
            if agents_md.exists() {
                let content = std::fs::read_to_string(&agents_md)?;
                println!("--- Generated AGENTS.md Block ---");
                println!("{content}");
                println!("---------------------------------");
            }
            if !ok || !agents_md.exists() {
                bail!("Step 4 failed: AGENTS.md was not written. Try: `automemory export-rules`");
            }
            println!("✓ done: AGENTS.md updated. Agents start informed with 0ms tool latency.\n");
        }
        5 => {
            prompt_or_auto(
                is_auto,
                "(simulated) Agent A edits src/main.rs -> Agent B tries src/main.rs",
                "Step 5/8 · Two Agents, One File (Collision Protection)\nWhen multiple agents work on the same codebase, AutoMemory prevents collisions.",
            );
            // 1. Connect claude in sandbox
            let _ = run_cmd(exe, home, project, &["connect", "claude"], None);
            let _ = run_cmd(exe, home, project, &["mode", "set", "strict"], None);

            // 2. Agent A leases src/main.rs
            let a_payload = serde_json::json!({
                "session_id": "agent-A",
                "tool_name": "Edit",
                "tool_input": { "file_path": "src/main.rs" }
            });
            let _ = run_cmd(
                exe,
                home,
                project,
                &["hook", "--agent", "claude", "--event", "pre-tool", "--project", "auto"],
                Some(&a_payload.to_string()),
            );
            println!("Agent A acquired lease on src/main.rs");

            // 3. Agent B attempts to edit src/main.rs
            let b_payload = serde_json::json!({
                "session_id": "agent-B",
                "tool_name": "Edit",
                "tool_input": { "file_path": "src/main.rs" }
            });
            let (_, b_out, _) = run_cmd(
                exe,
                home,
                project,
                &["hook", "--agent", "claude", "--event", "pre-tool", "--project", "auto"],
                Some(&b_payload.to_string()),
            );

            println!("Agent B edit attempt output:\n{b_out}");
            if !b_out.contains("deny") && !b_out.contains("Collision") && !b_out.contains("held") {
                bail!("Step 5 failed: Collision was not detected. Try: `automemory locks`");
            }
            println!("✓ done: Collision prevented! Agent B was stopped before overwriting Agent A's changes.\n");
        }
        6 => {
            prompt_or_auto(
                is_auto,
                "automemory handoff \"Completed storage migration\"",
                "Step 6/8 · Session Continuity & Hand-Off\nPass context packets between sessions so work resumes without loss.",
            );
            let (ok, stdout, _) = run_cmd(
                exe,
                home,
                project,
                &["handoff", "Completed storage migration"],
                None,
            );
            println!("{stdout}");
            let (_, resume_out, _) = run_cmd(exe, home, project, &["resume"], None);
            println!("Resume packet for next session:\n{resume_out}");
            if !ok || !resume_out.contains("Completed storage migration") {
                bail!("Step 6 failed: handoff baton was not recorded. Try: `automemory resume`");
            }
            println!("✓ done: Handoff baton generated and verified via automemory resume.\n");
        }
        7 => {
            prompt_or_auto(
                is_auto,
                "automemory mode set review",
                "Step 7/8 · Review Proposals & Safety Modes\nIn Review mode, high-risk actions generate proposals for human approval.",
            );
            let (ok, stdout, _) = run_cmd(exe, home, project, &["mode", "set", "review"], None);
            println!("{stdout}");
            let _ = run_cmd(exe, home, project, &["demo"], None);
            let (_, prop_list, _) = run_cmd(exe, home, project, &["proposal", "list"], None);
            println!("Pending suggestions:\n{prop_list}");
            let (app_ok, app_out, _) = run_cmd(exe, home, project, &["proposal", "approve-all"], None);
            println!("Approving suggestions:\n{app_out}");
            if !ok || !app_ok {
                bail!("Step 7 failed: review proposals could not be approved. Try: `automemory proposal list`");
            }
            println!("✓ done: Suggestions reviewed and approved.\n");
        }
        8 => {
            println!("Step 8/8 · Your Turn!\nThe tour sandbox is complete. Here is how to use AutoMemory in your real workspace:\n");
            println!("1. Connect your AI coding assistant (opt-in):");
            println!("   automemory connect claude     # Claude Code CLI");
            println!("   automemory connect agy        # Google Antigravity");
            println!("   automemory connect cursor     # Cursor IDE");
            println!("   automemory connect codex      # OpenAI Codex\n");
            println!("What connect writes:");
            println!("   - Claude Code: ~/.claude/settings.json or .claude/settings.local.json");
            println!("   - Antigravity: ~/.gemini/config/hooks.json or .agents/hooks.json");
            println!("   - Cursor:      .cursor/rules/automemory.mdc");
            println!("   - All:         AGENTS.md (managed <!-- automemory:begin --> block)\n");
            println!("Completely reversible at any time:");
            println!("   automemory disconnect <agent>");
            println!("   automemory disconnect all\n");
            println!("2. Multi-device sync (optional):");
            println!("   automemory sync remote <git-vault-url>");
            println!("   automemory sync auto\n");
            println!("3. Shell completions:");
            println!("   automemory completions zsh >> ~/.zshrc");
            println!("   automemory completions bash >> ~/.bashrc\n");

            if !is_auto {
                print!("Would you like to connect an agent now? [claude/agy/cursor/skip]: ");
                let _ = std::io::stdout().flush();
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                let choice = line.trim();
                if !choice.is_empty() && choice != "skip" {
                    println!("Running `automemory connect {}` in current workspace...", choice);
                    let _ = super::connect::execute(choice, false);
                }
            }
            println!("✓ done: You're ready to use AutoMemory! Run `automemory next` anytime.\n");
        }
        _ => {}
    }
    Ok(())
}

fn prompt_or_auto(is_auto: bool, cmd_hint: &str, explain: &str) {
    println!("{explain}");
    if is_auto {
        println!("$ {cmd_hint}\n");
    } else {
        print!("Press Enter to run, or type command [$ {cmd_hint}]: ");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        let trimmed = line.trim();
        if !trimmed.is_empty() && !is_near_match(trimmed, cmd_hint) {
            println!("(Tip: running `{cmd_hint}` for you)");
        }
        println!();
    }
}

fn is_near_match(input: &str, expected: &str) -> bool {
    let lower_in = input.to_lowercase();
    let lower_exp = expected.to_lowercase();
    lower_in == lower_exp || lower_exp.contains(&lower_in) || lower_in.contains("automemory")
}

fn run_cmd(
    exe: &Path,
    home: &Path,
    project: &Path,
    args: &[&str],
    input: Option<&str>,
) -> (bool, String, String) {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .env("HOME", home)
        .env("AUTOMEMORY_PASSWORD", "")
        .current_dir(project)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if input.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (false, String::new(), format!("Failed to spawn command: {e}")),
    };

    if let Some(text) = input {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
    }

    match child.wait_with_output() {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        ),
        Err(e) => (false, String::new(), format!("Failed waiting on output: {e}")),
    }
}

fn read_saved_step(path: &Path) -> usize {
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            return val.get("tour_step").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        }
    }
    0
}

fn save_step(path: &Path, step: usize) {
    let val = serde_json::json!({ "tour_step": step });
    let _ = std::fs::write(path, val.to_string());
}
