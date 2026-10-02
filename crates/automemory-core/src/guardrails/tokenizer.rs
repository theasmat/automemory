use serde::{Deserialize, Serialize};

/// Represents a single parsed executable command invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedCommand {
    pub binary: String,
    pub flags: Vec<String>,
    pub positional_args: Vec<String>,
    pub raw_tokens: Vec<String>,
}

impl ParsedCommand {
    /// Checks if the command matches a specific binary name (e.g. "rm", "git", "kubectl", "prisma").
    pub fn is_binary(&self, name: &str) -> bool {
        let b = self.binary.to_lowercase();
        b == name || b.ends_with(&format!("/{}", name))
    }

    /// Checks if the command has a specific flag (e.g. "-rf", "--force", "-f").
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag || f.contains(flag))
    }

    /// Checks if any argument matches or contains a target pattern.
    pub fn has_arg_matching<F>(&self, predicate: F) -> bool
    where
        F: Fn(&str) -> bool,
    {
        self.positional_args.iter().any(|arg| predicate(arg))
            || self.flags.iter().any(|flag| predicate(flag))
    }
}

/// A tree of parsed shell commands extracted from pipelines and subshells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellCommandTree {
    pub commands: Vec<ParsedCommand>,
    pub raw_input: String,
}

impl ShellCommandTree {
    /// Tokenizes a raw shell command string, resolves quotes/pipes/subshells (`sh -c`, `sudo`, `npx`, `eval`),
    /// and produces a normalized list of individual parsed commands.
    pub fn parse(raw: &str) -> Self {
        let mut commands = Vec::new();
        let segments = split_command_chain(raw);

        for seg in segments {
            let tokens = tokenize_shell_words(&seg);
            if tokens.is_empty() {
                continue;
            }

            let unwrapped_commands = unwrap_command_tokens(&tokens);
            commands.extend(unwrapped_commands);
        }

        Self {
            commands,
            raw_input: raw.to_string(),
        }
    }

