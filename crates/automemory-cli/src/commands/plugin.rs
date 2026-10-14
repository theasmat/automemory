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
