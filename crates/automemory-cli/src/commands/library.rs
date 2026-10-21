//! `automemory library`: documents for agents (PRDs, plans, notes) kept outside git. Adding, editing and removing are yours (password when set);
//! listing, reading and searching are free. The same documents are served to agents by `library.list/get/search`.
use super::brief::{human, open};
use anyhow::{anyhow, bail, Context, Result};
use automemory_core::library::{NewFile, Origin, PutOutcome};
use std::path::Path;

fn project_or_bail(p: Option<String>) -> Result<String> {
    p.ok_or_else(|| anyhow!("this folder is not a registered project: run `automemory connect <agent>` or register it first"))
}

fn put(db: &automemory_core::db::Database, project: &str, path: &str, content: String, personal: bool, allow_secrets: bool) -> Result<()> {
    let (f, outcome) = db.library_put(NewFile { project_id: Some(project.to_string()), path: path.to_string(), content, origin: Origin::Human, author: "human".into(), personal, allow_secrets, linked: false })?;
    match outcome {
        PutOutcome::Created => println!("✓ Added {} ({} KB). Agents see: {}", f.path, f.size / 1024 + 1, f.l0),
        PutOutcome::Updated => println!("✓ Updated {} (revision {}).", f.path, f.revisions),
        PutOutcome::Unchanged => println!("{} is already up to date.", f.path),
    }
    Ok(())
}

