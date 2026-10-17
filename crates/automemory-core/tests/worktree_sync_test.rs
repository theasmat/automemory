use automemory_core::baton;
use automemory_core::db::Database;
use automemory_core::models::*;
use automemory_core::sync::GitSync;
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Helper to initialize a real git repository with an initial commit
fn setup_test_git_repo(base_dir: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
    let main_repo = base_dir.join("main_repo");
    fs::create_dir_all(&main_repo).expect("create main_repo dir");

    // git init
    let init_status = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&main_repo)
        .status()
        .expect("git init failed");
    assert!(init_status.success());

    // git config user info
    let _ = Command::new("git")
        .args(["config", "user.name", "AutoMemory Test"])
        .current_dir(&main_repo)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@automemory.ai"])
        .current_dir(&main_repo)
        .status();

    // Initial commit
    fs::write(main_repo.join("README.md"), "# Test Repo\n").expect("write README");
    let add_status = Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&main_repo)
        .status()
        .expect("git add failed");
    assert!(add_status.success());

    let commit_status = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&main_repo)
        .status()
        .expect("git commit failed");
    assert!(commit_status.success());

    // Create Worktree 1: feat-auth
    let wt1 = base_dir.join("wt_auth");
    let wt1_status = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "feat-auth",
            wt1.to_str().unwrap(),
            "main",
        ])
        .current_dir(&main_repo)
        .status()
        .expect("git worktree add wt1 failed");
    assert!(wt1_status.success());

    // Create Worktree 2: feat-pay
    let wt2 = base_dir.join("wt_pay");
    let wt2_status = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "feat-pay",
            wt2.to_str().unwrap(),
            "main",
        ])
        .current_dir(&main_repo)
        .status()
        .expect("git worktree add wt2 failed");
    assert!(wt2_status.success());

    (
        main_repo.canonicalize().unwrap(),
        wt1.canonicalize().unwrap(),
        wt2.canonicalize().unwrap(),
    )
}

struct TempTestDir(PathBuf);

impl TempTestDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::now_v7()));
        fs::create_dir_all(&path).expect("create temp test dir");
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_worktree_canonical_root_resolution() {
    let temp_dir = TempTestDir::new("wt_res");
    let (main_repo, wt1, wt2) = setup_test_git_repo(temp_dir.path());

    // 1. Verify .git structure
    assert!(main_repo.join(".git").is_dir());
    assert!(wt1.join(".git").is_file());
    assert!(wt2.join(".git").is_file());

    // 2. Resolve canonical roots
    let root_main = GitSync::resolve_git_canonical_root(&main_repo)
        .expect("resolve_git_canonical_root main_repo failed");
    let root_wt1 = GitSync::resolve_git_canonical_root(&wt1)
        .expect("resolve_git_canonical_root wt1 failed");
    let root_wt2 = GitSync::resolve_git_canonical_root(&wt2)
        .expect("resolve_git_canonical_root wt2 failed");

    // All must resolve to the identical canonical root of main_repo
    assert_eq!(root_main, main_repo);
    assert_eq!(root_wt1, main_repo);
    assert_eq!(root_wt2, main_repo);

    // 3. Subdirectory resolution inside worktree
    let wt1_subdir = wt1.join("src").join("auth");
    fs::create_dir_all(&wt1_subdir).expect("create subdir");
    let root_sub = GitSync::resolve_git_canonical_root(&wt1_subdir)
        .expect("resolve from subdir failed");
    assert_eq!(root_sub, main_repo);
}

