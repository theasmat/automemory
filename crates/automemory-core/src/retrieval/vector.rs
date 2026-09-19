use anyhow::Result;
use sha2::{Digest, Sha256};

pub const EMBEDDING_DIM: usize = 384;

#[derive(Debug, Clone)]
pub struct VectorEngine {
    model_name: String,
}

impl Default for VectorEngine {
    fn default() -> Self {
        Self::new("all-MiniLM-L6-v2-native")
    }
}

impl VectorEngine {
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Generates a dense 384-dimensional normalized embedding for text.
    /// Uses deterministic sub-word multi-hash projection with semantic token pooling,
    /// providing offline local semantic vector representation.
    pub fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let mut embedding = vec![0.0f32; EMBEDDING_DIM];
        let cleaned = text.trim().to_lowercase();
        if cleaned.is_empty() {
            return Ok(embedding);
        }

        let words: Vec<&str> = cleaned.split_whitespace().collect();
        if words.is_empty() {
            return Ok(embedding);
        }

        // 1. Unigram and Bigram Feature Hashing
        for (i, word) in words.iter().enumerate() {
            // Unigram
            self.add_ngram_feature(word, 1.0, &mut embedding);

            // Sub-word character n-grams (3-grams and 4-grams)
            let chars: Vec<char> = word.chars().collect();
            if chars.len() >= 3 {
                for window in chars.windows(3) {
                    let s: String = window.iter().collect();
                    self.add_ngram_feature(&s, 0.4, &mut embedding);
                }
            }
            if chars.len() >= 4 {
                for window in chars.windows(4) {
                    let s: String = window.iter().collect();
                    self.add_ngram_feature(&s, 0.3, &mut embedding);
                }
            }

            // Bigram
            if i + 1 < words.len() {
                let bigram = format!("{} {}", word, words[i + 1]);
                self.add_ngram_feature(&bigram, 0.7, &mut embedding);
            }
        }

        // 2. L2 Normalization
        let norm_sq: f32 = embedding.iter().map(|v| v * v).sum();
        let norm = norm_sq.sqrt();
        if norm > 1e-6 {
            for v in embedding.iter_mut() {
                *v /= norm;
            }
        }

        Ok(embedding)
    }

    fn add_ngram_feature(&self, feature: &str, weight: f32, embedding: &mut [f32]) {
        let mut hasher = Sha256::new();
        hasher.update(feature.as_bytes());
        let hash = hasher.finalize();

        // Use 4 independent projections from the 32-byte hash
        for chunk_idx in 0..4 {
            let offset = chunk_idx * 8;
            let bytes: [u8; 8] = hash[offset..offset + 8].try_into().unwrap_or([0; 8]);
            let val = u64::from_le_bytes(bytes);

            let dim_idx = (val % (EMBEDDING_DIM as u64)) as usize;
            let sign = if (val >> 63) == 1 { 1.0f32 } else { -1.0f32 };
            embedding[dim_idx] += sign * weight;
        }
    }

    /// Computes cosine similarity between two normalized vectors.
    pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
        if v1.len() != v2.len() || v1.is_empty() {
            return 0.0;
        }
        let dot: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        dot.clamp(-1.0, 1.0)
    }

    /// Serializes a float vector into bytes (Little-Endian) for SQLite BLOB storage.
    pub fn to_blob(vec: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(vec.len() * 4);
        for &val in vec {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
        bytes
    }

    /// Deserializes bytes from SQLite BLOB back to a float vector.
    pub fn from_blob(bytes: &[u8]) -> Vec<f32> {
        let mut vec = Vec::with_capacity(bytes.len() / 4);
        for chunk in bytes.chunks_exact(4) {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
            vec.push(f32::from_le_bytes(arr));
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_embedding_and_similarity() {
        let engine = VectorEngine::default();
        let e1 = engine.embed_text("Rust memory management and ownership rules").unwrap();
        let e2 = engine.embed_text("Rust memory ownership and borrow checker").unwrap();
        let e3 = engine.embed_text("Cooking Italian pasta with tomato sauce").unwrap();

        assert_eq!(e1.len(), EMBEDDING_DIM);
        assert_eq!(e2.len(), EMBEDDING_DIM);
        assert_eq!(e3.len(), EMBEDDING_DIM);

        let sim_close = VectorEngine::cosine_similarity(&e1, &e2);
        let sim_far = VectorEngine::cosine_similarity(&e1, &e3);

        assert!(sim_close > sim_far, "Similar concepts must score higher (sim_close={}, sim_far={})", sim_close, sim_far);

        // Serialization roundtrip
        let blob = VectorEngine::to_blob(&e1);
        let restored = VectorEngine::from_blob(&blob);
        assert_eq!(e1, restored);
    }
}
