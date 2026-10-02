use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use crate::db::Database;
use crate::docs::scanner::DocumentScanner;
use crate::models::Project;

pub struct DocumentWatcher {
    db: Arc<Database>,
    scanner: DocumentScanner,
}

impl DocumentWatcher {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            scanner: DocumentScanner::new(db.clone()),
            db,
        }
    }

    pub fn scanner(&self) -> &DocumentScanner {
        &self.scanner
    }

    /// Watch a list of project roots for document modifications in background.
    pub fn start_watching(&self, projects_with_paths: Vec<(Project, PathBuf)>) -> Result<()> {
        let (tx, rx) = channel::<notify::Result<Event>>();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        for (project, path) in &projects_with_paths {
            if path.exists() && path.is_dir() {
                info!("Starting file watch for project '{}' at {:?}", project.slug, path);
                let _ = watcher.watch(path, RecursiveMode::Recursive);
            }
        }

        // Spawn processor thread
        let db = self.db.clone();
        std::thread::spawn(move || {
            // Keep watcher alive inside thread
            let _watcher = watcher;
            let scanner = DocumentScanner::new(db);
            let mut last_processed: std::collections::HashMap<PathBuf, std::time::Instant> = std::collections::HashMap::new();

            while let Ok(res) = rx.recv() {
                match res {
                    Ok(event) => {
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                for file_path in event.paths {
                                    if !file_path.is_file() {
                                        continue;
                                    }
                                    let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");
                                    if ext != "md" && ext != "txt" && ext != "json" {
                                        continue;
                                    }

                                    // 500ms sliding debounce per file path
                                    if let Some(last_time) = last_processed.get(&file_path) {
                                        if last_time.elapsed() < Duration::from_millis(500) {
                                            continue;
                                        }
                                    }
                                    last_processed.insert(file_path.clone(), std::time::Instant::now());

                                    // Prune old debounce entries if map grows large
                                    if last_processed.len() > 1000 {
                                        last_processed.retain(|_, time| time.elapsed() < Duration::from_secs(60));
                                    }

                                    for (project, root) in &projects_with_paths {
                                        if file_path.starts_with(root) {
                                            info!("⚡ Real-time file change detected: {:?} in project '{}'", file_path, project.slug);
                                            let _ = scanner.process_file(project, root, &file_path);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        warn!("File watch error: {:?}", e);
                    }
                }
            }
        });

        Ok(())
    }
}
