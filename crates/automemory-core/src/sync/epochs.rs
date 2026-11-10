//! Sync v2: cutting a device off.
//!
//! Revoking a device rotates the vault key. The device that revokes writes an *epoch bulletin*
//! (`epochs/epoch-N.json`, signed) holding the new key wrapped to every remaining device with
//! X25519 + AES-256-GCM, plus the older keys encrypted under the new one so a computer paired later
//! with the new recovery phrase can still read the history. The old recovery phrase stops working
//! for anything written afterwards. The bulletin is plain JSON: it names the revoked device and
//! carries only wrapped keys, never a usable one.
//!
//! What it cannot do: take back what the revoked device already read or copied.

use super::devices::own_key;
use crate::crypto::{BlindCrypto, EncryptedBlob};
use crate::db::Database;
use aes_gcm::aead::rand_core::RngCore;
use anyhow::{bail, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use x25519_dalek::{PublicKey, StaticSecret};

const EPOCH_KEY: &str = "epoch_current";
const OLD_KEYS: &str = "old_keys";
/// The rotated key waiting to be written to the OS keychain by the caller (core never touches it).
pub const PENDING_KEY: &str = "pending_master_key";
pub const SELF_REVOKED: &str = "self_revoked";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Wrap {
    eph: String,
    blob: EncryptedBlob,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bulletin {
    epoch: u64,
    revoked: String,
    by: String,
    strict: bool,
    wraps: BTreeMap<String, Wrap>,
    history: EncryptedBlob,
    sig: String,
}

fn enc_secret(db: &Database) -> Result<StaticSecret> {
    let seed = own_key(db)?.to_bytes();
    let mut h = Sha256::new();
    h.update(b"am-x25519");
    h.update(seed);
    Ok(StaticSecret::from(<[u8; 32]>::from(h.finalize())))
}

pub fn own_enc_pubkey(db: &Database) -> Result<String> {
    Ok(B64.encode(PublicKey::from(&enc_secret(db)?).as_bytes()))
}

fn kek(shared: &[u8; 32], eph: &[u8; 32], recipient: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"am-wrap-v1");
    h.update(shared);
    h.update(eph);
    h.update(recipient);
    h.finalize().into()
}

fn wrap(recipient_b64: &str, key: &[u8; 32]) -> Result<Wrap> {
    let recipient = <[u8; 32]>::try_from(B64.decode(recipient_b64)?.as_slice()).map_err(|_| anyhow::anyhow!("bad device key"))?;
    let mut seed = [0u8; 32];
    aes_gcm::aead::OsRng.fill_bytes(&mut seed);
    let eph = StaticSecret::from(seed);
    let eph_pub = PublicKey::from(&eph);
    let shared = eph.diffie_hellman(&PublicKey::from(recipient));
    let kek = kek(shared.as_bytes(), eph_pub.as_bytes(), &recipient);
    Ok(Wrap { eph: B64.encode(eph_pub.as_bytes()), blob: BlindCrypto::encrypt(&kek, key)? })
}

fn unwrap(db: &Database, w: &Wrap) -> Option<[u8; 32]> {
    let secret = enc_secret(db).ok()?;
    let eph = <[u8; 32]>::try_from(B64.decode(&w.eph).ok()?.as_slice()).ok()?;
    let me = *PublicKey::from(&secret).as_bytes();
    let shared = secret.diffie_hellman(&PublicKey::from(eph));
    let plain = BlindCrypto::decrypt(&kek(shared.as_bytes(), &eph, &me), &w.blob).ok()?;
    <[u8; 32]>::try_from(plain.as_slice()).ok()
}

fn signed_bytes(b: &Bulletin) -> Vec<u8> {
    let wraps = serde_json::to_string(&b.wraps).unwrap_or_default();
    format!(
        "am-epoch-v1\n{}\n{}\n{}\n{}\n{:x}\n{}",
        b.epoch,
        b.revoked,
        b.by,
        b.strict,
        Sha256::digest(wraps.as_bytes()),
        b.history.ciphertext_base64
    )
    .into_bytes()
}

pub fn current_epoch(db: &Database) -> u64 {
    db.sync_kv_get(EPOCH_KEY).and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// Keys of earlier epochs, newest last: what is needed to read files written before a rotation.
pub fn old_keys(db: &Database) -> Vec<[u8; 32]> {
    db.sync_kv_get(OLD_KEYS)
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
        .iter()
        .filter_map(|k| <[u8; 32]>::try_from(B64.decode(k).ok()?.as_slice()).ok())
        .collect()
}

fn remember_old_keys(db: &Database, add: &[[u8; 32]]) {
    let mut all = old_keys(db);
    for k in add {
        if !all.contains(k) {
            all.push(*k);
        }
    }
    let enc: Vec<String> = all.iter().map(|k| B64.encode(k)).collect();
    db.sync_kv_set(OLD_KEYS, &serde_json::to_string(&enc).unwrap_or_default());
}

fn epochs_dir(vault_root: &Path) -> PathBuf {
    vault_root.join("epochs")
}

/// Cuts `target` off: rotates the vault key and publishes the bulletin. Returns the new key; the caller
/// writes it to the keychain and shows it as the new recovery phrase.
pub fn revoke(db: &Database, vault_root: &Path, target: &str, current_key: &[u8; 32]) -> Result<[u8; 32]> {
    let me = super::deltas::device_id(db)?;
    if target == me {
        bail!("this is the computer you are on; revoke another one");
    }
    match db.get_sync_device(target)? {
        None => bail!("no device {target}; run `automemory sync devices` to see them"),
        Some(d) if d.status == "revoked" => bail!("{target} is already revoked"),
        Some(_) => {}
    }
    let mut seed = [0u8; 32];
    aes_gcm::aead::OsRng.fill_bytes(&mut seed);
    let new_key = seed;
    let epoch = current_epoch(db) + 1;

    let mut wraps = BTreeMap::new();
    for d in db.list_sync_devices()? {
        if d.status == "active" && d.device_id != target && d.device_id != me && !d.enc_pubkey.is_empty() {
            wraps.insert(d.device_id.clone(), wrap(&d.enc_pubkey, &new_key)?);
        }
    }
    let mut history = old_keys(db);
    history.push(*current_key);
    let history_json = serde_json::to_vec(&history.iter().map(|k| B64.encode(k)).collect::<Vec<_>>())?;
    let mut bulletin = Bulletin {
        epoch,
        revoked: target.to_string(),
        by: me,
        strict: true,
        wraps,
        history: BlindCrypto::encrypt(&new_key, &history_json)?,
        sig: String::new(),
    };
    bulletin.sig = B64.encode(own_key(db)?.sign(&signed_bytes(&bulletin)).to_bytes());

    fs::create_dir_all(epochs_dir(vault_root))?;
    let path = epochs_dir(vault_root).join(format!("epoch-{epoch}.json"));
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, serde_json::to_vec_pretty(&bulletin)?)?;
    fs::rename(&tmp, &path)?;

    db.set_sync_device_status(target, "revoked", epoch as i64)?;
    db.set_sync_requires_signatures(true);
    remember_old_keys(db, &[*current_key]);
    db.sync_kv_set(EPOCH_KEY, &epoch.to_string());
    db.sync_kv_set(PENDING_KEY, &B64.encode(new_key));
    Ok(new_key)
}

/// Reads bulletins newer than this device's epoch and follows them. Returns the new vault key when
/// the key changed. Errors when this device itself was revoked.
pub fn adopt(db: &Database, vault_root: &Path, my_device: &str, current_key: &[u8; 32]) -> Result<Option<[u8; 32]>> {
    let mut files: Vec<(u64, PathBuf)> = fs::read_dir(epochs_dir(vault_root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let n = name.strip_prefix("epoch-")?.strip_suffix(".json")?.parse::<u64>().ok()?;
            Some((n, e.path()))
        })
        .collect();
    files.sort();
    let mut key = *current_key;
    let mut changed = false;
    for (n, path) in files {
        if n <= current_epoch(db) {
            continue;
        }
        let Ok(b) = serde_json::from_slice::<Bulletin>(&fs::read(&path)?) else {
            tracing::warn!(path = %path.display(), "epoch bulletin unreadable: skipped");
            continue;
        };
        if b.epoch != n {
            continue;
        }
        if b.revoked == my_device {
            db.sync_kv_set(SELF_REVOKED, "1");
            bail!("this computer was removed from sync (device {my_device}); pair it again with the new recovery phrase");
        }
        let new_key = if let Some(w) = b.wraps.get(my_device) {
            // Trust only a bulletin signed by a device we know and that is still active.
            let signer = db.get_sync_device(&b.by)?.filter(|d| d.status == "active");
            let ok = signer.is_some_and(|d| {
                let pk = B64.decode(&d.pubkey).ok().and_then(|v| <[u8; 32]>::try_from(v.as_slice()).ok());
                let sig = B64.decode(&b.sig).ok().and_then(|v| <[u8; 64]>::try_from(v.as_slice()).ok());
                matches!((pk, sig), (Some(pk), Some(sig)) if VerifyingKey::from_bytes(&pk)
                    .and_then(|vk| vk.verify(&signed_bytes(&b), &Signature::from_bytes(&sig))).is_ok())
            });
            if !ok {
                tracing::warn!(epoch = n, by = %b.by, "epoch bulletin refused: not signed by a known active device");
                continue;
            }
            unwrap(db, w)
        } else if BlindCrypto::decrypt(&key, &b.history).is_ok() {
            Some(key) // paired with the new phrase: we already hold this epoch's key
        } else {
            None
        };
        let Some(new_key) = new_key else { continue };
        let mut older = vec![key];
        if let Ok(plain) = BlindCrypto::decrypt(&new_key, &b.history) {
            if let Ok(list) = serde_json::from_slice::<Vec<String>>(&plain) {
                older.extend(list.iter().filter_map(|k| <[u8; 32]>::try_from(B64.decode(k).ok()?.as_slice()).ok()));
            }
        }
        older.retain(|k| *k != new_key);
        remember_old_keys(db, &older);
        let _ = db.set_sync_device_status(&b.revoked, "revoked", n as i64);
        if b.strict {
            db.set_sync_requires_signatures(true);
        }
        db.sync_kv_set(EPOCH_KEY, &n.to_string());
        if new_key != key {
            db.sync_kv_set(PENDING_KEY, &B64.encode(new_key));
            key = new_key;
            changed = true;
        }
    }
    Ok(changed.then_some(key))
}

/// The rotated key a caller should write to the keychain, once.
pub fn take_pending_key(db: &Database) -> Option<[u8; 32]> {
    let b64 = db.sync_kv_get(PENDING_KEY).filter(|s| !s.is_empty())?;
    <[u8; 32]>::try_from(B64.decode(b64).ok()?.as_slice()).ok()
}

pub fn clear_pending_key(db: &Database) {
    db.sync_kv_set(PENDING_KEY, "");
}
