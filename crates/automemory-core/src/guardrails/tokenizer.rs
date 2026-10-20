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
    /// A downloader or decoder (`curl`, `wget`, `base64 -d`) piped straight into a shell.
    pub pipes_to_shell: bool,
    /// The command nested deeper than we inspect (`$($($(...`); what lies beyond was not examined.
    #[serde(default)]
    pub truncated: bool,
}

thread_local! {
    static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TRUNCATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Deeper than any real command; also keeps recursion far from the stack limit (it aborted the hook near 8000).
const MAX_DEPTH: usize = 40;

struct DepthGuard;
impl DepthGuard {
    fn enter() -> Option<Self> {
        DEPTH.with(|d| {
            if d.get() >= MAX_DEPTH {
                TRUNCATED.with(|t| t.set(true));
                None
            } else {
                d.set(d.get() + 1);
                Some(DepthGuard)
            }
        })
    }
}
impl Drop for DepthGuard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get() - 1));
    }
}

impl ShellCommandTree {
    /// Tokenizes a raw shell command string, resolves quotes/pipes/subshells (`sh -c`, `sudo`, `npx`, `eval`),
    /// and produces a normalized list of individual parsed commands.
    pub fn parse(raw: &str) -> Self {
        if DEPTH.with(|d| d.get()) == 0 {
            TRUNCATED.with(|t| t.set(false));
        }
        let Some(_guard) = DepthGuard::enter() else {
            return Self { commands: Vec::new(), raw_input: raw.to_string(), pipes_to_shell: false, truncated: true };
        };
        let stripped = strip_heredoc_bodies(raw);
        let mut commands = Vec::new();
        let mut pipes_to_shell = false;
        let mut prev: Option<Vec<String>> = None;

        for (seg, piped_from_prev) in split_command_chain(&stripped) {
            let tokens = tokenize_shell_words(&seg);
            if tokens.is_empty() {
                prev = None;
                continue;
            }
            if piped_from_prev {
                if let Some(upstream) = &prev {
                    pipes_to_shell |= is_fetcher(upstream) && is_bare_shell(&tokens);
                }
            }

            commands.extend(unwrap_command_tokens(&tokens));
            for inner in command_substitutions(&seg) {
                let sub = Self::parse(&inner);
                pipes_to_shell |= sub.pipes_to_shell;
                commands.extend(sub.commands);
            }
            prev = Some(tokens);
        }

        Self {
            commands,
            raw_input: raw.to_string(),
            pipes_to_shell,
            truncated: TRUNCATED.with(|t| t.get()),
        }
    }

    /// Word tokens of each command in a pipeline or chain (quotes resolved, here-document
    /// bodies dropped). Redirect operators stay as tokens, which the parsed commands drop.
    pub fn segment_tokens(raw: &str) -> Vec<Vec<String>> {
        split_command_chain(&strip_heredoc_bodies(raw))
            .into_iter()
            .map(|(seg, _)| tokenize_shell_words(&seg))
            .filter(|t| !t.is_empty())
            .collect()
    }

    /// Text for regex rules: the command with the printed or searched text of
    /// `echo`, `grep`, `git commit -m` and friends blanked out, so a command that
    /// merely mentions a dangerous string is not mistaken for running it. Shell
    /// substitutions (`$(...)`, backticks) still execute, so they are kept.
    pub fn rule_text(raw: &str) -> String {
        split_command_chain(&strip_heredoc_bodies(raw))
            .into_iter()
            .map(|(seg, _)| {
                let tokens = tokenize_shell_words(&seg);
                if !is_data_only(&tokens) {
                    return seg;
                }
                let mut kept: Vec<&str> = tokens
                    .iter()
                    .take(1)
                    .chain(tokens.iter().skip(1).filter(|t| t.starts_with('-')))
                    .map(String::as_str)
                    .collect();
                let subs = command_substitutions(&seg);
                kept.extend(subs.iter().map(String::as_str));
                kept.join(" ")
            })
            .collect::<Vec<_>>()
            .join(" ; ")
    }

