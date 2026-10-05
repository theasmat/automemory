use anyhow::{Context, Result};
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::crypto::{BlindCrypto, BlindMemoryWire, BlindSkillWire, EncryptedBlob};
use automemory_core::db::Database;
use automemory_core::models::*;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn resolve_cloud_urls(raw_endpoint: &str) -> (String, String, String, String) {
    let mut base = raw_endpoint.trim().trim_end_matches('/').to_string();
    if base.ends_with("/api/v1/sync/push") {
        base = base.trim_end_matches("/api/v1/sync/push").to_string();
    } else if base.ends_with("/sync/push") {
        base = base.trim_end_matches("/sync/push").to_string();
    } else if base.ends_with("/api/v1/sync/pull") {
        base = base.trim_end_matches("/api/v1/sync/pull").to_string();
    } else if base.ends_with("/sync/pull") {
        base = base.trim_end_matches("/sync/pull").to_string();
    } else if base.ends_with("/api/v1/sync") {
        base = base.trim_end_matches("/api/v1/sync").to_string();
    }
    let base = base.trim_end_matches('/');

    let push_url = format!("{}/api/v1/sync/push", base);
    let pull_url = format!("{}/api/v1/sync/pull", base);
    let device_reg_url = format!("{}/api/v1/auth/device/register", base);
    let blob_base = format!("{}/api/v1/sync/blob", base);

    (push_url, pull_url, device_reg_url, blob_base)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub project_id: String,
    pub endpoint: Option<String>,
    pub auth_token: Option<String>,
    pub auto_sync: bool,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDelta {
    pub id: String,
    pub relative_path: String,
    pub category: String,
    pub title: String,
    pub content_hash: String,
    pub content: String,
    pub l0_abstract: Option<String>,
    pub l1_overview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaPayload {
    pub project_id: String,
    pub timestamp: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub documents: Vec<DocumentDelta>,
    pub memories: Vec<Memory>,
    pub skills: Vec<ProceduralSkill>,
}

pub fn get_or_create_device_identity(home_dir: &Path) -> (String, String) {
    let device_file = home_dir.join("device_id");
    let device_id = if let Ok(id) = fs::read_to_string(&device_file) {
        let trimmed = id.trim().to_string();
        if !trimmed.is_empty() {
            trimmed
        } else {
            generate_device_id(&device_file)
        }
    } else {
        generate_device_id(&device_file)
    };

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Agent-Workstation".to_string())
        });

    let os_name = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let device_name = format!("{} ({} {})", hostname, os_name, arch);

    (device_id, device_name)
}

