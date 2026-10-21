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
    let (f, outcome) = db.library_put(NewFile { project_id: Some(project.to_string()), path: path.to_string(), content, origin: Origin::Human, author: "human".into(), personal, allow_secrets })?;
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
    put(&db, &project, &path, text, personal, allow_secrets)?;
    if mv {
        std::fs::remove_file(file)?;
        println!("  Moved: {file} was deleted. If git tracked it, run: git rm --cached {file}   (then commit when you are ready; AutoMemory never commits for you)");
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
