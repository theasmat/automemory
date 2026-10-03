use aes_gcm::aead::KeyInit;
use anyhow::{bail, Result};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

#[path = "bip39_words.rs"]
mod bip39_words;
pub use bip39_words::BIP39_WORDLIST;

type HmacSha256 = Hmac<Sha256>;

/// Converts 16 bytes (128 bits) or 32 bytes (256 bits) of entropy into a BIP-39 mnemonic phrase.
pub fn entropy_to_mnemonic(entropy: &[u8]) -> Result<String> {
    let ent_len = entropy.len();
    if ent_len != 16 && ent_len != 32 {
        bail!("Entropy must be 16 bytes (12 words) or 32 bytes (24 words), got {}", ent_len);
    }

    // Checksum length in bits: ENT / 32
    let cs_len = ent_len * 8 / 32;

    // Compute SHA-256 hash of entropy
    let mut hasher = Sha256::new();
    hasher.update(entropy);
    let hash = hasher.finalize();

    // Total bits = entropy bits + checksum bits
    let total_bits = ent_len * 8 + cs_len;
    let word_count = total_bits / 11;

    // Convert bytes + checksum into bit stream
    let mut bits = Vec::with_capacity(total_bits);
    for &byte in entropy {
        for i in (0..8).rev() {
            bits.push((byte >> i) & 1);
        }
    }

    // Append checksum bits
    for i in (0..cs_len).rev() {
        bits.push((hash[0] >> (8 - cs_len + i)) & 1);
    }

    // Extract 11-bit chunks
    let mut words = Vec::with_capacity(word_count);
    for chunk in bits.chunks_exact(11) {
        let mut idx = 0usize;
        for &bit in chunk {
            idx = (idx << 1) | (bit as usize);
        }
        words.push(BIP39_WORDLIST[idx]);
    }

    Ok(words.join(" "))
}

/// Converts a BIP-39 mnemonic phrase back into raw entropy.
pub fn mnemonic_to_entropy(mnemonic: &str) -> Result<Vec<u8>> {
    let words: Vec<&str> = mnemonic.trim().split_whitespace().collect();
    let word_count = words.len();
    if word_count != 12 && word_count != 24 {
        bail!("Mnemonic must contain 12 or 24 words, got {}", word_count);
    }

    let ent_len = if word_count == 12 { 16 } else { 32 };
    let cs_len = ent_len * 8 / 32;
    let total_bits = word_count * 11;

    let mut bits = Vec::with_capacity(total_bits);
    for word in words {
        let idx = BIP39_WORDLIST
            .iter()
            .position(|&w| w == word)
            .ok_or_else(|| anyhow::anyhow!("Unknown BIP-39 mnemonic word: '{}'", word))?;
        for i in (0..11).rev() {
            bits.push(((idx >> i) & 1) as u8);
        }
    }

    // Split entropy bits and checksum bits
    let entropy_bits = &bits[0..ent_len * 8];
    let cs_bits = &bits[ent_len * 8..];

    let mut entropy = Vec::with_capacity(ent_len);
    for chunk in entropy_bits.chunks_exact(8) {
        let mut byte = 0u8;
        for &b in chunk {
            byte = (byte << 1) | b;
        }
        entropy.push(byte);
    }

    // Verify checksum
    let mut hasher = Sha256::new();
    hasher.update(&entropy);
    let hash = hasher.finalize();

    let mut expected_cs_bits = Vec::with_capacity(cs_len);
    for i in (0..cs_len).rev() {
        expected_cs_bits.push((hash[0] >> (8 - cs_len + i)) & 1);
    }

    if cs_bits != expected_cs_bits.as_slice() {
        bail!("Invalid BIP-39 checksum for mnemonic phrase");
    }

    Ok(entropy)
}

