//! Sync v2: who may write to the vault.
//!
//! Every device holds an ed25519 key and announces its public half as a `device` record. Ops carry a
//! signature over their identifying fields, so an op can no longer claim another device or author.
//! The root of trust is the recovery phrase: whoever holds the vault key can enrol a device (first
//! announcement wins; a second, different key for the same device id is refused). A lost or copied
//! device is cut off with [`revoke`], which also rotates the key (see `sync::epochs`).
//!
//! The signing seed is kept in `sync_kv`, protected like the database file itself (which already holds
//! every note in plain text); revocation is the remedy if that file is copied.

use super::deltas::{DeltaOp, ApplyReport};
use crate::db::Database;
use aes_gcm::aead::rand_core::RngCore;
use anyhow::{bail, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rusqlite::params;

pub const WIRE_V2: u8 = 2;
const SEED_KEY: &str = "device_seed";
const SEQ_KEY: &str = "op_seq";
const STRICT_KEY: &str = "require_signatures";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub device_id: String,
    pub pubkey: String,
    /// X25519 public key other devices wrap the vault key to.
    pub enc_pubkey: String,
    pub name: String,
    pub status: String,
    pub epoch: i64,
    pub added_at: String,
    pub revoked_at: Option<String>,
}

impl Database {
    pub fn get_sync_device(&self, device_id: &str) -> Result<Option<DeviceInfo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT device_id, pubkey, enc_pubkey, name, status, epoch, added_at, revoked_at FROM sync_devices WHERE device_id = ?1",
        )?;
        let mut rows = stmt.query(params![device_id])?;
        Ok(match rows.next()? {
            Some(r) => Some(DeviceInfo {
                device_id: r.get(0)?,
                pubkey: r.get(1)?,
                enc_pubkey: r.get(2)?,
                name: r.get(3)?,
                status: r.get(4)?,
                epoch: r.get(5)?,
                added_at: r.get(6)?,
                revoked_at: r.get(7)?,
            }),
            None => None,
        })
    }

    pub fn list_sync_devices(&self) -> Result<Vec<DeviceInfo>> {
        let ids: Vec<String> = {
            let conn = self.conn();
            let mut stmt = conn.prepare("SELECT device_id FROM sync_devices ORDER BY added_at, device_id")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut out = Vec::new();
        for id in ids {
            out.extend(self.get_sync_device(&id)?);
        }
        Ok(out)
    }

    pub fn set_sync_device_status(&self, device_id: &str, status: &str, epoch: i64) -> Result<bool> {
        let revoked_at = (status == "revoked").then(|| Utc::now().to_rfc3339());
        let n = self.conn().execute(
            "UPDATE sync_devices SET status = ?2, epoch = ?3, revoked_at = COALESCE(?4, revoked_at) WHERE device_id = ?1",
            params![device_id, status, epoch, revoked_at],
        )?;
        Ok(n > 0)
    }

    pub(super) fn register_own_device(&self, device_id: &str, pubkey: &str, enc_pubkey: &str) -> Result<()> {
        self.insert_sync_device(device_id, pubkey, enc_pubkey, "this computer", 0)
    }

    fn insert_sync_device(&self, device_id: &str, pubkey: &str, enc_pubkey: &str, name: &str, epoch: i64) -> Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO sync_devices (device_id, pubkey, enc_pubkey, name, status, epoch, added_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6)",
            params![device_id, pubkey, enc_pubkey, name, epoch, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Refuse unsigned (v1) ops once every device has upgraded.
    pub fn sync_requires_signatures(&self) -> bool {
        self.sync_kv_get(STRICT_KEY).as_deref() == Some("1")
    }

    pub fn set_sync_requires_signatures(&self, on: bool) {
        self.sync_kv_set(STRICT_KEY, if on { "1" } else { "0" });
    }
}

/// This device's signing key, created on first use.
pub fn own_key(db: &Database) -> Result<SigningKey> {
    if let Some(b64) = db.sync_kv_get(SEED_KEY) {
        if let Ok(bytes) = B64.decode(b64) {
            if let Ok(seed) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(SigningKey::from_bytes(&seed));
            }
        }
    }
    let mut seed = [0u8; 32];
    aes_gcm::aead::OsRng.fill_bytes(&mut seed);
    db.sync_kv_set(SEED_KEY, &B64.encode(seed));
    Ok(SigningKey::from_bytes(&seed))
}

pub fn own_pubkey(db: &Database) -> Result<String> {
    Ok(B64.encode(own_key(db)?.verifying_key().to_bytes()))
}

/// The bytes a signature covers. Includes the fingerprint, so the payload is covered too.
pub fn canonical(op: &DeltaOp) -> Vec<u8> {
    format!(
        "am-op-v2\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        op.kind,
        op.id,
        op.fingerprint,
        op.device_id,
        op.seq.unwrap_or(0),
        op.author.as_deref().unwrap_or(""),
        op.updated_at,
        op.project_key.as_deref().unwrap_or(""),
        op.scrub.unwrap_or(false)
    )
    .into_bytes()
}

fn next_seq(db: &Database) -> u64 {
    let n = db.sync_kv_get(SEQ_KEY).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0) + 1;
    db.sync_kv_set(SEQ_KEY, &n.to_string());
    n
}

