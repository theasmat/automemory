use crate::models::{
    Memory, MemoryScope, MemorySourceKind, MemoryStatus, MemoryType, ProceduralSkill,
};
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{DateTime, Utc};
use hmac::Hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedBlob {
    pub ciphertext_base64: String,
    pub nonce_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindMemoryPayload {
    pub summary: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub metadata: serde_json::Value,
    pub entities: Vec<String>,
    pub tags: Vec<String>,
    pub source_kind: MemorySourceKind,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindMemoryWire {
    pub id: String,
    pub blind_project_id: String,
    pub ciphertext_base64: String,
    pub nonce_base64: String,
    pub blind_scope: String,
    pub blind_type: String,
    pub blind_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    pub confidence: f64,
    pub importance: f64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindSkillPayload {
    pub name: String,
    pub description: String,
    pub trigger_patterns: Vec<String>,
    pub preconditions: Vec<String>,
    pub steps: Vec<String>,
    pub caveats: Vec<String>,
    pub source_trace_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindSkillWire {
    pub id: String,
    pub blind_project_id: Option<String>,
    pub ciphertext_base64: String,
    pub nonce_base64: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedDeltaWire {
    pub path: String,
    pub category: Option<String>,
    pub sha256: String,
    pub ciphertext_base64: String,
    pub nonce_base64: String,
    pub size_bytes: usize,
}

pub struct BlindCrypto;

impl BlindCrypto {
    /// Derive a 256-bit master encryption key from a user secret and salt using PBKDF2-HMAC-SHA256.
    pub fn derive_key(secret: &str, salt: &[u8]) -> [u8; 32] {
        use hmac::digest::Mac;

        let mut key = [0u8; 32];
        let mut block = [0u8; 32];

        // Iterative HMAC-SHA256 (PBKDF2 style, 10,000 iterations)
        let mut hmac = <HmacSha256 as KeyInit>::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        hmac.update(salt);
        hmac.update(&[0, 0, 0, 1]);
        let mut u = hmac.finalize().into_bytes();
        block.copy_from_slice(&u);

        for _ in 1..10_000 {
            let mut hmac = <HmacSha256 as KeyInit>::new_from_slice(secret.as_bytes())
                .expect("HMAC can take key of any size");
            hmac.update(&u);
            u = hmac.finalize().into_bytes();
            for i in 0..32 {
                block[i] ^= u[i];
            }
        }

        key.copy_from_slice(&block);
        key
    }

    /// Retrieve or automatically generate a 256-bit zero-knowledge master key.
    pub fn get_or_create_master_key(custom_key: Option<&str>) -> Result<[u8; 32]> {
        // 1. Explicit CLI argument override
        if let Some(k) = custom_key {
            if !k.trim().is_empty() {
                return Ok(Self::derive_key(k.trim(), b"automemory-salt-v1"));
            }
        }

        // 2. AUTOMEMORY_KEY environment variable
        if let Ok(env_key) = std::env::var("AUTOMEMORY_KEY") {
            if !env_key.trim().is_empty() {
                return Ok(Self::derive_key(env_key.trim(), b"automemory-salt-v1"));
            }
        }

        // 3. Global config (~/.automemory/config.toml)
        if let Ok(global_cfg) = crate::config::GlobalConfig::load() {
            if let Some(k) = global_cfg
                .cloud
                .encryption_key
                .as_ref()
                .or(global_cfg.vault.encryption_key.as_ref())
            {
                if !k.trim().is_empty() {
                    return Ok(Self::derive_key(k.trim(), b"automemory-salt-v1"));
                }
            }
        }

        // 4. Keychain or local master.key in user home directory
        if let Some(key) = crate::keychain::get_master_key() {
            return Ok(key);
        }

        // Generate a new 256-bit random master key and save it securely
        let mut raw_key = [0u8; 32];
        use aes_gcm::aead::rand_core::RngCore;
        aes_gcm::aead::OsRng.fill_bytes(&mut raw_key);

        let _ = crate::keychain::set_master_key(&raw_key);

        Ok(raw_key)
    }

    /// Derive an index key for generating blind search tokens without exposing the master key.
    pub fn derive_index_key(master_key: &[u8; 32]) -> [u8; 32] {
        use hmac::digest::Mac;

        let mut hmac = <HmacSha256 as KeyInit>::new_from_slice(master_key)
            .expect("HMAC can take key of any size");
        hmac.update(b"automemory:blind-index:v1");
        let result = hmac.finalize().into_bytes();
        let mut index_key = [0u8; 32];
        index_key.copy_from_slice(&result);
        index_key
    }

    /// Encrypt plaintext bytes using AES-256-GCM authenticated encryption.
    pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<EncryptedBlob> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow::anyhow!("Failed to initialize AES-GCM cipher: {}", e))?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("AES-GCM encryption failed: {}", e))?;

        Ok(EncryptedBlob {
            ciphertext_base64: BASE64.encode(ciphertext),
            nonce_base64: BASE64.encode(nonce),
        })
    }

    /// Decrypt an EncryptedBlob using AES-256-GCM authenticated encryption.
    pub fn decrypt(key: &[u8; 32], blob: &EncryptedBlob) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow::anyhow!("Failed to initialize AES-GCM cipher: {}", e))?;

        let nonce_bytes = BASE64
            .decode(&blob.nonce_base64)
            .context("Failed to base64 decode nonce")?;
        if nonce_bytes.len() != 12 {
            bail!(
                "Invalid nonce length: expected 12 bytes, got {}",
                nonce_bytes.len()
            );
        }
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = BASE64
            .decode(&blob.ciphertext_base64)
            .context("Failed to base64 decode ciphertext")?;

        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).map_err(|e| {
            anyhow::anyhow!("AES-GCM decryption failed (tampered or wrong key): {}", e)
        })?;

        Ok(plaintext)
    }

    /// Encrypt a UTF-8 string into an EncryptedBlob.
    pub fn encrypt_string(key: &[u8; 32], plaintext: &str) -> Result<EncryptedBlob> {
        Self::encrypt(key, plaintext.as_bytes())
    }

    /// Decrypt an EncryptedBlob into a UTF-8 string.
    pub fn decrypt_string(key: &[u8; 32], blob: &EncryptedBlob) -> Result<String> {
        let bytes = Self::decrypt(key, blob)?;
        String::from_utf8(bytes).context("Decrypted data is not valid UTF-8")
    }

    /// Compute a deterministic blind index token: HMAC-SHA256(index_key, normalized_token).
    /// The blind token allows exact equality search on the server without revealing plaintext.
    pub fn blind_token(index_key: &[u8; 32], token: &str) -> String {
        use hmac::digest::Mac;

        let normalized = token.trim().to_lowercase();
        let mut hmac = <HmacSha256 as KeyInit>::new_from_slice(index_key)
            .expect("HMAC can take key of any size");
        hmac.update(normalized.as_bytes());
        let result = hmac.finalize().into_bytes();
        result.iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Encrypt a local Memory record into a zero-knowledge BlindMemoryWire for transmission.
    pub fn encrypt_memory(
        master_key: &[u8; 32],
        index_key: &[u8; 32],
        memory: &Memory,
        project_slug: &str,
    ) -> Result<BlindMemoryWire> {
        Self::encrypt_memory_with_embedding(master_key, index_key, memory, project_slug, None)
    }

    /// Encrypt a local Memory record with an optional client-computed embedding vector for blind ANN indexing.
    pub fn encrypt_memory_with_embedding(
        master_key: &[u8; 32],
        index_key: &[u8; 32],
        memory: &Memory,
        project_slug: &str,
        embedding: Option<Vec<f32>>,
    ) -> Result<BlindMemoryWire> {
        let payload = BlindMemoryPayload {
            summary: memory.summary.clone(),
            content: memory.content.clone(),
            memory_type: memory.memory_type,
            metadata: memory.metadata.clone(),
            entities: memory.entities.clone(),
            tags: memory.tags.clone(),
            source_kind: memory.source_kind,
            version: memory.version,
        };
        let payload_json = serde_json::to_string(&payload)?;
        let blob = Self::encrypt_string(master_key, &payload_json)?;

        let blind_type = Self::blind_token(index_key, &memory.memory_type.to_string());
        let blind_tags = memory
            .tags
            .iter()
            .map(|t| Self::blind_token(index_key, t))
            .collect();

        Ok(BlindMemoryWire {
            id: memory.id.clone(),
            blind_project_id: project_slug.to_string(),
            ciphertext_base64: blob.ciphertext_base64,
            nonce_base64: blob.nonce_base64,
            blind_scope: memory.scope.to_string(),
            blind_type,
            blind_tags,
            embedding,
            confidence: memory.confidence,
            importance: memory.importance,
            status: memory.status.to_string(),
            created_at: memory.created_at.to_rfc3339(),
            updated_at: memory.updated_at.to_rfc3339(),
        })
    }

    /// Decrypt a BlindMemoryWire received from the server into an authentic Memory record.
    pub fn decrypt_memory(
        master_key: &[u8; 32],
        wire: &BlindMemoryWire,
        default_project_id: Option<&str>,
    ) -> Result<Memory> {
        let blob = EncryptedBlob {
            ciphertext_base64: wire.ciphertext_base64.clone(),
            nonce_base64: wire.nonce_base64.clone(),
        };
        let payload_json = Self::decrypt_string(master_key, &blob)?;
        let payload: BlindMemoryPayload = serde_json::from_str(&payload_json)?;

        let scope = wire
            .blind_scope
            .parse::<MemoryScope>()
            .unwrap_or(MemoryScope::Project);
        let status = wire
            .status
            .parse::<MemoryStatus>()
            .unwrap_or(MemoryStatus::Active);
        let created_at = DateTime::parse_from_rfc3339(&wire.created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&wire.updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Memory {
            id: wire.id.clone(),
            scope,
            project_id: if scope == MemoryScope::Global {
                None
            } else {
                default_project_id
                    .map(|s| s.to_string())
                    .or_else(|| Some(wire.blind_project_id.clone()))
            },
            memory_type: payload.memory_type,
            content: payload.content,
            summary: payload.summary,
            confidence: wire.confidence,
            importance: wire.importance,
            freshness: 1.0,
            status,
            source_kind: payload.source_kind,
            version: payload.version,
            created_at,
            updated_at,
            last_recalled_at: None,
            tags: payload.tags,
            entities: payload.entities,
            metadata: payload.metadata,
        })
    }

    /// Encrypt a ProceduralSkill into a zero-knowledge BlindSkillWire.
    pub fn encrypt_skill(
        master_key: &[u8; 32],
        skill: &ProceduralSkill,
        project_slug: &str,
    ) -> Result<BlindSkillWire> {
        let payload = BlindSkillPayload {
            name: skill.name.clone(),
            description: skill.description.clone(),
            trigger_patterns: skill.trigger_patterns.clone(),
            preconditions: skill.preconditions.clone(),
            steps: skill.steps.clone(),
            caveats: skill.caveats.clone(),
            source_trace_ids: skill.source_trace_ids.clone(),
        };
        let payload_json = serde_json::to_string(&payload)?;
        let blob = Self::encrypt_string(master_key, &payload_json)?;

        Ok(BlindSkillWire {
            id: skill.id.clone(),
            blind_project_id: Some(project_slug.to_string()),
            ciphertext_base64: blob.ciphertext_base64,
            nonce_base64: blob.nonce_base64,
            success_count: skill.success_count,
            failure_count: skill.failure_count,
            created_at: skill.created_at.to_rfc3339(),
            updated_at: skill.updated_at.to_rfc3339(),
        })
    }

    /// Decrypt a BlindSkillWire into an authentic ProceduralSkill.
    pub fn decrypt_skill(
        master_key: &[u8; 32],
        wire: &BlindSkillWire,
        default_project_id: Option<&str>,
    ) -> Result<ProceduralSkill> {
        let blob = EncryptedBlob {
            ciphertext_base64: wire.ciphertext_base64.clone(),
            nonce_base64: wire.nonce_base64.clone(),
        };
        let payload_json = Self::decrypt_string(master_key, &blob)?;
        let payload: BlindSkillPayload = serde_json::from_str(&payload_json)?;

        let created_at = DateTime::parse_from_rfc3339(&wire.created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&wire.updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(ProceduralSkill {
            id: wire.id.clone(),
            project_id: default_project_id
                .map(|s| s.to_string())
                .or_else(|| wire.blind_project_id.clone()),
            name: payload.name,
            description: payload.description,
            trigger_patterns: payload.trigger_patterns,
            preconditions: payload.preconditions,
            steps: payload.steps,
            caveats: payload.caveats,
            success_count: wire.success_count,
            failure_count: wire.failure_count,
            source_trace_ids: payload.source_trace_ids,
            created_at,
            updated_at,
        })
    }

    /// Encrypt a document or plan delta for zero-knowledge cloud storage.
    pub fn encrypt_delta(
        master_key: &[u8; 32],
        path: &str,
        category: &str,
        content: &str,
    ) -> Result<EncryptedDeltaWire> {
        use sha2::Digest;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let sha256 = format!("{:x}", hasher.finalize());

        let blob = Self::encrypt(master_key, content.as_bytes())?;

        Ok(EncryptedDeltaWire {
            path: path.to_string(),
            category: Some(category.to_string()),
            sha256,
            ciphertext_base64: blob.ciphertext_base64,
            nonce_base64: blob.nonce_base64,
            size_bytes: content.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption_roundtrip() {
        let secret = "my-super-secure-passphrase-2026";
        let salt = b"automemory-salt";
        let key = BlindCrypto::derive_key(secret, salt);

        let sensitive_text =
            "# Q4 Architecture Plan\nInternal confidential roadmaps with secret API keys.";
        let encrypted = BlindCrypto::encrypt_string(&key, sensitive_text).unwrap();

        assert_ne!(encrypted.ciphertext_base64, sensitive_text);
        assert!(!encrypted.ciphertext_base64.contains("Roadmaps"));

        let decrypted = BlindCrypto::decrypt_string(&key, &encrypted).unwrap();
        assert_eq!(decrypted, sensitive_text);
    }

    #[test]
    fn test_decryption_fails_with_tampered_ciphertext() {
        let secret = "my-super-secure-passphrase-2026";
        let salt = b"automemory-salt";
        let key = BlindCrypto::derive_key(secret, salt);

        let sensitive_text = "Classified system memory";
        let mut encrypted = BlindCrypto::encrypt_string(&key, sensitive_text).unwrap();

        // Tamper with ciphertext
        encrypted.ciphertext_base64.replace_range(0..2, "XX");

        let result = BlindCrypto::decrypt_string(&key, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_blind_token_determinism_and_hashing() {
        let secret = "my-super-secure-passphrase-2026";
        let key = BlindCrypto::derive_key(secret, b"salt");
        let index_key = BlindCrypto::derive_index_key(&key);

        let token1 = BlindCrypto::blind_token(&index_key, "c-todo-app");
        let token2 = BlindCrypto::blind_token(&index_key, "C-TODO-APP ");
        let different_token = BlindCrypto::blind_token(&index_key, "python-microservice");

        assert_eq!(token1, token2);
        assert_ne!(token1, different_token);
        assert_eq!(token1.len(), 64); // 32 bytes in hex = 64 chars
    }

    #[test]
    fn test_memory_blind_encryption_and_decryption_roundtrip() {
        let master_key = BlindCrypto::derive_key("super-secret", b"salt");
        let index_key = BlindCrypto::derive_index_key(&master_key);

        let mem = Memory {
            id: "mem_test_123".to_string(),
            scope: MemoryScope::Project,
            project_id: Some("my-cool-project".to_string()),
            memory_type: MemoryType::Decision,
            content: "We decided to use SQLite at the edge for sub-millisecond queries."
                .to_string(),
            summary: "Architecture: SQLite edge database decision".to_string(),
            confidence: 0.98,
            importance: 0.95,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["architecture".to_string(), "sqlite".to_string()],
            entities: vec!["SQLite".to_string()],
            metadata: serde_json::json!({"phase": "q4"}),
        };

        let wire =
            BlindCrypto::encrypt_memory(&master_key, &index_key, &mem, "my-cool-project").unwrap();

        // Verify zero-knowledge properties
        assert_ne!(wire.ciphertext_base64, mem.content);
        assert!(!wire.ciphertext_base64.contains("SQLite"));
        assert!(!wire.ciphertext_base64.contains("Architecture"));
        assert_eq!(wire.blind_tags.len(), 2);
        assert_ne!(wire.blind_tags[0], "architecture");

        // Decrypt
        let restored =
            BlindCrypto::decrypt_memory(&master_key, &wire, Some("my-cool-project")).unwrap();
        assert_eq!(restored.id, mem.id);
        assert_eq!(restored.content, mem.content);
        assert_eq!(restored.summary, mem.summary);
        assert_eq!(restored.memory_type, MemoryType::Decision);
        assert_eq!(restored.tags, mem.tags);
        assert_eq!(restored.entities, mem.entities);
        assert_eq!(restored.metadata, mem.metadata);
    }

    #[test]
    fn test_skill_blind_encryption_and_decryption_roundtrip() {
        let master_key = BlindCrypto::derive_key("super-secret", b"salt");

        let skill = ProceduralSkill {
            id: "skill_test_456".to_string(),
            project_id: Some("my-cool-project".to_string()),
            name: "Deploy to Cloudflare Workers".to_string(),
            description: "Step by step procedure to deploy worker with D1 and R2".to_string(),
            trigger_patterns: vec!["deploy to cloudflare".to_string()],
            preconditions: vec!["wrangler installed".to_string()],
            steps: vec!["npx wrangler deploy".to_string()],
            caveats: vec!["Ensure R2 binding exists".to_string()],
            success_count: 5,
            failure_count: 0,
            source_trace_ids: vec!["trace_1".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let wire = BlindCrypto::encrypt_skill(&master_key, &skill, "my-cool-project").unwrap();
        assert!(!wire.ciphertext_base64.contains("Cloudflare"));

        let restored =
            BlindCrypto::decrypt_skill(&master_key, &wire, Some("my-cool-project")).unwrap();
        assert_eq!(restored.name, skill.name);
        assert_eq!(restored.description, skill.description);
        assert_eq!(restored.steps, skill.steps);
        assert_eq!(restored.caveats, skill.caveats);
        assert_eq!(restored.success_count, 5);
    }

    #[test]
    fn test_delta_blind_encryption_roundtrip() {
        let master_key = BlindCrypto::derive_key("super-secret", b"salt");
        let content = "# Secret Roadmap\nDo not leak!";

        let delta_wire =
            BlindCrypto::encrypt_delta(&master_key, "plans/secret.md", "plans", content).unwrap();
        assert_ne!(delta_wire.ciphertext_base64, content);
        assert!(!delta_wire.ciphertext_base64.contains("Secret"));

        let decrypted_bytes = BlindCrypto::decrypt(
            &master_key,
            &EncryptedBlob {
                ciphertext_base64: delta_wire.ciphertext_base64,
                nonce_base64: delta_wire.nonce_base64,
            },
        )
        .unwrap();

        assert_eq!(String::from_utf8(decrypted_bytes).unwrap(), content);
    }
}