    /// Returns true if any parsed command is a destructive `rm -rf /` or `rm -rf ~`.
    pub fn is_destructive_rm_root_or_home(&self) -> bool {
        for cmd in &self.commands {
            if cmd.is_binary("rm") {
                let has_recursive = cmd.flags.iter().any(|f| {
                    (f.starts_with('-') && f.contains('r') && f.contains('f'))
                        || f == "-r"
                        || f == "-R"
                        || f == "--recursive"
                        || f == "-rf"
                        || f == "-fr"
                });

                if has_recursive {
                    for arg in &cmd.positional_args {
                        let a = arg.trim();
                        if a == "/"
                            || a == "/*"
                            || a == "~"
                            || a == "~/*"
                            || a == "$HOME"
                            || a == "$HOME/*"
                            || a == "${HOME}"
                            || a == "${HOME}/*"
                        {
                            return true;
                        }
                    }
                }
            }

            // Inline language scripts: python -c "...", node -e "...", etc.
            if cmd.is_binary("python") || cmd.is_binary("python3") || cmd.is_binary("node") || cmd.is_binary("ruby") || cmd.is_binary("perl") {
                let is_eval = cmd.has_flag("-c") || cmd.has_flag("-e");
                if is_eval {
                    for arg in &cmd.positional_args {
                        let lower = arg.to_lowercase();
                        if (lower.contains("rmtree") || lower.contains("rmdir") || lower.contains("rmsync") || lower.contains("unlink"))
                            && (lower.contains("'/") || lower.contains("\"/") || lower.contains("'~") || lower.contains("\"~") || lower.contains("os.environ['home']"))
                        {
                            return true;
                        }
                        if lower.contains("system(") || lower.contains("popen(") || lower.contains("exec(") || lower.contains("spawn(") {
                            if lower.contains("rm -rf /") || lower.contains("rm -rf ~") || lower.contains("rm -rf $home") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Returns true if any command is a destructive git force push to main, master, prod, or release.
    pub fn is_git_force_push_prod(&self) -> bool {
        for cmd in &self.commands {
            if cmd.is_binary("git") && cmd.positional_args.first().map(|s| s.as_str()) == Some("push") {
                let has_force = cmd.flags.iter().any(|f| f == "--force" || f == "-f" || f == "--force-with-lease");
                let targets_prod = cmd.positional_args.iter().skip(1).any(|arg| {
                    let a = arg.to_lowercase();
                    a == "main"
                        || a == "master"
                        || a == "prod"
                        || a == "production"
                        || a == "release"
                        || a.ends_with(":main")
                        || a.ends_with(":master")
                });

                if has_force && targets_prod {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if any command is a raw disk format / block overwrite.
    pub fn is_raw_disk_format(&self) -> bool {
        for cmd in &self.commands {
            if cmd.binary.starts_with("mkfs") {
                return true;
            }
            if cmd.is_binary("dd") {
                if cmd.flags.iter().any(|f| f.starts_with("of=/dev/"))
                    || cmd.positional_args.iter().any(|a| a.starts_with("of=/dev/"))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if any command is an unescaped raw database drop or truncate.
    pub fn is_raw_db_drop(&self) -> bool {
        let upper = self.raw_input.to_uppercase();
        upper.contains("DROP DATABASE") || upper.contains("TRUNCATE TABLE") || upper.contains("DROP SCHEMA")
    }

    /// Returns true if plain text secrets (API keys, PATs) are detected directly in the command.
    pub fn contains_plaintext_secret(&self) -> bool {
        let patterns = [
            "sk-",
            "ghp_",
            "xoxb-",
            "AWS_SECRET_ACCESS_KEY=",
            "AKIA",
            "AIzaSy",
        ];
        for cmd in &self.commands {
            for tok in &cmd.raw_tokens {
                for pat in &patterns {
                    if tok.contains(pat) && tok.len() >= pat.len() + 16 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Returns true if database migrations are executing.
    pub fn is_db_migration(&self) -> bool {
        for cmd in &self.commands {
            if (cmd.is_binary("alembic") || cmd.has_arg_matching(|a| a == "alembic"))
                && cmd.has_arg_matching(|a| a == "upgrade")
            {
                return true;
            }
            if (cmd.is_binary("prisma") || cmd.has_arg_matching(|a| a == "prisma"))
                && cmd.has_arg_matching(|a| a == "migrate")
            {
                return true;
            }
            if (cmd.is_binary("diesel") || cmd.has_arg_matching(|a| a == "diesel"))
                && cmd.has_arg_matching(|a| a == "migration")
            {
                return true;
            }
            if (cmd.is_binary("flyway") || cmd.has_arg_matching(|a| a == "flyway"))
                && cmd.has_arg_matching(|a| a == "migrate")
            {
                return true;
            }
            if (cmd.is_binary("rake") || cmd.has_arg_matching(|a| a == "rake"))
                && cmd.has_arg_matching(|a| a == "db:migrate")
            {
                return true;
            }
        }
        false
    }

    /// Returns true if cloud infrastructure teardown is executing.
    pub fn is_cloud_infra_delete(&self) -> bool {
        for cmd in &self.commands {
            if cmd.is_binary("kubectl") && cmd.positional_args.first().map(|s| s.as_str()) == Some("delete") {
                return true;
            }
            if cmd.is_binary("helm") && cmd.positional_args.first().map(|s| s.as_str()) == Some("uninstall") {
                return true;
            }
            if cmd.is_binary("terraform") && cmd.positional_args.first().map(|s| s.as_str()) == Some("destroy") {
                return true;
            }
            if cmd.is_binary("docker")
                && cmd.positional_args.first().map(|s| s.as_str()) == Some("rm")
                && cmd.has_flag("-f")
            {
                return true;
            }
        }
        false
    }

    /// Returns true if destructive git reset --hard or git clean is executing.
    pub fn is_destructive_git_reset(&self) -> bool {
        for cmd in &self.commands {
            if cmd.is_binary("git") {
                if cmd.positional_args.first().map(|s| s.as_str()) == Some("reset") && cmd.has_flag("--hard") {
                    return true;
                }
                if cmd.positional_args.first().map(|s| s.as_str()) == Some("clean")
                    && (cmd.has_flag("-f") || cmd.has_flag("-fd") || cmd.has_flag("-df"))
                {
                    return true;
                }
            }
        }
        false
    }
}

/// Splits a compound command string into individual pipelines / statements.
fn split_command_chain(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    let chars: Vec<char> = raw.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        if escaped {
            current.push(c);
            escaped = false;
            i += 1;
            continue;
        }

        if c == '\\' && !in_single_quote {
            escaped = true;
            current.push(c);
            i += 1;
            continue;
        }

        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            current.push(c);
            i += 1;
            continue;
        }

        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            current.push(c);
            i += 1;
            continue;
        }

        if !in_single_quote && !in_double_quote {
            if c == ';' || c == '|' {
                if c == '|' && i + 1 < len && chars[i + 1] == '|' {
                    segments.push(current.trim().to_string());
                    current.clear();
                    i += 2;
                    continue;
                }
                segments.push(current.trim().to_string());
                current.clear();
                i += 1;
                continue;
            } else if c == '&' && i + 1 < len && chars[i + 1] == '&' {
                segments.push(current.trim().to_string());
                current.clear();
                i += 2;
                continue;
            }
        }

        current.push(c);
        i += 1;
    }

    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }

    segments
}

/// Tokenizes a single command statement respecting quotes and escapes.
fn tokenize_shell_words(statement: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for c in statement.chars() {
        if escaped {
            current.push(c);
            escaped = false;
            continue;
        }

        if c == '\\' && !in_single_quote {
            escaped = true;
            continue;
        }

        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }

        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }

        if c.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            continue;
        }

        current.push(c);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Unwraps wrapper commands (`sudo`, `env`, `npx`, `pnpm`, `bunx`, `sh -c`, `bash -c`, `eval`) into target commands.
fn unwrap_command_tokens(tokens: &[String]) -> Vec<ParsedCommand> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let binary = tokens[0].clone();

    // 0. Handle tool invocation wrapper tokens (e.g. `run_command`, `exec_command`)
    if (binary == "run_command" || binary == "exec_command") && tokens.len() >= 2 {
        let mut unwrapped = unwrap_command_tokens(&tokens[1..]);
        unwrapped.push(build_parsed_command(tokens));
        return unwrapped;
    }

    // 1. Handle `sudo`, `doas`, `env`
    if binary == "sudo" || binary == "doas" {
        let mut idx = 1;
        while idx < tokens.len() && tokens[idx].starts_with('-') {
            idx += 1;
        }
        if idx < tokens.len() {
            let mut unwrapped = unwrap_command_tokens(&tokens[idx..]);
            unwrapped.push(build_parsed_command(tokens));
            return unwrapped;
        }
    }

    if binary == "env" {
        let mut idx = 1;
        while idx < tokens.len() && (tokens[idx].starts_with('-') || tokens[idx].contains('=')) {
            idx += 1;
        }
        if idx < tokens.len() {
            let mut unwrapped = unwrap_command_tokens(&tokens[idx..]);
            unwrapped.push(build_parsed_command(tokens));
            return unwrapped;
        }
    }

    // 2. Handle package/runner wrappers: `npx`, `pnpm`, `bunx`, `yarn`, `bundle exec`, `python -m`
    if (binary == "npx" || binary == "bunx" || binary == "pnpm" || binary == "yarn") && tokens.len() >= 2 {
        let mut idx = 1;
        while idx < tokens.len() && tokens[idx].starts_with('-') {
            idx += 1;
        }
        if idx < tokens.len() {
            let mut unwrapped = unwrap_command_tokens(&tokens[idx..]);
            unwrapped.push(build_parsed_command(tokens));
            return unwrapped;
        }
    }

    if binary == "bundle" && tokens.len() >= 3 && tokens[1] == "exec" {
        let mut unwrapped = unwrap_command_tokens(&tokens[2..]);
        unwrapped.push(build_parsed_command(tokens));
        return unwrapped;
    }

    // 3. Handle subshells: `sh -c "..."`, `bash -c "..."`, `zsh -c "..."`
    if (binary == "sh" || binary == "bash" || binary == "zsh" || binary == "dash") && tokens.len() >= 3 {
        if tokens[1] == "-c" {
            let inner_cmd_str = tokens[2..].join(" ");
            let inner_tree = ShellCommandTree::parse(&inner_cmd_str);
            let mut all = inner_tree.commands;
            all.push(build_parsed_command(tokens));
            return all;
        }
    }

    // 4. Handle `eval "..."`
    if binary == "eval" && tokens.len() >= 2 {
        let inner_cmd_str = tokens[1..].join(" ");
        let inner_tree = ShellCommandTree::parse(&inner_cmd_str);
        let mut all = inner_tree.commands;
        all.push(build_parsed_command(tokens));
        return all;
    }

    vec![build_parsed_command(tokens)]
}

fn build_parsed_command(tokens: &[String]) -> ParsedCommand {
    let binary = tokens[0].clone();
    let mut flags = Vec::new();
    let mut positional_args = Vec::new();

    for tok in &tokens[1..] {
        if tok.starts_with('-') {
            flags.push(tok.clone());
        } else {
            positional_args.push(tok.clone());
        }
    }

    ParsedCommand {
        binary,
        flags,
        positional_args,
        raw_tokens: tokens.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rm_root_detection() {
        let t1 = ShellCommandTree::parse("rm -rf /");
        assert!(t1.is_destructive_rm_root_or_home());

        let t2 = ShellCommandTree::parse("sudo sh -c 'rm -rf /'");
        assert!(t2.is_destructive_rm_root_or_home());

        let t3 = ShellCommandTree::parse("rm -rf ./target");
        assert!(!t3.is_destructive_rm_root_or_home());
    }

    #[test]
    fn test_git_force_push_prod() {
        let t1 = ShellCommandTree::parse("git push origin main --force");
        assert!(t1.is_git_force_push_prod());

        let t2 = ShellCommandTree::parse("git push -f origin feature/auth");
        assert!(!t2.is_git_force_push_prod());
    }

    #[test]
    fn test_prisma_migration() {
        let t1 = ShellCommandTree::parse("npx prisma migrate deploy");
        assert!(t1.is_db_migration());

        let t2 = ShellCommandTree::parse("alembic upgrade head");
        assert!(t2.is_db_migration());
    }

    #[test]
    fn test_python_rm_root_detection() {
        let t = ShellCommandTree::parse("python3 -c 'import shutil; shutil.rmtree(\"/\")'");
        assert!(t.is_destructive_rm_root_or_home());
    }
}
