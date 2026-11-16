//! Who is running this command? A person at a terminal, or one of the coding agents through its shell tool. Nobody types a name: the name comes from
//! the evidence, strongest first.
//!   1. a person is proven (a terminal on stdin, or the human password in the environment): `human`;
//!   2. a parent process is a known agent (walked up from this process): that agent, the same whether or not its hooks are connected;
//!   3. an environment variable only an agent's shell sets (a hint, since it can be missing or faked);
//!   4. nothing matches: `shell`, which is kept but never counts as a person.
//!
//! An agent can pose as another agent this way, never as a person: "human" needs the terminal or the password.

use std::collections::HashMap;

/// Where the name came from, for `doctor` and the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Person,
    Process,
    Env,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub name: String,
    pub via: Via,
}

impl Actor {
    pub fn is_human(&self) -> bool {
        self.via == Via::Person
    }
}

/// (name, executable names, command-line fragments). One line adds an agent.
const AGENTS: &[(&str, &[&str], &[&str])] = &[
    ("claude", &["claude"], &["/claude-code/", "claude.app/contents/macos/claude"]),
    ("codex", &["codex"], &["/codex/bin/codex", "codex-cli"]),
    ("agy", &["agy"], &["antigravity", "/agy "]),
    ("cursor", &["cursor-agent"], &["cursor-agent"]),
    ("gemini", &["gemini"], &["gemini-cli", "/gemini "]),
];

/// Variables an agent's shell tool sets and a person's terminal does not. Only a hint.
const ENV_MARKERS: &[(&str, &str)] = &[
    ("claude", "CLAUDECODE"),
    ("claude", "CLAUDE_CODE_SESSION_ID"),
    ("codex", "CODEX_SANDBOX"),
    ("codex", "CODEX_THREAD_ID"),
    ("codex", "CODEX_SANDBOX_NETWORK_DISABLED"),
    ("gemini", "GEMINI_CLI"),
];

/// One process of the chain: its executable name (lowercase, no path) and its whole command line (lowercase).
#[derive(Debug, Clone)]
pub struct Proc {
    pub exe: String,
    pub command: String,
}

/// The agent a process chain belongs to, nearest parent first.
pub fn agent_in_chain(chain: &[Proc]) -> Option<&'static str> {
    chain.iter().find_map(|p| {
        AGENTS.iter().find(|(_, exes, frags)| exes.contains(&p.exe.as_str()) || frags.iter().any(|f| p.command.contains(f))).map(|(n, _, _)| *n)
    })
}

/// The agent an environment points to.
pub fn agent_in_env(get: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    ENV_MARKERS.iter().find(|(_, var)| get(var).is_some_and(|v| !v.is_empty())).map(|(n, _)| *n)
}

