//! The contract every reader must pass. A new reader's test is one call to `conformance` with a small anonymised fixture.
use super::*;
use crate::continuity::import::import_project;
use crate::db::Database;
use sha2::{Digest, Sha256};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() { copy_dir(&src, &dst) } else { std::fs::copy(&src, &dst).unwrap(); }
    }
}

fn hash_tree(dir: &Path) -> String {
    let mut files = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            if e.path().is_dir() { walk(&e.path(), out) } else { out.push(e.path()) }
        }
    }
    walk(dir, &mut files);
    files.sort();
    let mut h = Sha256::new();
    for f in files {
        h.update(f.to_string_lossy().as_bytes());
        h.update(std::fs::read(f).unwrap());
    }
    format!("{:x}", h.finalize())
}

/// `append_message(source_path)`: add exactly one more valid message the way the agent would (and, if the format can have
/// corrupt entries, one corrupt entry before it). `secret`: a fragment of a secret planted in the fixture.
pub fn conformance(reader: &dyn TranscriptReader, fixture_home: &Path, project_root: &Path, append_message: &dyn Fn(&Path), secret: &str) {
    let tmp = std::env::temp_dir().join(format!("am-testkit-{}-{}", reader.id(), std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    copy_dir(fixture_home, &tmp);

    let sessions = reader.sessions(&tmp, project_root);
    assert!(!sessions.is_empty(), "finds the project's sessions");
    assert!(!reader.locations(&tmp).is_empty(), "says where it reads");
    assert!(reader.sessions(&tmp, Path::new("/somewhere/else")).is_empty(), "other projects' chats are not returned");

    // Read-only: reading changes nothing in the agent's files.
    let before = hash_tree(&tmp);
    let first = reader.read(&sessions[0], None).unwrap();
    assert_eq!(before, hash_tree(&tmp), "the source files are untouched");
    assert!(!first.messages.is_empty() && first.cursor.is_some(), "reads messages and a cursor");

    // Incremental: nothing new, then exactly the new line, even behind a corrupt one.
    let again = reader.read(&sessions[0], first.cursor.as_ref()).unwrap();
    assert!(again.messages.is_empty(), "nothing is read twice");
    append_message(&sessions[0].path);
    let more = reader.read(&sessions[0], first.cursor.as_ref()).unwrap();
    assert_eq!(more.messages.iter().filter(|m| m.role != Role::Artifact).count(), 1, "a corrupt line is skipped and the new message arrives once: {:?}", more.messages);

    // Through the importer: secrets removed, each message stored once.
    let db = Database::open_in_memory().unwrap();
    let r1 = import_project(&db, reader, &tmp, Some("p"), project_root).unwrap();
    assert!(r1.new_messages > 0);
    let r2 = import_project(&db, reader, &tmp, Some("p"), project_root).unwrap();
    assert_eq!(r2.new_messages, 0, "importing twice adds nothing");
    let all: String = db.chat_sessions(Some("p"), None).unwrap().iter().flat_map(|s| db.chat_get(&s.id, 0, 1000).unwrap()).map(|m| m.text).collect::<Vec<_>>().join("\n");
    assert!(!all.contains(secret), "the planted secret is not in the archive");
    assert!(!all.contains(tmp.to_string_lossy().as_ref()), "the home folder is shown as ~");
    let _ = std::fs::remove_dir_all(&tmp);
}