    /// Returns true if any parsed command is a destructive `rm -rf /` or `rm -rf ~`.
    pub fn is_destructive_rm_root_or_home(&self) -> bool {
        let mut cwd_is_root = false;
        for cmd in &self.commands {
            if cmd.is_binary("cd") {
                cwd_is_root = cmd
                    .positional_args
                    .first()
                    .is_some_and(|a| is_root_or_home_path(a));
                continue;
            }
            if cmd.is_binary("find") && find_deletes_from_root(cmd) {
                return true;
            }
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
                        if is_root_or_home_path(a)
                            || (cwd_is_root && matches!(a, "." | "*" | "./*" | ".*"))
                        {
                            return true;
                        }
                    }
                }
            }

            // Inline language scripts: python -c "...", node -e "...", etc.
            if cmd.is_binary("python")
                || cmd.is_binary("python3")
                || cmd.is_binary("node")
                || cmd.is_binary("ruby")
                || cmd.is_binary("perl")
            {
                let is_eval = cmd.has_flag("-c") || cmd.has_flag("-e");
                if is_eval {
                    for arg in &cmd.positional_args {
                        let lower = arg.to_lowercase();
                        if (lower.contains("rmtree")
                            || lower.contains("rmdir")
                            || lower.contains("rmsync")
                            || lower.contains("unlink"))
                            && (lower.contains("'/")
                                || lower.contains("\"/")
                                || lower.contains("'~")
                                || lower.contains("\"~")
                                || lower.contains("os.environ['home']"))
                        {
                            return true;
                        }
                        if (lower.contains("system(")
                            || lower.contains("popen(")
                            || lower.contains("exec(")
                            || lower.contains("spawn("))
                            && (lower.contains("rm -rf /")
                                || lower.contains("rm -rf ~")
                                || lower.contains("rm -rf $home"))
                        {
                            return true;
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
            if cmd.is_binary("git")
                && cmd.positional_args.first().map(|s| s.as_str()) == Some("push")
            {
                let has_force = cmd
                    .flags
                    .iter()
                    .any(|f| f == "--force" || f == "-f" || f == "--force-with-lease")
                    || cmd
                        .positional_args
                        .iter()
                        .skip(1)
                        .any(|a| a.starts_with('+'));
                let targets_prod = cmd.positional_args.iter().skip(1).any(|arg| {
                    let a = arg.trim_start_matches('+').to_lowercase();
                    // `src:dst` pushes to dst; `refs/heads/main` is `main`.
                    let dst = a.rsplit(':').next().unwrap_or("");
                    let branch = dst.strip_prefix("refs/heads/").unwrap_or(dst);
                    matches!(
                        branch,
                        "main" | "master" | "prod" | "production" | "release"
                    )
                });

                if has_force && targets_prod {
                    return true;
                }
            }
        }
        false
    }

    /// Recursive chmod/chown on `/` or the home directory.
    pub fn is_recursive_perm_change_root(&self) -> bool {
        self.commands.iter().any(|c| {
            (c.is_binary("chmod") || c.is_binary("chown") || c.is_binary("chgrp"))
                && c.flags
                    .iter()
                    .any(|f| f == "--recursive" || (!f.starts_with("--") && f.contains('R')))
                && c.positional_args
                    .iter()
                    .any(|a| is_root_or_home_path(a.trim()))
        })
    }

    /// `curl … | sh` style execution of unreviewed downloaded or decoded text.
    pub fn is_pipe_to_shell(&self) -> bool {
        self.pipes_to_shell
    }

    /// Returns true if any command is a raw disk format / block overwrite.
    pub fn is_raw_disk_format(&self) -> bool {
        for cmd in &self.commands {
            if cmd.binary.starts_with("mkfs") {
                return true;
            }
            if cmd.is_binary("dd")
                && (cmd.flags.iter().any(|f| f.starts_with("of=/dev/"))
                    || cmd
                        .positional_args
                        .iter()
                        .any(|a| a.starts_with("of=/dev/")))
            {
                return true;
            }
        }
        false
    }

    /// Returns true if any command is an unescaped raw database drop or truncate.
    pub fn is_raw_db_drop(&self) -> bool {
        let upper = self.raw_input.to_uppercase();
        upper.contains("DROP DATABASE")
            || upper.contains("TRUNCATE TABLE")
            || upper.contains("DROP SCHEMA")
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
            if cmd.is_binary("kubectl")
                && cmd.positional_args.first().map(|s| s.as_str()) == Some("delete")
            {
                return true;
            }
            if cmd.is_binary("helm")
                && cmd.positional_args.first().map(|s| s.as_str()) == Some("uninstall")
            {
                return true;
            }
            if cmd.is_binary("terraform")
                && cmd.positional_args.first().map(|s| s.as_str()) == Some("destroy")
            {
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
                if cmd.positional_args.first().map(|s| s.as_str()) == Some("reset")
                    && cmd.has_flag("--hard")
                {
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
/// Drops here-document bodies (`<<EOF ... EOF`): they are data fed to a command, not
/// commands. A body is dropped only when the `<<` is outside quotes, is followed by a
/// delimiter word, and a line holding exactly that delimiter really follows; otherwise
/// the lines stay visible, so a stray `<<` can never hide a later command.
fn strip_heredoc_bodies(raw: &str) -> String {
    if !raw.contains("<<") {
        return raw.to_string();
    }
    let lines: Vec<&str> = raw.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        out.push(lines[i]);
        if let Some(delim) = heredoc_delimiter(lines[i]) {
            if let Some(end) = lines[i + 1..].iter().position(|l| l.trim() == delim) {
                i += end + 1; // skip the body and the closing line
            }
        }
        i += 1;
    }
    out.join("\n")
}

/// The delimiter word of a here-document opened on `line`, if any.
fn heredoc_delimiter(line: &str) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let (mut in_single, mut in_double) = (false, false);
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '<' if !in_single && !in_double && chars.get(i + 1) == Some(&'<') => {
                let mut j = i + 2;
                if chars.get(j) == Some(&'<') {
                    return None; // here-string `<<<`
                }
                if chars.get(j) == Some(&'-') {
                    j += 1;
                }
                while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                    j += 1;
                }
                if matches!(chars.get(j), Some('\'' | '"')) {
                    j += 1;
                }
                let word: String = chars[j..]
                    .iter()
                    .take_while(|c| c.is_alphanumeric() || **c == '_')
                    .collect();
                // A word starting with a digit is a shift (`1 << 3`), not a delimiter.
                return (!word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit()))
                    .then_some(word);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Commands whose arguments are text to print or search, never code to run.
fn is_data_only(tokens: &[String]) -> bool {
    let rest = if tokens.first().is_some_and(|t| t == "sudo") { &tokens[1..] } else { tokens };
    let Some(bin) = rest.first().map(|t| t.rsplit('/').next().unwrap_or(t)) else {
        return false;
    };
    matches!(bin, "echo" | "printf" | "grep" | "egrep" | "fgrep" | "rg" | "ag" | "ack")
        || (bin == "git"
            && rest.iter().skip(1).find(|t| !t.starts_with('-')).is_some_and(|t| {
                matches!(t.as_str(), "commit" | "tag" | "notes")
            }))
}

/// Bodies of `$(...)` and `` `...` `` that the shell would run (not inside single quotes).
fn command_substitutions(seg: &str) -> Vec<String> {
    let chars: Vec<char> = seg.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut in_single) = (0, false);
    while i < chars.len() {
        match chars[i] {
            '\\' if !in_single => i += 1,
            '\'' => in_single = !in_single,
            '$' if !in_single && chars.get(i + 1) == Some(&'(') => {
                let (mut depth, mut j) = (1, i + 2);
                while j < chars.len() && depth > 0 {
                    match chars[j] {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                let end = if depth == 0 { j - 1 } else { j };
                out.push(chars[i + 2..end].iter().collect());
                i = j.saturating_sub(1);
            }
            '`' if !in_single => {
                if let Some(rel) = chars[i + 1..].iter().position(|&c| c == '`') {
                    out.push(chars[i + 1..i + 1 + rel].iter().collect());
                    i += rel + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn is_root_or_home_path(a: &str) -> bool {
    matches!(
        a,
        "/" | "/*" | "~" | "~/" | "~/*" | "$HOME" | "$HOME/" | "$HOME/*" | "${HOME}" | "${HOME}/*"
    )
}

/// `find <root|home> … -delete` or `-exec rm …`.
fn find_deletes_from_root(cmd: &ParsedCommand) -> bool {
    let starts_at_root = cmd
        .raw_tokens
        .iter()
        .skip(1)
        .take_while(|t| !t.starts_with('-'))
        .any(|t| is_root_or_home_path(t.trim()));
    let deletes = cmd.raw_tokens.iter().any(|t| t == "-delete")
        || cmd
            .raw_tokens
            .windows(2)
            .any(|w| (w[0] == "-exec" || w[0] == "-execdir") && w[1].ends_with("rm"));
    starts_at_root && deletes
}

fn is_fetcher(tokens: &[String]) -> bool {
    match tokens.first().map(String::as_str) {
        Some("curl" | "wget") => true,
        Some("base64") => tokens
            .iter()
            .any(|t| t == "-d" || t == "-D" || t == "--decode"),
        _ => false,
    }
}

/// A shell reading its script from stdin (no `-c`), optionally under sudo.
fn is_bare_shell(tokens: &[String]) -> bool {
    let rest = if tokens.first().is_some_and(|t| t == "sudo") {
        &tokens[1..]
    } else {
        tokens
    };
    matches!(
        rest.first().map(String::as_str),
        Some("sh" | "bash" | "zsh" | "dash")
    ) && !rest.iter().any(|t| t == "-c")
}

/// Splits on `;`, `&&`, `||` and `|`. The flag marks a segment fed by a single pipe.
fn split_command_chain(raw: &str) -> Vec<(String, bool)> {
    let mut segments = Vec::new();
    let mut piped = false; // the segment being built follows a single `|`
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
            if c == ';' || c == '|' || c == '\n' {
                if c == '|' && i + 1 < len && chars[i + 1] == '|' {
                    segments.push((current.trim().to_string(), piped));
                    piped = false;
                    current.clear();
                    i += 2;
                    continue;
                }
                segments.push((current.trim().to_string(), piped));
                piped = c == '|';
                current.clear();
                i += 1;
                continue;
            } else if c == '&' && i + 1 < len && chars[i + 1] == '&' {
                segments.push((current.trim().to_string(), piped));
                piped = false;
                current.clear();
                i += 2;
                continue;
            }
        }

        current.push(c);
        i += 1;
    }

    if !current.trim().is_empty() {
        segments.push((current.trim().to_string(), piped));
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
    // `sudo sudo sudo ...` recurses once per wrapper; stop at the depth limit and keep what we have.
    let Some(_guard) = DepthGuard::enter() else {
        return vec![build_parsed_command(tokens)];
    };

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
    if (binary == "npx" || binary == "bunx" || binary == "pnpm" || binary == "yarn")
        && tokens.len() >= 2
    {
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
    if ((binary == "sh" || binary == "bash" || binary == "zsh" || binary == "dash")
        && tokens.len() >= 3)
        && tokens[1] == "-c"
    {
        let inner_cmd_str = tokens[2..].join(" ");
        let inner_tree = ShellCommandTree::parse(&inner_cmd_str);
        let mut all = inner_tree.commands;
        all.push(build_parsed_command(tokens));
        return all;
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

    #[test]
    fn test_adversarial_bypass_attempts() {
        // Subshell / eval unwrapping must not hide the payload.
        for cmd in [
            "sh -c 'rm -rf /'",
            "bash -c 'rm -rf ~'",
            "eval 'rm -rf /'",
            "sudo sh -c 'rm -rf /'",
            "python -c \"import os; os.system('rm -rf /')\"",
            "node -e \"require('child_process').exec('rm -rf ~')\"",
        ] {
            let tree = ShellCommandTree::parse(cmd);
            assert!(
                tree.is_destructive_rm_root_or_home(),
                "bypass missed: {cmd}"
            );
        }

        // Raw disk overwrite in its common spellings.
        for cmd in ["dd if=/dev/zero of=/dev/sda", "dd of=/dev/sda if=/dev/zero"] {
            let tree = ShellCommandTree::parse(cmd);
            assert!(tree.is_raw_disk_format(), "dd bypass missed: {cmd}");
        }

        // Benign lookalikes must stay quiet.
        assert!(!ShellCommandTree::parse("dd if=/dev/zero of=backup.img").is_raw_disk_format());
        assert!(!ShellCommandTree::parse("rm -rf ./target").is_destructive_rm_root_or_home());
    }

    #[test]
    fn heredoc_bodies_are_data_not_commands() {
        let danger = ["r", "m -", "rf /"].concat();
        let script = format!("cat > notes.md <<'EOF'\nrun {danger} to wipe\nEOF\nls");
        let t = ShellCommandTree::parse(&script);
        assert!(!t.is_destructive_rm_root_or_home());
        assert!(!ShellCommandTree::rule_text(&script).contains(&danger));
        // A command after the heredoc still counts.
        let real = format!("cat <<EOF\nhello\nEOF\n{danger}");
        assert!(ShellCommandTree::parse(&real).is_destructive_rm_root_or_home());
        // `<<<` here-strings are not heredocs.
        assert!(!strip_heredoc_bodies("grep x <<< \"a\"").is_empty());
    }

    #[test]
    fn a_newline_separates_commands() {
        let danger = ["r", "m -", "rf /"].concat();
        assert!(ShellCommandTree::parse(&format!("ls\n{danger}")).is_destructive_rm_root_or_home());
    }

    #[test]
    fn a_stray_double_angle_never_hides_later_commands() {
        let danger = ["r", "m -", "rf /"].concat();
        for first in ["echo \"x <<EOF\"", "echo $((1 << 3))", "echo 'a <<EOF'", "echo $((1<<n))"] {
            let script = format!("{first}\n{danger}");
            assert!(
                ShellCommandTree::parse(&script).is_destructive_rm_root_or_home(),
                "{first}"
            );
        }
        // An unterminated heredoc does not swallow the rest either.
        let open = format!("cat <<EOF\n{danger}");
        assert!(ShellCommandTree::parse(&open).is_destructive_rm_root_or_home());
    }
}

#[cfg(test)]
mod depth_tests {
    use super::*;

    #[test]
    fn absurd_nesting_is_cut_off_instead_of_overflowing_the_stack() {
        for unit in ["$((", "$(", "sudo ", "sh -c '", "eval "] {
            let tree = ShellCommandTree::parse(&format!("{}rm -rf /", unit.repeat(20_000)));
            assert!(tree.truncated, "{unit}");
        }
        assert!(!ShellCommandTree::parse("sudo sh -c 'echo $(date)'").truncated, "normal nesting is untouched");
        assert!(ShellCommandTree::parse("rm -rf /").is_destructive_rm_root_or_home(), "and the flag resets between calls");
    }
}