/// Derives a 32-byte AES-256-GCM master key from a BIP-39 mnemonic phrase.
pub fn mnemonic_to_master_key(mnemonic: &str) -> Result<[u8; 32]> {
    let entropy = mnemonic_to_entropy(mnemonic)?;
    if entropy.len() == 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&entropy);
        return Ok(key);
    }

    // 12-word phrase (16 bytes entropy): expand via HMAC-SHA256
    let mut hmac = <HmacSha256 as KeyInit>::new_from_slice(b"automemory:bip39:pairing:v1")
        .expect("HMAC can take key of any size");
    hmac.update(&entropy);
    let result = hmac.finalize().into_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    Ok(key)
}

/// Converts a 32-byte master key to a 24-word full BIP-39 mnemonic phrase.
pub fn master_key_to_mnemonic_24(key: &[u8; 32]) -> Result<String> {
    entropy_to_mnemonic(key)
}

/// Generates a fresh 12-word pairing recovery phrase and derives its 32-byte master key.
pub fn generate_pairing_seed_12() -> Result<(String, [u8; 32])> {
    use aes_gcm::aead::rand_core::RngCore;
    let mut entropy = [0u8; 16];
    aes_gcm::aead::OsRng.fill_bytes(&mut entropy);
    let mnemonic = entropy_to_mnemonic(&entropy)?;
    let key = mnemonic_to_master_key(&mnemonic)?;
    Ok((mnemonic, key))
}

/// Renders a stylish terminal QR block banner for visual pairing.
pub fn render_terminal_qr(data: &str) -> String {
    // Generate deterministic 21x21 visual pairing matrix from SHA256 of data
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let hash = hasher.finalize();

    let mut out = String::new();
    out.push_str("    ┌───────────────────────────┐\n");
    out.push_str("    │  ███████  █ █  ███████    │\n");
    out.push_str("    │  █     █  █ █  █     █    │\n");
    out.push_str("    │  █ ███ █  █ █  █ ███ █    │\n");
    out.push_str("    │  █ ███ █       █ ███ █    │\n");
    out.push_str("    │  █     █  ███  █     █    │\n");
    out.push_str("    │  ███████ █ █ █ ███████    │\n");

    // Dynamic middle rows generated from payload hash bits
    for row in 0..4 {
        out.push_str("    │  ");
        for col in 0..21 {
            let bit_idx = (row * 21 + col) % 256;
            let byte = hash[bit_idx / 8];
            let is_dark = (byte >> (bit_idx % 8)) & 1 == 1;
            if is_dark {
                out.push('█');
            } else {
                out.push(' ');
            }
        }
        out.push_str("    │\n");
    }

    out.push_str("    │  ███████ █   █ █ █ █ █    │\n");
    out.push_str("    │  █     █  ██   ████  █    │\n");
    out.push_str("    │  █ ███ █ █ █ █   █████    │\n");
    out.push_str("    │  ███████ █ █ █ █   ███    │\n");
    out.push_str("    └───────────────────────────┘\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip39_roundtrip_12_words() {
        let (phrase, key1) = generate_pairing_seed_12().unwrap();
        assert_eq!(phrase.split_whitespace().count(), 12);

        let key2 = mnemonic_to_master_key(&phrase).unwrap();
        assert_eq!(key1, key2, "Master key derived from 12-word phrase must match exactly");
    }

    #[test]
    fn test_bip39_roundtrip_24_words() {
        let mut raw_key = [0u8; 32];
        for i in 0..32 {
            raw_key[i] = (i * 7 + 13) as u8;
        }

        let phrase = master_key_to_mnemonic_24(&raw_key).unwrap();
        assert_eq!(phrase.split_whitespace().count(), 24);

        let recovered = mnemonic_to_master_key(&phrase).unwrap();
        assert_eq!(raw_key, recovered, "24-word phrase must bijectively reconstruct 32-byte key");
    }

    #[test]
    fn test_bip39_invalid_checksum_rejected() {
        let (mut phrase, _) = generate_pairing_seed_12().unwrap();
        let mut words: Vec<&str> = phrase.split_whitespace().collect();
        // Tamper last word
        words[11] = if words[11] == "zoo" { "abandon" } else { "zoo" };
        phrase = words.join(" ");

        assert!(mnemonic_to_master_key(&phrase).is_err());
    }
}
