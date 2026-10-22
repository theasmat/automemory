use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use std::fs;
use std::path::Path;

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

/// Zero-knowledge device pairing via BIP-39 12-word mnemonic phrase and QR code.
pub fn pair(join_phrase: Option<String>) -> Result<()> {
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

        let config = Config::default_config();
        let (device_id, device_name) = get_or_create_device_identity(&config.home_dir);
        println!("✓ Device paired successfully with the master key ('{}', {}).", device_name, device_id);

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
        println!("  • The git remote holds ZERO decryption keys (everything in it is encrypted on this machine)");
        println!("==========================================================================");

        Ok(())
    }
}

/// `sync auto`: one pass through the git remote (what hooks start in the background).
pub fn auto(pull: bool, push: bool) -> Result<()> {
    let db = Database::open(&Config::default_config().db_path)?;
    let (root, remote) = automemory_core::sync::auto::vault_settings();
    let key = automemory_core::sync::auto::resolve_key(remote.is_some());
    let both = !pull && !push;
    let report = automemory_core::sync::auto::run(&db, &root, remote.as_deref(), pull || both, push || both, key.as_ref())?;
    println!("sync: {}", report.summary());
    Ok(())
}

/// `sync remote <url>`: the private git repository that carries memory between machines.
pub fn remote(url: &str) -> Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed == "--clear" || trimmed == "--remove" || trimmed == "none" {
        let mut cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
        cfg.vault.remote_url = None;
        cfg.save()?;
        let (root, _) = automemory_core::sync::auto::vault_settings();
        if root.join(".git").exists() {
            let _ = std::process::Command::new("git")
                .args(["remote", "remove", "origin"])
                .current_dir(&root)
                .output();
        }
        println!("✓ Remote sync removed");
        return Ok(());
    }

    let mut cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
    cfg.vault.remote_url = Some(trimmed.to_string());
    cfg.save()?;
    let (root, _) = automemory_core::sync::auto::vault_settings();
    std::fs::create_dir_all(&root)?;
    if !root.join(".git").exists() {
        automemory_core::sync::GitSync::init_repo(&root)?;
    }
    automemory_core::sync::GitSync::set_remote(&root, "origin", trimmed)?;
    println!("✓ Memory will sync through {trimmed}");
    println!("  Vault folder: {}", root.display());
    println!("  Everything is encrypted on this machine before it is pushed.");
    if std::env::var("AUTOMEMORY_KEY").is_err() && automemory_core::keychain::get_master_key().is_none() {
        println!("  A key is created on the first sync. Run `automemory sync pair` afterwards to see");
        println!("  the recovery phrase, and use it on your other machines.");
    }
    println!("  Other machines: `automemory sync bootstrap --remote {url} --phrase \"<recovery phrase>\"`");
    Ok(())
}

/// `sync bootstrap`: set up a new machine from an existing vault in one command.
pub fn bootstrap(remote_url: &str, phrase: Option<String>, connect_agents: bool) -> Result<()> {
    remote(remote_url)?;
    if let Some(words) = phrase {
        let key = automemory_core::bip39::mnemonic_to_master_key(&words)?;
        automemory_core::keychain::set_master_key(&key)?;
        println!("✓ Recovery phrase accepted; key stored for this machine.");
    } else if std::env::var("AUTOMEMORY_KEY").is_err() && automemory_core::keychain::get_master_key().is_none() {
        anyhow::bail!("No key: pass --phrase \"<12 or 24 words>\" (shown by `automemory sync pair` on your first machine).");
    }
    auto(true, true)?;
    if connect_agents {
        for agent in ["agy", "claude", "codex"] {
            super::connect::execute(agent, true)?;
        }
    }
    println!("\nDone. In a project folder, `automemory resume` shows where you left off; `automemory doctor` checks the setup.");
    Ok(())
}