/// Copies a file in (`--as` chooses the library path, default its file name). `--move` deletes the original afterwards; it is never committed or untracked for you.
pub fn add(file: &str, name: Option<String>, mv: bool, personal: bool, allow_secrets: bool) -> Result<()> {
    let text = std::fs::read_to_string(file).with_context(|| format!("cannot read {file} as text"))?;
    let path = name.unwrap_or_else(|| Path::new(file).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let (db, project) = human("add a file to the project library")?;
    let project = project_or_bail(project)?;
    // Where git stands before the file goes: tracked files need `git rm --cached`, others need nothing.
    let abs = std::fs::canonicalize(file).unwrap_or_else(|_| Path::new(file).to_path_buf());
    let dir = abs.parent().map(Path::to_path_buf).unwrap_or_default();
    let git = |args: &[&str]| std::process::Command::new("git").arg("-C").arg(&dir).args(args).arg(abs.as_os_str()).output().ok();
    let in_repo = std::process::Command::new("git").arg("-C").arg(&dir).args(["rev-parse", "--is-inside-work-tree"]).output().map(|o| o.status.success()).unwrap_or(false);
    let tracked = in_repo && git(&["ls-files", "--error-unmatch"]).is_some_and(|o| o.status.success());
    put(&db, &project, &path, text, personal, allow_secrets)?;
    if mv {
        std::fs::remove_file(file)?;
        if tracked {
            println!("  Moved: {file} was deleted from the folder, but git still tracks it. Stop tracking it with:\n      git rm --cached {file}\n  and commit when you are ready (AutoMemory never commits or changes git for you). Older commits still contain the file.");
        } else {
            println!("  Moved: {file} was deleted. Git was not tracking it, so there is nothing to do in git.");
        }
    }
    Ok(())
}

/// Creates a file (or edits an existing one) in $EDITOR.
pub fn new(name: &str, personal: bool) -> Result<()> {
    let (db, project) = human("write to the project library")?;
    let project = project_or_bail(project)?;
    let existing = db.library_get(Some(&project), name)?.map(|f| f.content).unwrap_or_else(|| format!("# {}\n\n", name.rsplit('/').next().unwrap_or(name).trim_end_matches(".md")));
    let tmp = std::env::temp_dir().join(format!("automemory-lib-{}-{}", std::process::id(), name.replace('/', "_")));
    std::fs::write(&tmp, &existing)?;
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".into());
    let status = std::process::Command::new(&editor).arg(&tmp).status().with_context(|| format!("could not start {editor}"))?;
    let text = std::fs::read_to_string(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    if !status.success() {
        bail!("{editor} did not finish: nothing saved");
    }
    put(&db, &project, name, text, personal, false)
}

pub fn ls(prefix: Option<String>) -> Result<()> {
    let (db, project) = open()?;
    let files = db.library_list(project.as_deref(), prefix.as_deref())?;
    if files.is_empty() {
        println!("The library is empty. Add a document: automemory library add ~/Desktop/PRD.md --as plans/prd.md");
    }
    for f in files {
        println!("{:<36} {:>6} B  {}  [{}{}]", f.path, f.size, f.l0.chars().take(70).collect::<String>(), f.author, if f.scope == "personal" { ", personal" } else { "" });
    }
    Ok(())
}

pub fn show(path: &str, tier: u8) -> Result<()> {
    let (db, project) = open()?;
    let f = db.library_get(project.as_deref(), path)?.ok_or_else(|| anyhow!("no such library file: {path} (see: automemory library ls)"))?;
    print!("{}", match tier { 0 => format!("{}\n", f.l0), 1 => f.l1, _ => f.content });
    Ok(())
}

pub fn search(query: &str, limit: usize) -> Result<()> {
    let (db, project) = open()?;
    let hits = db.library_search(project.as_deref(), query, limit.clamp(1, 20))?;
    if hits.is_empty() {
        println!("Nothing found.");
    }
    for h in hits {
        println!("{}  [{}]\n   {}", h.path, h.heading, h.snippet.replace('\n', " "));
    }
    Ok(())
}

pub fn rm(path: &str) -> Result<()> {
    let (db, project) = human("remove a file from the project library")?;
    println!("{}", if db.library_remove(project.as_deref(), path)? { format!("✓ Removed {path} from the library (a file you copied from elsewhere is untouched).") } else { format!("No such library file: {path}") });
    Ok(())
}

pub fn history(path: &str) -> Result<()> {
    let (db, project) = open()?;
    for (rev, author, origin, at) in db.library_revisions(project.as_deref(), path)? {
        println!("rev {rev:<3} {at}  {author} ({origin})");
    }
    Ok(())
}

/// The project of this folder: (id, slug, root).
fn here(db: &automemory_core::db::Database, project: Option<String>) -> Result<(String, String, std::path::PathBuf)> {
    let pid = project_or_bail(project)?;
    let slug = db.get_project_by_id(&pid)?.map(|p| p.slug).unwrap_or_default();
    let root = super::brief::project_root(db, Some(&pid));
    Ok((pid, slug, root))
}

/// One pass: edits made in `.automemory/library/` become revisions, files named in the config are read where they are, copies and INDEX.md are written.
pub fn sync() -> Result<()> {
    let (db, project) = open()?;
    let (pid, slug, root) = here(&db, project)?;
    let cfg = automemory_core::config::GlobalConfig::load()?;
    let r = automemory_core::library::fs::sync_project(&db, &pid, &slug, &root, &cfg.library)?;
    println!("✓ Library synced: {} copied into .automemory/library, {} taken back from it, {} kept-in-place file(s) read, {} copy(ies) removed.", r.written, r.ingested, r.linked, r.removed);
    for s in r.skipped {
        println!("  skipped {s}");
    }
    Ok(())
}

/// Adds a pattern (relative to the project root) to the global config: those files are kept in sync where they are, tracked by git or not.
pub fn link(pattern: &str, project_only: bool) -> Result<()> {
    if pattern.starts_with('/') || pattern.contains("..") || pattern.contains(':') {
        bail!("use a path or pattern relative to the project folder, such as PRD-private.md or plans/**/*.md");
    }
    automemory_core::auth::require_human(automemory_core::auth::stored_password_hash().as_deref(), "change which files the library keeps in sync")?;
    let (db, project) = open()?;
    let (_, slug, _) = here(&db, project)?;
    let mut cfg = automemory_core::config::GlobalConfig::load()?;
    let list = if project_only { &mut cfg.library.projects.entry(slug.clone()).or_default().files } else { &mut cfg.library.files };
    if !list.iter().any(|p| p == pattern) {
        list.push(pattern.to_string());
    }
    cfg.save()?;
    println!("✓ \"{pattern}\" is now kept in sync{}. Git does not matter: tracked, ignored or untracked files are all read as they are. Run: automemory library sync", if project_only { format!(" for project {slug}") } else { " in every project".into() });
    Ok(())
}

pub fn unlink(pattern: &str) -> Result<()> {
    automemory_core::auth::require_human(automemory_core::auth::stored_password_hash().as_deref(), "change which files the library keeps in sync")?;
    let mut cfg = automemory_core::config::GlobalConfig::load()?;
    let before = cfg.library.files.len() + cfg.library.projects.values().map(|p| p.files.len()).sum::<usize>();
    cfg.library.files.retain(|p| p != pattern);
    for p in cfg.library.projects.values_mut() {
        p.files.retain(|x| x != pattern);
    }
    let after = cfg.library.files.len() + cfg.library.projects.values().map(|p| p.files.len()).sum::<usize>();
    cfg.save()?;
    println!("{}", if before == after { format!("\"{pattern}\" was not in the config.") } else { format!("✓ \"{pattern}\" removed. Files already in the library stay until: automemory library rm <path>") });
    Ok(())
}

pub fn links() -> Result<()> {
    let (db, project) = open()?;
    let (pid, slug, _) = here(&db, project)?;
    let cfg = automemory_core::config::GlobalConfig::load()?;
    let pats = cfg.library.patterns_for(&slug);
    if pats.is_empty() {
        println!("No files are kept in sync in place. Add one: automemory library link PRD-private.md");
    }
    for p in pats {
        println!("{p}");
    }
    for f in db.library_list(Some(&pid), None)?.into_iter().filter(|f| f.linked) {
        println!("   in library: {}", f.path);
    }
    println!("Folder kept out of git status: {}", if cfg.library.exclude_from_git { "yes (.git/info/exclude)" } else { "no ([library] exclude_from_git = false)" });
    Ok(())
}