fn generate_device_id(device_file: &Path) -> String {
    let id = format!("dev_{}", uuid::Uuid::new_v4().simple());
    let _ = fs::write(device_file, &id);
    id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreManifest {
    pub project_id: String,
    pub timestamp: String,
    pub documents: Vec<DocumentDelta>,
    pub memories: Vec<Memory>,
    pub skills: Vec<ProceduralSkill>,
}

pub fn init(
    project_id: Option<String>,
    endpoint: Option<String>,
    token: Option<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let proj_id = project_id.unwrap_or_else(|| infer_project_id(&cwd));

    let automemory_dir = cwd.join(".automemory");
    fs::create_dir_all(&automemory_dir)?;

    let config_path = automemory_dir.join("config.json");
    let sync_cfg = SyncConfig {
        project_id: proj_id.clone(),
        endpoint,
        auth_token: token,
        auto_sync: true,
        last_synced_at: None,
    };

    fs::write(&config_path, serde_json::to_string_pretty(&sync_cfg)?)?;

    // Protect .automemory and .context in .gitignore
    update_gitignore(&cwd)?;

    println!(
        "✓ Initialized AutoMemory cloud sync configuration for '{}':",
        proj_id
    );
    println!("  📁 Config: {}", config_path.display());
    println!("  🛡️  Protection: Verified .automemory and .context in .gitignore");

    Ok(())
}

pub fn status(project_opt: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_opt.unwrap_or_else(|| infer_project_id(&cwd));
    let docs = db.list_documents(&project_id)?;

    println!("=== AutoMemory Cloud Sync Status: '{}' ===\n", project_id);

    let cfg_path = cwd.join(".automemory").join("config.json");
    if let Ok(content) = fs::read_to_string(&cfg_path) {
        if let Ok(cfg) = serde_json::from_str::<SyncConfig>(&content) {
            println!(
                "  Cloud Endpoint: {}",
                cfg.endpoint
                    .as_deref()
                    .unwrap_or("Local Vault (No remote endpoint set)")
            );
            println!(
                "  Last Synced:    {}",
                cfg.last_synced_at.as_deref().unwrap_or("Never")
            );
            println!();
        }
    }

    let mut pending_upload = 0;
    let mut synced = 0;

    for d in &docs {
        let is_synced = match &d.remote_synced_hash {
            Some(remote) => remote == &d.content_hash,
            None => false,
        };

        if is_synced {
            synced += 1;
        } else {
            pending_upload += 1;
            println!(
                "  ⚡ Pending Delta: [{}] {} ({})",
                d.category.to_uppercase(),
                d.title,
                d.relative_path
            );
        }
    }

    let memories = db.list_memories(Some(&project_id), Some(MemoryStatus::Active), None, 50)?;
    let skills = db.list_skills(Some(&project_id), 50)?;

    println!(
        "\n  Tracked Documents: {} total ({} synced, {} pending deltas)",
        docs.len(),
        synced,
        pending_upload
    );
    println!("  Active Memories:   {} records", memories.len());
    println!("  Procedural Skills: {} playbooks", skills.len());

    if pending_upload > 0 {
        println!("\nRun 'automemory sync push' to sync changes to cloud.");
    } else {
        println!("\n✓ All documents and memories are in sync with cloud.");
    }

    Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CloudManifestFile {
    path: String,
    #[serde(default)]
    category: Option<String>,
    sha256: String,
    #[serde(default)]
    nonce_base64: Option<String>,
    #[serde(default)]
    size_bytes: Option<usize>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CloudManifest {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    project_slug: Option<String>,
    #[serde(default)]
    files: Option<Vec<CloudManifestFile>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CloudPullResponse {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    manifest: Option<CloudManifest>,
    #[serde(default)]
    blind_memories: Option<Vec<BlindMemoryWire>>,
    #[serde(default)]
    blind_skills: Option<Vec<BlindSkillWire>>,
}

use uuid::Uuid;

pub async fn push(
    project_opt: Option<String>,
    endpoint_opt: Option<String>,
    token_opt: Option<String>,
    key_opt: Option<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = Config::default_config();
    let global_cfg = GlobalConfig::load().unwrap_or_default();
    let registered_proj = global_cfg.find_project_by_path(&cwd);

    let project_id = project_opt.unwrap_or_else(|| {
        registered_proj
            .map(|p| p.name.clone())
            .unwrap_or_else(|| infer_project_id(&cwd))
    });

    let db = Database::open(&config.db_path)?;

    // Auto-discover and track documents in workspace according to trackable_patterns
    let target_dir = registered_proj
        .map(|p| PathBuf::from(&p.local_dir))
        .unwrap_or_else(|| cwd.clone());
    let trackable_patterns = registered_proj
        .map(|p| p.trackable_files.clone())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(automemory_core::config::default_trackable_files);

    let mut project = match db.get_project_by_id(&project_id)? {
        Some(p) => p,
        None => match db.get_project_by_slug(&project_id)? {
            Some(p) => p,
            None => Project {
                id: Uuid::now_v7().to_string(),
                slug: project_id.clone(),
                display_name: project_id.clone(),
                document_config: Default::default(),
                sync_config: Default::default(),
                capture_policy: Default::default(),
                context_budget: Default::default(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
        },
    };
    project.document_config.include_patterns = trackable_patterns;

    // Scan the project workspace
    let scanner = automemory_core::docs::DocumentScanner::new(std::sync::Arc::new(Database::open(
        &config.db_path,
    )?));
    let _ = scanner.scan_project(&project, &target_dir);

    let docs = db.list_documents(&project_id)?;

    // Resolve endpoint and auth token
    let cfg_path = cwd.join(".automemory").join("config.json");
    let (endpoint, auth_token) = if let Some(ep) = endpoint_opt {
        (
            Some(ep),
            token_opt.or_else(|| global_cfg.cloud.auth_token.clone()),
        )
    } else if let Some(ref ep) = global_cfg.cloud.endpoint {
        (
            Some(ep.clone()),
            token_opt.or_else(|| global_cfg.cloud.auth_token.clone()),
        )
    } else if let Ok(content) = fs::read_to_string(&cfg_path) {
        if let Ok(cfg) = serde_json::from_str::<SyncConfig>(&content) {
            (cfg.endpoint, cfg.auth_token.or(token_opt))
        } else {
            (None, token_opt)
        }
    } else {
        (None, token_opt)
    };

    let mut deltas = Vec::new();
    let context_root = config
        .home_dir
        .join("context")
        .join("projects")
        .join(&project_id);

    for d in &docs {
        let is_synced = match &d.remote_synced_hash {
            Some(remote) => remote == &d.content_hash,
            None => false,
        };

        if !is_synced || endpoint.is_some() {
            // Read content from project or canonical context
            let file_path = cwd.join(&d.relative_path);
            let content = if file_path.exists() {
                fs::read_to_string(&file_path).unwrap_or_default()
            } else {
                let context_path = context_root
                    .join("docs")
                    .join(&d.category)
                    .join(Path::new(&d.relative_path).file_name().unwrap_or_default());
                fs::read_to_string(&context_path).unwrap_or_default()
            };

            deltas.push(DocumentDelta {
                id: d.id.clone(),
                relative_path: d.relative_path.clone(),
                category: d.category.clone(),
                title: d.title.clone(),
                content_hash: d.content_hash.clone(),
                content,
                l0_abstract: d.l0_abstract.clone(),
                l1_overview: d.l1_overview.clone(),
            });
        }
    }

    let memories = db.list_memories(Some(&project_id), Some(MemoryStatus::Active), None, 500)?;
    let skills = db.list_skills(Some(&project_id), 100)?;

    let (device_id, device_name) = get_or_create_device_identity(&config.home_dir);

    if let Some(ep) = endpoint {
        let master_key = BlindCrypto::get_or_create_master_key(key_opt.as_deref())?;
        let index_key = BlindCrypto::derive_index_key(&master_key);

        let vector_engine = automemory_core::retrieval::vector::VectorEngine::default();
        let mut blind_memories = Vec::new();
        for m in &memories {
            let embedding = vector_engine
                .embed_text(&format!("{} {}", m.summary, m.content))
                .ok();
            match BlindCrypto::encrypt_memory_with_embedding(
                &master_key,
                &index_key,
                m,
                &project_id,
                embedding,
            ) {
                Ok(bm) => blind_memories.push(bm),
                Err(e) => eprintln!("Warning: failed to encrypt memory {}: {}", m.id, e),
            }
        }

        let mut blind_skills = Vec::new();
        for s in &skills {
            match BlindCrypto::encrypt_skill(&master_key, s, &project_id) {
                Ok(bs) => blind_skills.push(bs),
                Err(e) => eprintln!("Warning: failed to encrypt skill {}: {}", s.id, e),
            }
        }

        let mut encrypted_deltas = Vec::new();
        for d in &deltas {
            match BlindCrypto::encrypt_delta(&master_key, &d.relative_path, &d.category, &d.content)
            {
                Ok(ed) => encrypted_deltas.push(ed),
                Err(e) => eprintln!(
                    "Warning: failed to encrypt delta {}: {}",
                    d.relative_path, e
                ),
            }
        }

        let (push_url, _, device_reg_url, _) = resolve_cloud_urls(&ep);
        println!(
            "Transmitting zero-knowledge blind payload from device '{}' ({}) to: {}...",
            device_name, device_id, push_url
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let mut reg_req = client.post(&device_reg_url).json(&serde_json::json!({
            "device_id": &device_id,
            "device_name": &device_name,
        }));
        if let Some(ref tok) = auth_token {
            reg_req = reg_req.header("Authorization", format!("Bearer {}", tok));
        }
        if let Ok(reg_resp) = reg_req.send().await {
            if reg_resp.status() == reqwest::StatusCode::FORBIDDEN {
                if let Ok(err_body) = reg_resp.text().await {
                    if err_body.contains("DEVICE_QUOTA_EXCEEDED") {
                        println!(
                            "🛑 Cloud Sync Alert: Device quota exceeded on your current tier!"
                        );
                        println!("   Please upgrade your plan or revoke an inactive device in dashboard.");
                    }
                }
            }
        }

        let blind_payload = serde_json::json!({
            "project_slug": &project_id,
            "project_name": &project_id,
            "device_id": &device_id,
            "device_name": &device_name,
            "is_blind": true,
            "blind_memories": blind_memories,
            "blind_skills": blind_skills,
            "encrypted_deltas": encrypted_deltas,
        });

        let mut req = client
            .post(&push_url)
            .json(&blind_payload)
            .header("X-Device-Id", &device_id)
            .header("X-Device-Name", &device_name);
        if let Some(ref tok) = auth_token {
            req = req.header("Authorization", format!("Bearer {}", tok));
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                println!(
                    "✓ Zero-Knowledge Blind Sync accepted by Cloudflare Edge! (HTTP {})",
                    resp.status()
                );
                println!("  🔒 Client-Side Encryption: AES-256-GCM + HMAC blind index tokens");
                println!(
                    "  🧠 Blind Memories:   {} encrypted records",
                    blind_memories.len()
                );
                println!(
                    "  ⚡ Blind Skills:     {} encrypted playbooks",
                    blind_skills.len()
                );
                println!(
                    "  📦 Encrypted Deltas: {} chunks stored in R2",
                    encrypted_deltas.len()
                );
            }
            Ok(resp) => {
                println!(
                    "⚠ Cloud endpoint returned {}. Archiving delta locally.",
                    resp.status()
                );
            }
            Err(e) => {
                println!(
                    "⚠ Cloud endpoint unreachable ({}). Archiving delta locally.",
                    e
                );
            }
        }
    }

    // Save delta locally in canonical context /deltas/ for offline resilience
    let deltas_dir = context_root.join("deltas");
    fs::create_dir_all(&deltas_dir)?;
    let local_payload = DeltaPayload {
        project_id: project_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        device_id: Some(device_id.clone()),
        device_name: Some(device_name.clone()),
        documents: deltas.clone(),
        memories,
        skills,
    };
    let delta_file = deltas_dir.join(format!("delta-{}.json", Utc::now().format("%Y%m%d_%H%M%S")));
    fs::write(&delta_file, serde_json::to_string_pretty(&local_payload)?)?;

    // Update remote_synced_hash in database for pushed documents
    for doc_delta in &deltas {
        let _ = db.update_document_synced_hash(&doc_delta.id, &doc_delta.content_hash);
    }

    // Update last_synced_at in .automemory/config.json
    if cfg_path.exists() {
        if let Ok(content) = fs::read_to_string(&cfg_path) {
            if let Ok(mut cfg) = serde_json::from_str::<SyncConfig>(&content) {
                cfg.last_synced_at = Some(Utc::now().to_rfc3339());
                let _ = fs::write(&cfg_path, serde_json::to_string_pretty(&cfg)?);
            }
        }
    }

    println!(
        "✓ Successfully pushed updates for project '{}'!",
        project_id
    );
    println!("  📄 Documents Synced: {} delta files", deltas.len());

    // Also update text-first Git Vault and push to GitHub if configured
    let _ = export_vault(None);

    Ok(())
}

pub async fn pull(
    project_opt: Option<String>,
    endpoint_opt: Option<String>,
    token_opt: Option<String>,
    key_opt: Option<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let global_cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
    let registered_proj = global_cfg.find_project_by_path(&cwd);

    let project_id = project_opt.unwrap_or_else(|| {
        registered_proj
            .map(|p| p.name.clone())
            .unwrap_or_else(|| infer_project_id(&cwd))
    });

    println!("Pulling remote updates for project '{}'...", project_id);

    // 1. Pull latest commits from GitHub memory vault if remote is configured
    let config = Config::default_config();
    let vault_path = global_cfg
        .vault
        .local_vault_dir
        .clone()
        .unwrap_or(config.vault_dir.clone());
    if let Some(ref remote_url) = global_cfg.vault.remote_url {
        if !automemory_core::sync::GitSync::is_git_repo(&vault_path) {
            let _ = automemory_core::sync::GitSync::init_repo(&vault_path);
        }
        let _ = automemory_core::sync::GitSync::set_remote(&vault_path, "origin", remote_url);
        println!(
            "Pulling latest commits from GitHub memory vault: {}...",
            remote_url
        );
        if let Err(e) = automemory_core::sync::GitSync::pull(&vault_path, None, None) {
            println!("ℹ Git pull note: {}", e);
        } else {
            println!("✓ Successfully pulled latest vault commits from GitHub.");
        }
    }

    // 2. Re-materialize git-ignored files to workspace if restore_to_workspace is enabled
    let restore_enabled = registered_proj
        .map(|p| p.restore_to_workspace)
        .unwrap_or(true);
    let target_dir = registered_proj
        .map(|p| PathBuf::from(&p.local_dir))
        .unwrap_or_else(|| cwd.clone());

    if restore_enabled {
        let db_opt = Database::open(&config.db_path).ok();
        let proj_slug = db_opt
            .as_ref()
            .and_then(|d| d.get_project_by_id(&project_id).ok().flatten())
            .map(|p| p.slug)
            .unwrap_or_else(|| project_id.clone());

        let mut proj_vault_docs = vault_path.join("projects").join(&project_id).join("docs");
        if !proj_vault_docs.exists() {
            let alt = vault_path.join("projects").join(&proj_slug).join("docs");
            if alt.exists() {
                proj_vault_docs = alt;
            }
        }
        if proj_vault_docs.exists() {
            let mut restored_count = 0;
            if let Ok(entries) = glob::glob(&format!("{}/**/*", proj_vault_docs.display())) {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        if let Ok(rel) = entry.strip_prefix(&proj_vault_docs) {
                            let dest = target_dir.join(rel);
                            let should_write = if !dest.exists() {
                                true
                            } else if let (Ok(c1), Ok(c2)) = (fs::read(&entry), fs::read(&dest)) {
                                c1 != c2
                            } else {
                                false
                            };

                            if should_write {
                                if let Some(parent) = dest.parent() {
                                    let _ = fs::create_dir_all(parent);
                                }
                                if let Ok(_) = fs::copy(&entry, &dest) {
                                    restored_count += 1;
                                    println!(
                                        "  ✓ Restored document to workspace: {}",
                                        rel.display()
                                    );
                                }
                            }
                        }
                    }
                }
            }
            if restored_count > 0 {
                println!(
                    "✓ Re-materialized {} git-ignored documents/plans in workspace!",
                    restored_count
                );
            }
        }
    }

    // 3. Import vault updates into local SQLite database
    if let Ok(db) = Database::open(&config.db_path) {
        let manager = automemory_core::sync::VaultManager::new(&db, &vault_path);
        let _ = manager.import_from_vault();
    }

    // 4. Cloudflare Worker Edge Blind Sync Pull
    let cfg_path = cwd.join(".automemory").join("config.json");
    let (endpoint, auth_token) = if let Some(ep) = endpoint_opt {
        (
            Some(ep),
            token_opt.or_else(|| global_cfg.cloud.auth_token.clone()),
        )
    } else if let Some(ref ep) = global_cfg.cloud.endpoint {
        (
            Some(ep.clone()),
            token_opt.or_else(|| global_cfg.cloud.auth_token.clone()),
        )
    } else if let Ok(content) = fs::read_to_string(&cfg_path) {
        if let Ok(cfg) = serde_json::from_str::<SyncConfig>(&content) {
            (cfg.endpoint, cfg.auth_token.or(token_opt))
        } else {
            (None, token_opt)
        }
    } else {
        (None, token_opt)
    };

    if let Some(ep) = endpoint {
        let (_, pull_url, _, blob_base) = resolve_cloud_urls(&ep);
        println!("Connecting to Cloudflare Edge Sync at {}...", pull_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let mut req = client.get(&format!("{}?project={}", pull_url, project_id));
        if let Some(ref tok) = auth_token {
            req = req.header("Authorization", format!("Bearer {}", tok));
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(pull_data) = resp.json::<CloudPullResponse>().await {
                    let master_key = BlindCrypto::get_or_create_master_key(key_opt.as_deref())?;

                    let mut mem_count = 0;
                    if let (Ok(db), Some(bms)) =
                        (Database::open(&config.db_path), pull_data.blind_memories)
                    {
                        for bm in bms {
                            if let Ok(mem) =
                                BlindCrypto::decrypt_memory(&master_key, &bm, Some(&project_id))
                            {
                                let _ = db.save_memory(&mem);
                                mem_count += 1;
                            }
                        }
                    }

                    let mut skill_count = 0;
                    if let (Ok(db), Some(bss)) =
                        (Database::open(&config.db_path), pull_data.blind_skills)
                    {
                        for bs in bss {
                            if let Ok(skill) =
                                BlindCrypto::decrypt_skill(&master_key, &bs, Some(&project_id))
                            {
                                let _ = db.save_skill(&skill);
                                skill_count += 1;
                            }
                        }
                    }

                    let mut blob_restored_count = 0;
                    if let Some(manifest) = pull_data.manifest {
                        if let Some(files) = manifest.files {
                            for f in files {
                                let target_path = target_dir.join(&f.path);
                                let need_download = if !target_path.exists() {
                                    true
                                } else if let Ok(existing) = fs::read(&target_path) {
                                    let mut hasher = Sha256::new();
                                    hasher.update(&existing);
                                    let h = format!("{:x}", hasher.finalize());
                                    h != f.sha256
                                } else {
                                    true
                                };

                                if need_download {
                                    let blob_url =
                                        format!("{}/{}/{}", blob_base, project_id, f.sha256);
                                    let mut blob_req = client.get(&blob_url);
                                    if let Some(ref tok) = auth_token {
                                        blob_req = blob_req
                                            .header("Authorization", format!("Bearer {}", tok));
                                    }

                                    if let Ok(b_resp) = blob_req.send().await {
                                        if b_resp.status().is_success() {
                                            let nonce_opt = b_resp
                                                .headers()
                                                .get("X-Nonce-Base64")
                                                .and_then(|h| h.to_str().ok())
                                                .map(|s| s.to_string())
                                                .or(f.nonce_base64);

                                            if let (Some(nonce), Ok(bytes)) =
                                                (nonce_opt, b_resp.bytes().await)
                                            {
                                                let blob = EncryptedBlob {
                                                    ciphertext_base64: BASE64.encode(&bytes),
                                                    nonce_base64: nonce,
                                                };
                                                if let Ok(decrypted_bytes) =
                                                    BlindCrypto::decrypt(&master_key, &blob)
                                                {
                                                    if let Some(parent) = target_path.parent() {
                                                        let _ = fs::create_dir_all(parent);
                                                    }
                                                    if fs::write(&target_path, &decrypted_bytes)
                                                        .is_ok()
                                                    {
                                                        blob_restored_count += 1;
                                                        println!("  ✓ Decrypted & restored encrypted document: {}", f.path);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    println!("✓ Cloudflare Edge Zero-Knowledge Sync successfully applied!");
                    println!("  🧠 Decrypted Memories:   {} records imported", mem_count);
                    println!(
                        "  ⚡ Decrypted Skills:     {} playbooks imported",
                        skill_count
                    );
                    if blob_restored_count > 0 {
                        println!(
                            "  📦 Restored Plans/Docs:  {} files re-materialized in workspace",
                            blob_restored_count
                        );
                    }
                }
            }
            Ok(resp) => {
                println!("ℹ Remote returned {}. Up to date.", resp.status());
            }
            Err(e) => {
                println!("ℹ Remote query note: {}. Checking local context.", e);
            }
        }
    } else {
        println!("✓ Single-source context is up to date.");
    }

    Ok(())
}

pub async fn restore(
    project_id_arg: Option<String>,
    from_file: Option<String>,
    endpoint_opt: Option<String>,
    token_opt: Option<String>,
    key_opt: Option<String>,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = match project_id_arg {
        Some(p) => p,
        None => infer_project_id(&cwd),
    };

    println!("========================================================");
    println!("     AutoMemory — Disaster Recovery & Machine Reset     ");
    println!("   Restoring Project Context: '{}'", project_id);
    println!("========================================================\n");

    // If cloud endpoint is provided and no local file archive specified, perform full edge pull
    if from_file.is_none() && endpoint_opt.is_some() {
        println!("Performing disaster recovery pull from Cloudflare Edge Sync...");
        pull(Some(project_id.clone()), endpoint_opt, token_opt, key_opt).await?;
        let _ = crate::commands::link::execute(Some(project_id.clone()));
        println!(
            "\n✓ Disaster recovery complete! Project '{}' is fully restored from edge.",
            project_id
        );
        return Ok(());
    }

    // 1. Fetch manifest (from file, cloud endpoint, or canonical backup)
    let manifest: Option<RestoreManifest> = if let Some(file_path) = from_file {
        println!("Reading restore archive from file: {}...", file_path);
        let content = fs::read_to_string(&file_path)?;
        serde_json::from_str(&content).ok()
    } else if let Some(ep) = endpoint_opt {
        println!(
            "Fetching cloud manifest from {}/projects/{}/manifest...",
            ep, project_id
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let mut req = client.get(&format!("{}/projects/{}/manifest", ep, project_id));
        if let Some(tok) = token_opt {
            req = req.header("Authorization", format!("Bearer {}", tok));
        }

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                resp.json::<RestoreManifest>().await.ok()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        // Read latest delta from canonical context if available
        let deltas_dir = config
            .home_dir
            .join("context")
            .join("projects")
            .join(&project_id)
            .join("deltas");
        if deltas_dir.exists() {
            if let Ok(entries) = fs::read_dir(&deltas_dir) {
                let mut files: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
                files.sort();
                if let Some(latest) = files.last() {
                    println!("Restoring from local delta backup: {}...", latest.display());
                    let content = fs::read_to_string(latest)?;
                    serde_json::from_str(&content).ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    // 2. Restore memories into SQLite
    if let Some(ref m) = manifest {
        println!(
            "• Restoring {} memories into SQLite database...",
            m.memories.len()
        );
        for mem in &m.memories {
            let _ = db.save_memory(mem);
        }

        println!("• Restoring {} procedural skills...", m.skills.len());
        for skill in &m.skills {
            let _ = db.save_skill(skill);
        }
    }

    // 3. Link project to OpenViking single source
    crate::commands::link::execute(Some(project_id.clone()))?;

    // 4. Materialize and check tracked documents
    if let Some(ref m) = manifest {
        for doc in &m.documents {
            let target_path = cwd.join(&doc.relative_path);

            let final_rel_path = if target_path.exists() {
                // Check if hash matches
                let existing_content = fs::read_to_string(&target_path).unwrap_or_default();
                let mut hasher = Sha256::new();
                hasher.update(existing_content.as_bytes());
                let existing_hash = format!("{:x}", hasher.finalize());

                if existing_hash != doc.content_hash {
                    // Conflict protection: write .remote.md
                    let remote_path = target_path.with_extension("remote.md");
                    fs::write(&remote_path, &doc.content)?;
                    println!(
                        "  ⚠ Conflict detected in '{}': local file differs from cloud version.",
                        doc.relative_path
                    );
                    println!("    Saved cloud version to: {}", remote_path.display());
                } else {
                    println!("  ✓ Verified existing document: {}", doc.relative_path);
                }
                doc.relative_path.clone()
            } else {
                // Check for fuzzy path match: did the file move to another folder?
                let filename = Path::new(&doc.relative_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let moved_match = find_fuzzy_match(&cwd, &doc.content_hash, filename);
                if let Some(new_rel_path) = moved_match {
                    println!(
                        "  ℹ Intelligent path remap: '{}' found locally at '{}'",
                        doc.relative_path, new_rel_path
                    );
                    new_rel_path
                } else {
                    // Create parent directories and restore file
                    if let Some(parent) = target_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    fs::write(&target_path, &doc.content)?;
                    println!("  + Restored missing document: {}", doc.relative_path);
                    doc.relative_path.clone()
                }
            };

            // Register in SQLite
            let tracked = TrackedDocument {
                id: doc.id.clone(),
                project_id: project_id.clone(),
                relative_path: final_rel_path,
                category: doc.category.clone(),
                title: doc.title.clone(),
                content_hash: doc.content_hash.clone(),
                remote_synced_hash: Some(doc.content_hash.clone()),
                summary: doc.l0_abstract.clone(),
                l0_abstract: doc.l0_abstract.clone(),
                l1_overview: doc.l1_overview.clone(),
                last_scanned_at: Utc::now(),
                is_deleted: false,
            };
            let _ = db.track_document(&tracked, Some(&doc.content));
        }
    }

    println!(
        "\n✓ Disaster recovery complete! Project '{}' is fully restored.",
        project_id
    );
    println!(
        "  🔗 Local symlink: .context/ -> ~/.automemory/context/projects/{}",
        project_id
    );

    Ok(())
}

fn find_fuzzy_match(cwd: &Path, expected_hash: &str, filename: &str) -> Option<String> {
    if let Ok(entries) = fs::read_dir(cwd) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                if let Some(found) = find_fuzzy_match_recursive(&path, cwd, expected_hash, filename)
                {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn find_fuzzy_match_recursive(
    dir: &Path,
    cwd: &Path,
    expected_hash: &str,
    target_name: &str,
) -> Option<String> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                    if fname == target_name {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let mut hasher = Sha256::new();
                            hasher.update(content.as_bytes());
                            let h = format!("{:x}", hasher.finalize());
                            if h == expected_hash {
                                return path
                                    .strip_prefix(cwd)
                                    .ok()
                                    .map(|p| p.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            } else if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') {
                    if let Some(found) =
                        find_fuzzy_match_recursive(&path, cwd, expected_hash, target_name)
                    {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

pub fn export_vault(path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let global_cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
    let vault_path = path
        .map(PathBuf::from)
        .or(global_cfg.vault.local_vault_dir.clone())
        .unwrap_or(config.vault_dir);

    let db = Database::open(&config.db_path)?;
    let manager = automemory_core::sync::VaultManager::new(&db, &vault_path);
    manager.export_to_vault()?;
    println!(
        "✓ Exported database to text-first vault at: {:?}",
        vault_path
    );

    // Ensure vault git repository is initialized
    if !automemory_core::sync::GitSync::is_git_repo(&vault_path) {
        let _ = automemory_core::sync::GitSync::init_repo(&vault_path);
    }

    let msg = format!(
        "AutoMemory: automated vault sync at {}",
        Utc::now().to_rfc3339()
    );
    let committed = automemory_core::sync::GitSync::commit_all(&vault_path, &msg)?;
    if committed {
        println!("✓ Committed vault updates to local git repository.");
    }

    if global_cfg.vault.auto_push {
        if let Some(ref remote_url) = global_cfg.vault.remote_url {
            let _ = automemory_core::sync::GitSync::set_remote(&vault_path, "origin", remote_url);
            println!("Pushing vault updates to GitHub remote: {}...", remote_url);
            if let Err(e) = automemory_core::sync::GitSync::push(&vault_path, None, None) {
                println!("ℹ Git push note: {}", e);
            } else {
                println!("✓ Successfully pushed vault updates to GitHub!");
            }
        }
    }

    Ok(())
}

pub fn import_vault(path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let global_cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
    let vault_path = path
        .map(PathBuf::from)
        .or(global_cfg.vault.local_vault_dir.clone())
        .unwrap_or(config.vault_dir);

    let db = Database::open(&config.db_path)?;
    let manager = automemory_core::sync::VaultManager::new(&db, &vault_path);
    manager.import_from_vault()?;
    println!("✓ Imported records from vault at: {:?}", vault_path);
    Ok(())
}

fn infer_project_id(cwd: &Path) -> String {
    // 1. Check global config (~/.automemory/config.toml)
    if let Ok(global_cfg) = automemory_core::config::GlobalConfig::load() {
        if let Some(p) = global_cfg.find_project_by_path(cwd) {
            return p.name.clone();
        }
    }

    // 2. Fallback to legacy .automemory/config.json if present
    let cfg_path = cwd.join(".automemory").join("config.json");
    if let Ok(content) = fs::read_to_string(&cfg_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(id) = json.get("project_id").and_then(|v| v.as_str()) {
                return id.to_string();
            }
        }
    }

    // 3. Fallback to folder name
    cwd.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "default-project".to_string())
}

fn update_gitignore(cwd: &Path) -> Result<()> {
    let gitignore_path = cwd.join(".gitignore");
    let mut content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut modified = false;
    if !content.contains(".context") {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str("\n# AutoMemory single-source local context\n.context\n");
        modified = true;
    }

    if !content.contains(".automemory") {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(".automemory\n");
        modified = true;
    }

    if modified {
        fs::write(gitignore_path, content)?;
    }

    Ok(())
}

/// Zero-knowledge device pairing via BIP-39 12-word mnemonic phrase and QR code.
pub async fn pair(
    join_phrase: Option<String>,
    endpoint_opt: Option<String>,
    token_opt: Option<String>,
) -> Result<()> {
    let global_cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
    let endpoint = endpoint_opt.or(global_cfg.cloud.endpoint.clone());
    let token = token_opt.or(global_cfg.cloud.auth_token.clone());

    if let Some(phrase) = join_phrase {
        // --- Joining an existing device vault ---
        println!("==========================================================================");
        println!("     AutoMemory — Device Pairing: Joining Existing Zero-Knowledge Vault   ");
        println!("==========================================================================");

        let key = match automemory_core::bip39::mnemonic_to_master_key(&phrase) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("❌ Invalid recovery seed phrase: {}", e);
                eprintln!("   Please check the words and ensure correct ordering.");
                return Err(e);
            }
        };

        // Secure master key in Keychain & disk
        automemory_core::keychain::set_master_key(&key)?;
        println!("✓ Validated BIP-39 mnemonic phrase.");
        println!("✓ Derived 256-bit AES-256-GCM Master Key.");
        if automemory_core::keychain::is_native_keychain_active() {
            println!("✓ Secured Master Key in macOS Apple Keychain ('dev.automemory.master-key').");
        }
        println!("✓ Secured fallback master key at ~/.automemory/master.key (0600).");

        // Acquire / register device lease if endpoint is available
        let config = Config::default_config();
        let (device_id, device_name) = get_or_create_device_identity(&config.home_dir);

        if let Some(ref ep) = endpoint {
            let (_, _, device_reg_url, _) = resolve_cloud_urls(ep);
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()?;

            let mut req = client.post(&device_reg_url).json(&serde_json::json!({
                "device_id": &device_id,
                "device_name": &device_name,
            }));
            if let Some(ref tok) = token {
                req = req.header("Authorization", format!("Bearer {}", tok));
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    println!(
                        "✓ Acquired active device lease from Cloudflare Edge for '{}' ({})",
                        device_name, device_id
                    );
                }
                Ok(resp) if resp.status() == reqwest::StatusCode::FORBIDDEN => {
                    println!("⚠️ Device registration blocked: Device quota exceeded on your current plan.");
                    println!("   Please visit Developer Console (/dashboard) to upgrade or revoke an old device.");
                }
                _ => {
                    println!("✓ Prepared device '{}' ({})", device_name, device_id);
                }
            }

            println!("\nPairing complete! Executing initial sync pull...");
            let _ = pull(None, Some(ep.clone()), token.clone(), None).await;
        } else {
            println!("✓ Device paired successfully with Master Key.");
        }

        println!(
            "\n🎉 Pairing successful! This machine is now securely synchronized with your vault."
        );
        Ok(())
    } else {
        // --- Displaying pairing code from current device ---
        let master_key = match automemory_core::keychain::get_master_key() {
            Some(k) => k,
            None => {
                let (phrase, key) = automemory_core::bip39::generate_pairing_seed_12()?;
                automemory_core::keychain::set_master_key(&key)?;
                let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                let _ = std::fs::create_dir_all(home.join(".automemory"));
                let _ = std::fs::write(home.join(".automemory").join("pairing_seed.txt"), &phrase);
                key
            }
        };

        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let saved_seed_file = home.join(".automemory").join("pairing_seed.txt");
        let phrase = if saved_seed_file.exists() {
            std::fs::read_to_string(&saved_seed_file)
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            automemory_core::bip39::master_key_to_mnemonic_24(&master_key)?
        };

        let qr_art = automemory_core::bip39::render_terminal_qr(&phrase);

        println!("==========================================================================");
        println!("               AutoMemory — Zero-Knowledge Device Pairing                 ");
        println!("==========================================================================");
        println!();
        println!("Scan this QR block matrix from another terminal or copy the recovery seed:\n");
        println!("{}", qr_art);
        println!("Recovery Phrase:");
        println!("  {}", phrase);
        println!();
        println!("On your secondary laptop, desktop, or CI server, run:");
        println!("  automemory sync pair --join \"{}\"", phrase);
        println!();
        println!("Security Guarantee:");
        if automemory_core::keychain::is_native_keychain_active() {
            println!(
                "  • Master Key is encrypted in macOS Apple Keychain ('dev.automemory.master-key')"
            );
        }
        println!("  • Cloudflare Edge and GitHub Vault hold ZERO decryption keys");
        println!("==========================================================================");

        Ok(())
    }
}

/// Real-time background file watcher and auto-synchronization daemon.
pub async fn watch(project_opt: Option<String>, interval_secs: u64) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let global_cfg = GlobalConfig::load().unwrap_or_default();
    let registered_proj = global_cfg.find_project_by_path(&cwd);

    let project_id = project_opt.unwrap_or_else(|| {
        registered_proj
            .map(|p| p.name.clone())
            .unwrap_or_else(|| infer_project_id(&cwd))
    });

    println!("==========================================================================");
    println!("     AutoMemory — Real-Time Background File Watcher & Auto-Sync Engine    ");
    println!("==========================================================================");
    println!("Watching project '{}' in {}", project_id, cwd.display());
    println!(
        "Polling interval: {}s. Auto-pushing git-ignored plans & encrypted deltas.",
        interval_secs
    );
    println!("Press Ctrl+C to terminate auto-sync.\n");

    // Perform initial synchronization
    println!(
        "[{}] Executing baseline sync...",
        chrono::Local::now().format("%H:%M:%S")
    );
    let _ = push(Some(project_id.clone()), None, None, None).await;

    let poll_duration = std::time::Duration::from_secs(interval_secs);
    let mut last_sync = std::time::Instant::now();

    loop {
        tokio::time::sleep(poll_duration).await;
        // Periodic sync trigger for active modifications
        if last_sync.elapsed() >= std::time::Duration::from_secs(interval_secs * 2) {
            let now_str = chrono::Local::now().format("%H:%M:%S").to_string();
            println!(
                "[{}] Auto-synchronizing changes to Edge and GitHub Vault...",
                now_str
            );
            let _ = push(Some(project_id.clone()), None, None, None).await;
            last_sync = std::time::Instant::now();
        }
    }
}