/// Signs ops as this device (call after the author is set).
pub fn sign_ops(db: &Database, ops: &mut [DeltaOp]) -> Result<()> {
    let key = own_key(db)?;
    for op in ops {
        op.v = Some(WIRE_V2);
        op.seq = Some(next_seq(db));
        op.sig = Some(B64.encode(key.sign(&canonical(op)).to_bytes()));
    }
    Ok(())
}

/// The announcement of this device's public key, as a record the other devices register.
pub fn announce_op(db: &Database, device_id: &str) -> Result<DeltaOp> {
    let payload = serde_json::json!({
        "device_id": device_id,
        "pubkey": own_pubkey(db)?,
        "enc_pubkey": super::epochs::own_enc_pubkey(db)?,
        "name": std::env::var("HOSTNAME").ok().or_else(|| std::env::var("HOST").ok()).unwrap_or_default(),
    });
    let fingerprint = super::deltas::fingerprint_of(&payload);
    let mut op = DeltaOp {
        op: "upsert".into(),
        kind: "device".into(),
        id: device_id.to_string(),
        project_id: None,
        updated_at: Utc::now().to_rfc3339(),
        device_id: device_id.to_string(),
        fingerprint,
        payload,
        project_key: None,
        project_slug: None,
        author: None,
        v: None,
        seq: None,
        sig: None,
        scrub: None,
    };
    sign_ops(db, std::slice::from_mut(&mut op))?;
    db.sync_kv_set(&format!("announce_op:{device_id}"), &serde_json::to_string(&op)?);
    Ok(op)
}

fn verify(pubkey_b64: &str, op: &DeltaOp) -> Result<()> {
    let Some(sig_b64) = op.sig.as_deref() else { bail!("unsigned") };
    let pk = <[u8; 32]>::try_from(B64.decode(pubkey_b64)?.as_slice()).map_err(|_| anyhow::anyhow!("bad key"))?;
    let sig = <[u8; 64]>::try_from(B64.decode(sig_b64)?.as_slice()).map_err(|_| anyhow::anyhow!("bad signature"))?;
    VerifyingKey::from_bytes(&pk)?.verify(&canonical(op), &Signature::from_bytes(&sig))?;
    Ok(())
}

/// Registers announced devices and drops ops that are not acceptable. `device` records are consumed here.
/// Unsigned (v1) ops pass unless [`Database::sync_requires_signatures`].
pub fn screen(db: &Database, ops: Vec<DeltaOp>, report: &mut ApplyReport) -> Vec<DeltaOp> {
    let strict = db.sync_requires_signatures();
    let (announcements, rest): (Vec<_>, Vec<_>) = ops.into_iter().partition(|o| o.kind == "device");
    for op in announcements {
        let (Some(id), Some(pk)) = (op.payload["device_id"].as_str(), op.payload["pubkey"].as_str()) else {
            report.rejected += 1;
            continue;
        };
        if id != op.device_id || verify(pk, &op).is_err() {
            tracing::warn!(device = %op.device_id, "sync: device announcement rejected (bad signature)");
            report.rejected += 1;
            continue;
        }
        match db.get_sync_device(id) {
            Ok(Some(known)) if known.pubkey != pk => {
                tracing::warn!(device = id, "sync: a different key for a known device was refused");
                report.rejected += 1;
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                let name = op.payload["name"].as_str().unwrap_or("");
                let enc = op.payload["enc_pubkey"].as_str().unwrap_or("");
                let _ = db.insert_sync_device(id, pk, enc, name, 0);
                // Kept verbatim (it is self-signed) so a compacted vault can still introduce this device.
                db.sync_kv_set(&format!("announce_op:{id}"), &serde_json::to_string(&op).unwrap_or_default());
            }
            Err(_) => report.rejected += 1,
        }
    }
    rest.into_iter()
        .filter(|op| {
            let ok = match (op.v, db.get_sync_device(&op.device_id).ok().flatten()) {
                (Some(WIRE_V2), Some(dev)) => dev.status == "active" && verify(&dev.pubkey, op).is_ok(),
                (Some(WIRE_V2), None) => false, // signed by a device we have not seen announce itself
                (_, Some(_)) => false,          // a device that signs does not send unsigned ops (or it was revoked)
                (_, None) => !strict,
            };
            if !ok {
                report.rejected += 1;
            }
            ok
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::deltas::{collect_deltas, sync_cycle, write_delta_file};

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn a_signature_covers_the_op_and_a_tampered_op_fails() {
        let a = db();
        let mut op = announce_op(&a, "dev-a").unwrap();
        let pk = own_pubkey(&a).unwrap();
        assert!(verify(&pk, &op).is_ok());
        op.id = "dev-evil".into();
        assert!(verify(&pk, &op).is_err());
        assert_ne!(own_pubkey(&db()).unwrap(), pk, "every device has its own key");
    }

    #[test]
    fn the_key_is_stable_across_calls() {
        let a = db();
        assert_eq!(own_pubkey(&a).unwrap(), own_pubkey(&a).unwrap());
        let mut ops = vec![announce_op(&a, "d").unwrap(), announce_op(&a, "d").unwrap()];
        assert!(ops[1].seq > ops[0].seq);
        sign_ops(&a, &mut ops).unwrap();
        assert!(ops.iter().all(|o| o.v == Some(WIRE_V2) && o.sig.is_some()));
        let _ = (collect_deltas, sync_cycle, write_delta_file);
    }
}