/// This process's ancestors, nearest first (Unix: `ps`; elsewhere empty).
#[cfg(unix)]
pub fn process_chain() -> Vec<Proc> {
    let run = |fmt: &str| -> String {
        std::process::Command::new("ps").args(["-ax", "-o", fmt]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
    };
    let mut parent: HashMap<u32, u32> = HashMap::new();
    let mut exe: HashMap<u32, String> = HashMap::new();
    for line in run("pid=,ppid=,comm=").lines() {
        let mut it = line.split_whitespace();
        if let (Some(p), Some(pp)) = (it.next().and_then(|v| v.parse::<u32>().ok()), it.next().and_then(|v| v.parse::<u32>().ok())) {
            parent.insert(p, pp);
            let comm = it.collect::<Vec<_>>().join(" ");
            exe.insert(p, comm.rsplit('/').next().unwrap_or("").to_lowercase());
        }
    }
    let mut command: HashMap<u32, String> = HashMap::new();
    for line in run("pid=,command=").lines() {
        let line = line.trim_start();
        if let Some((p, rest)) = line.split_once(' ') {
            if let Ok(p) = p.parse::<u32>() {
                command.insert(p, rest.trim().to_lowercase());
            }
        }
    }
    let mut chain = Vec::new();
    let mut pid = std::process::id();
    for _ in 0..20 {
        let Some(&pp) = parent.get(&pid) else { break };
        if pp <= 1 {
            break;
        }
        chain.push(Proc { exe: exe.get(&pp).cloned().unwrap_or_default(), command: command.get(&pp).cloned().unwrap_or_default() });
        pid = pp;
    }
    chain
}

#[cfg(not(unix))]
pub fn process_chain() -> Vec<Proc> {
    Vec::new()
}

/// The decision, with the evidence passed in so it can be tested.
pub fn decide(proven_human: bool, chain: &[Proc], env: impl Fn(&str) -> Option<String>) -> Actor {
    if proven_human {
        return Actor { name: "human".into(), via: Via::Person };
    }
    if let Some(n) = agent_in_chain(chain) {
        return Actor { name: n.into(), via: Via::Process };
    }
    if let Some(n) = agent_in_env(env) {
        return Actor { name: n.into(), via: Via::Env };
    }
    Actor { name: "shell".into(), via: Via::Unknown }
}

/// Password in the environment: a person, whatever the process chain says. A terminal on stdin: a person, unless an agent is in the chain (an
/// agent's shell tool may run commands in a pseudo-terminal, and a person typing in an agent's own terminal pane then has to use the password).
pub fn classify(password_proof: bool, tty: bool, chain: &[Proc], env: impl Fn(&str) -> Option<String>) -> Actor {
    decide(password_proof || (tty && agent_in_chain(chain).is_none()), chain, env)
}

/// Who is running this command right now.
pub fn current(stored_password_hash: Option<&str>) -> Actor {
    use std::io::IsTerminal;
    let password = crate::auth::password_proves_human(stored_password_hash);
    let tty = std::io::stdin().is_terminal();
    // the process chain costs two `ps` calls: only worth it when the password has not already settled it
    let chain = if password { Vec::new() } else { process_chain() };
    classify(password, tty, &chain, |k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(exe: &str, command: &str) -> Proc {
        Proc { exe: exe.into(), command: command.to_lowercase() }
    }
    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_person_is_a_person_even_inside_an_agents_terminal() {
        let chain = [p("zsh", "-zsh"), p("codex", "/usr/local/bin/codex")];
        assert_eq!(decide(true, &chain, no_env), Actor { name: "human".into(), via: Via::Person });
    }

    #[test]
    fn a_terminal_alone_does_not_make_a_person_when_an_agent_is_in_the_chain() {
        // live run 2026-11-16: Antigravity runs `automemory note` in a pseudo-terminal, and the note was filed as written by the person
        let agy = [p("zsh", "zsh"), p("language_server", "/Applications/Antigravity.app/Contents/Resources/language_server")];
        assert_eq!(classify(false, true, &agy, no_env), Actor { name: "agy".into(), via: Via::Process });
        // a person at an ordinary terminal
        assert_eq!(classify(false, true, &[p("zsh", "-zsh"), p("terminal", "terminal")], no_env).name, "human");
        // the password still proves a person inside an agent's terminal
        assert_eq!(classify(true, true, &agy, no_env).name, "human");
        // no terminal, no password: never a person
        assert_ne!(classify(false, false, &[p("zsh", "zsh")], no_env).name, "human");
    }

    #[test]
    fn the_nearest_known_agent_in_the_chain_names_the_actor() {
        let chain = [p("sh", "/bin/sh -c automemory decision record"), p("zsh", "zsh"), p("codex", "/Users/as/.local/bin/codex exec"), p("claude", "claude")];
        assert_eq!(decide(false, &chain, no_env), Actor { name: "codex".into(), via: Via::Process });
        let desktop = [p("bash", "bash"), p("claude", "/Users/as/Library/Application Support/Claude/claude-code/2.1.321/x/claude.app/Contents/MacOS/claude --output-format")];
        assert_eq!(agent_in_chain(&desktop), Some("claude"));
        let agy = [p("zsh", "zsh"), p("language_server", "/Applications/Antigravity.app/Contents/Resources/language_server")];
        assert_eq!(agent_in_chain(&agy), Some("agy"));
    }

    #[test]
    fn the_environment_is_the_fallback_and_nothing_means_shell() {
        let env = |k: &str| (k == "CODEX_SANDBOX_NETWORK_DISABLED").then(|| "1".to_string());
        assert_eq!(decide(false, &[p("zsh", "zsh")], env), Actor { name: "codex".into(), via: Via::Env });
        assert_eq!(decide(false, &[p("zsh", "zsh"), p("sshd", "sshd")], no_env), Actor { name: "shell".into(), via: Via::Unknown });
        assert!(!decide(false, &[], no_env).is_human());
    }

    #[cfg(unix)]
    #[test]
    fn the_real_chain_of_this_test_process_is_readable() {
        assert!(!process_chain().is_empty(), "ps answered");
    }
}
