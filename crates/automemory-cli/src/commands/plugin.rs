use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::ports::{PluginManifest, PORTS};

/// Lists the 16 versioned ports with their status and default, plus any
/// plugin manifests found in `~/.automemory/plugins/*.toml` (validated).
pub fn list() -> Result<()> {
    println!("Ports ({}):", PORTS.len());
    for p in PORTS {
        println!(
            "  {:<16} v{:<4} {:<8} {}",
            p.id,
            p.api_version,
            match p.status {
                automemory_core::ports::PortStatus::Native => "native",
                automemory_core::ports::PortStatus::Concrete => "concrete",
                automemory_core::ports::PortStatus::Deferred => "deferred",
            },
            p.default_impl
        );
    }

    let config = Config::default_config();
    let dir = config.home_dir.join("plugins");
    let mut found = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            found += 1;
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| PluginManifest::from_toml_str(&text).map_err(|e| e.to_string()))
            {
                Ok(manifest) => match manifest.validate() {
                    Ok(_) => println!(
                        "  plugin {} v{} ports=[{}] ({})",
                        manifest.name,
                        manifest.version,
                        manifest.ports.join(","),
                        path.display()
                    ),
                    Err(e) => println!("  plugin {} INVALID: {e}", path.display()),
                },
                Err(e) => println!("  plugin {} unreadable: {e}", path.display()),
            }
        }
    }
    if found == 0 {
        println!("No plugin manifests in {}.", dir.display());
    }
    Ok(())
}

/// Installs a plugin manifest into the plugins dir after validation.
/// Refuses version mismatches (api_version gate) and pinned plugins.
/// Keeps the previous manifest as `<name>.toml.prev` for rollback.
pub fn install_manifest(src: &std::path::Path, dir: &std::path::Path) -> Result<()> {
    let text = std::fs::read_to_string(src)?;
    let manifest = PluginManifest::from_toml_str(&text)?;
    manifest.validate()?;
    for port in &manifest.ports {
        let expected = automemory_core::ports::PORTS
            .iter()
            .find(|p| p.id == *port)
            .map(|p| p.api_version)
            .unwrap_or("1");
        automemory_core::ports::check_api_version(&manifest.name, &manifest.api_version, expected)?;
    }
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(format!("{}.toml", manifest.name));
    if dest.exists() {
        if dir.join(format!("{}.pinned", manifest.name)).exists() {
            anyhow::bail!(
                "plugin '{}' is pinned; unpin or roll back before updating",
                manifest.name
            );
        }
        let prev = dir.join(format!("{}.toml.prev", manifest.name));
        let _ = std::fs::remove_file(&prev);
        std::fs::rename(&dest, &prev)?;
    }
    std::fs::write(&dest, text)?;
    println!(
        "✓ Installed plugin {} v{} (ports: [{}])",
        manifest.name,
        manifest.version,
        manifest.ports.join(", ")
    );
    println!(
        "  Capabilities: read_ledger={} write_memory={} block_tools={} network={}",
        manifest.capabilities.read_ledger,
        manifest.capabilities.write_memory,
        manifest.capabilities.block_tools,
        manifest.capabilities.network
    );
    Ok(())
}

/// Pins an installed plugin version so updates refuse to replace it.
pub fn pin_manifest(name: &str, dir: &std::path::Path) -> Result<()> {
    let dest = dir.join(format!("{name}.toml"));
    if !dest.exists() {
        anyhow::bail!("plugin '{name}' is not installed");
    }
    std::fs::write(dir.join(format!("{name}.pinned")), "pinned\n")?;
    println!("✓ Pinned plugin '{name}'.");
    Ok(())
}

/// Restores the pre-update manifest kept by install/update.
pub fn rollback_manifest(name: &str, dir: &std::path::Path) -> Result<()> {
    let prev = dir.join(format!("{name}.toml.prev"));
    if !prev.exists() {
        anyhow::bail!("plugin '{name}' has no previous version to roll back to");
    }
    std::fs::rename(&prev, dir.join(format!("{name}.toml")))?;
    println!("✓ Rolled back plugin '{name}'.");
    Ok(())
}

fn plugins_dir() -> Result<std::path::PathBuf> {
    let config = Config::default_config();
    Ok(config.home_dir.join("plugins"))
}

/// Installs a plugin manifest from a user-supplied path.
pub fn install(path: &str) -> Result<()> {
    install_manifest(std::path::Path::new(path), &plugins_dir()?)
}

/// Re-installs (updates) a plugin manifest from a user-supplied path.
pub fn update(path: &str) -> Result<()> {
    install_manifest(std::path::Path::new(path), &plugins_dir()?)
}

/// Pins an installed plugin version.
pub fn pin(name: &str) -> Result<()> {
    pin_manifest(name, &plugins_dir()?)
}

/// Rolls a plugin back to its pre-update manifest.
pub fn rollback(name: &str) -> Result<()> {
    rollback_manifest(name, &plugins_dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_toml(version: &str, api: &str) -> String {
        format!(
            "name = \"demo\"\nversion = \"{version}\"\napi_version = \"{api}\"\nports = [\"Verifier\"]\n"
        )
    }

    fn scrub(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
    }

    #[test]
    fn install_pin_rollback_lifecycle() {
        let dir =
            std::env::temp_dir().join(format!("automemory-plugin-test-{}", std::process::id()));
        scrub(&dir);
        // Source manifest lives apart from the install dir: a refused
        // install must never clobber either side.
        let src = dir.join("src-demo.toml");
        std::fs::write(&src, manifest_toml("1", "1")).unwrap();

        install_manifest(&src, &dir).unwrap();
        assert!(dir.join("demo.toml").exists());

        // Mismatched api_version is refused before anything is written.
        std::fs::write(&src, manifest_toml("9", "99")).unwrap();
        assert!(install_manifest(&src, &dir).is_err());
        let kept = std::fs::read_to_string(dir.join("demo.toml")).unwrap();
        assert!(kept.contains("version = \"1\""));

        // Update keeps the previous manifest for rollback.
        std::fs::write(&src, manifest_toml("2", "1")).unwrap();
        install_manifest(&src, &dir).unwrap();
        assert!(dir.join("demo.toml.prev").exists());

        // Pinned plugins refuse updates; rollback still restores v1.
        pin_manifest("demo", &dir).unwrap();
        std::fs::write(&src, manifest_toml("3", "1")).unwrap();
        assert!(install_manifest(&src, &dir).is_err());
        rollback_manifest("demo", &dir).unwrap();
        let restored = std::fs::read_to_string(dir.join("demo.toml")).unwrap();
        assert!(restored.contains("version = \"1\""));

        // Unknown plugin names fail cleanly.
        assert!(pin_manifest("missing", &dir).is_err());
        assert!(rollback_manifest("missing", &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