#[test]
fn test_worktree_project_matching_and_lease_collision() {
    let temp_dir = TempTestDir::new("wt_match");
    let (main_repo, wt1, wt2) = setup_test_git_repo(temp_dir.path());

    let db = Database::open_in_memory().expect("in-memory db open failed");
    let project_id = "proj-worktree-sync-01";

    // Register project pointing to the main_repo path
    let project = Project {
        id: project_id.to_string(),
        slug: "worktree-sync".to_string(),
        display_name: "Worktree Sync Project".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        capture_policy: CapturePolicy::default(),
        context_budget: ContextBudget::default(),
        document_config: DocumentConfig::default(),
        sync_config: SyncConfig::default(),
    };
    db.save_project(&project).expect("save project failed");
    db.add_project_path(project_id, &main_repo.to_string_lossy(), true)
        .expect("register project path failed");

    // 1. Both sibling worktrees resolve to the registered project!
    let found_from_wt1 = db
        .find_project_by_path(&wt1.to_string_lossy())
        .expect("query wt1 failed");
    assert!(found_from_wt1.is_some());
    assert_eq!(found_from_wt1.unwrap().id, project_id);

    let found_from_wt2 = db
        .find_project_by_path(&wt2.to_string_lossy())
        .expect("query wt2 failed");
    assert!(found_from_wt2.is_some());
    assert_eq!(found_from_wt2.unwrap().id, project_id);

    // 2. Cross-worktree lease acquisition & collision detection
    let acquire_res = db
        .acquire_lease(LeaseAcquireArgs {
            project_id: Some(project_id),
            agent: "antigravity-wt1",
            session_id: "sess-wt1-auth",
            scope: "src/auth/jwt.rs",
            scope_type: LeaseScopeType::File,
            goal: "Refactoring JWT token verification in wt_auth",
            ttl_seconds: 300,
        })
        .expect("acquire lease failed");

    match acquire_res {
        LeaseAcquireResult::Acquired { lease } => {
            assert_eq!(lease.scope, "src/auth/jwt.rs");
            assert_eq!(lease.agent, "antigravity-wt1");
        }
        _ => panic!("Expected lease to be acquired cleanly"),
    }

    // 3. Agent in Worktree 2 attempts to modify src/auth/jwt.rs
    let collision = db
        .check_collision(
            Some(project_id),
            "sess-wt2-pay",
            &["src/auth/jwt.rs".to_string()],
            &[],
        )
        .expect("check collision failed");

    assert!(collision.is_some(), "Collision must be detected across worktrees!");
    let conflict = collision.unwrap();
    assert_eq!(conflict.scope, "src/auth/jwt.rs");
    assert_eq!(conflict.holder_agent, "antigravity-wt1");
    assert_eq!(conflict.holder_session_id, "sess-wt1-auth");
    assert!(conflict.remaining_seconds > 0);

    // 4. Modifying an unleased file in Worktree 2 has no collision
    let no_collision = db
        .check_collision(
            Some(project_id),
            "sess-wt2-pay",
            &["src/billing/stripe.rs".to_string()],
            &[],
        )
        .expect("check collision on unleased file failed");
    assert!(no_collision.is_none());
}

#[test]
fn test_worktree_baton_and_decision_sharing() {
    let temp_dir = TempTestDir::new("wt_baton");
    let (main_repo, _wt1, wt2) = setup_test_git_repo(temp_dir.path());

    let db = Database::open_in_memory().expect("in-memory db open failed");
    let project_id = "proj-worktree-baton-01";

    let project = Project {
        id: project_id.to_string(),
        slug: "worktree-baton".to_string(),
        display_name: "Worktree Baton Project".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        capture_policy: CapturePolicy::default(),
        context_budget: ContextBudget::default(),
        document_config: DocumentConfig::default(),
        sync_config: SyncConfig::default(),
    };
    db.save_project(&project).expect("save project failed");
    db.add_project_path(project_id, &main_repo.to_string_lossy(), true)
        .expect("register path");

    // 1. Agent 1 in Worktree 1 records an architectural decision
    let dec = db
        .record_decision(DecisionRecordArgs {
            project_id: Some(project_id),
            topic: "auth-signing",
            decision: "Use Ed25519 for session token signing",
            reason: "Faster verification and smaller signatures than RSA",
            alternatives: &["RSA-2048".to_string(), "ECDSA-P256".to_string()],
            dissent: None,
            agent: "antigravity-wt1",
            symbols: &["auth::jwt::sign".to_string()],
            evidence: &["benchmark: ed25519 is 5x faster".to_string()],
        })
        .expect("record decision failed");

    assert_eq!(dec.agent, "antigravity-wt1");
    assert_eq!(dec.topic, "auth-signing");

    // Query decision from Worktree 2
    let wt2_proj = db
        .find_project_by_path(&wt2.to_string_lossy())
        .expect("query wt2")
        .expect("wt2 project found");

    let query_res = db
        .get_decision(Some(wt2_proj.id.as_str()), "auth-signing")
        .expect("get decision");
    assert!(query_res.active.is_some());
    let fetched = query_res.active.unwrap();
    assert_eq!(fetched.agent, "antigravity-wt1");
    assert_eq!(fetched.decision, "Use Ed25519 for session token signing");

    // 2. Agent 1 in wt1 passes baton to Agent 2
    let packet = baton::build_packet(
        &db,
        Some(project_id),
        "sess-wt1-auth",
        "antigravity-wt1",
        Some("Auth refactoring ready for payment integration in wt_pay"),
    )
    .expect("build baton packet");

    assert!(packet.len() <= baton::BATON_CHAR_BUDGET);

    let saved_baton = baton::pass_baton(
        &db,
        Some(project_id),
        "sess-wt1-auth",
        Some("sess-wt2-pay"),
        "antigravity-wt1",
        Some("Auth refactoring ready for payment integration in wt_pay"),
    )
    .expect("pass baton");

    assert_eq!(saved_baton.to_session.as_deref(), Some("sess-wt2-pay"));

    // Pickup baton from Worktree 2
    let picked = db
        .latest_baton(Some(project_id), Some("sess-wt2-pay"))
        .expect("latest baton query")
        .expect("baton picked up");

    assert_eq!(picked.id, saved_baton.id);
    assert_eq!(picked.agent, "antigravity-wt1");
    assert!(picked.packet.contains("baton from sess-wt1-auth"));
}
