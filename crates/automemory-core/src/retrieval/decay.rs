use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDecayConfig {
    pub half_life_days: f64,
    pub access_boost_weight: f64,
    pub stale_penalty: f64,
    pub min_retrieval_threshold: f64,
}

impl Default for MemoryDecayConfig {
    fn default() -> Self {
        Self {
            half_life_days: 14.0,
            access_boost_weight: 0.25,
            stale_penalty: 0.80,
            min_retrieval_threshold: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayEvaluation {
    pub original_importance: f64,
    pub effective_score: f64,
    pub days_since_update: f64,
    pub time_decay_factor: f64,
    pub usage_boost_factor: f64,
    pub is_stale: bool,
    pub staleness_reason: Option<String>,
}

pub struct MemoryDecayScorer {
    config: MemoryDecayConfig,
}

impl Default for MemoryDecayScorer {
    fn default() -> Self {
        Self::new(MemoryDecayConfig::default())
    }
}

impl MemoryDecayScorer {
    pub fn new(config: MemoryDecayConfig) -> Self {
        Self { config }
    }

    /// Computes composite decay score for a memory record based on age, usage frequency, and contradiction flags.
    pub fn evaluate(
        &self,
        importance: f64,
        confidence: f64,
        updated_at: DateTime<Utc>,
        access_count: u32,
        is_contradicted: bool,
        contradiction_reason: Option<String>,
    ) -> DecayEvaluation {
        let now = Utc::now();
        let duration = now.signed_duration_since(updated_at);
        let days = (duration.num_seconds() as f64 / 86400.0).max(0.0);

        // Exponential decay: e^(-lambda * days), where lambda = ln(2) / half_life
        let lambda = 0.69314718056 / self.config.half_life_days;
        let time_decay = (-lambda * days).exp().clamp(0.01, 1.0);

        // Access boost: 1 + alpha * ln(1 + count)
        let usage_boost = 1.0 + self.config.access_boost_weight * (1.0 + access_count as f64).ln();

        let mut effective =
            (importance.clamp(0.0, 1.0)) * (confidence.clamp(0.1, 1.0)) * time_decay * usage_boost;

        if is_contradicted {
            effective *= 1.0 - self.config.stale_penalty;
        }

        DecayEvaluation {
            original_importance: importance,
            effective_score: effective.clamp(0.0, 1.0),
            days_since_update: days,
            time_decay_factor: time_decay,
            usage_boost_factor: usage_boost,
            is_stale: is_contradicted || effective < self.config.min_retrieval_threshold,
            staleness_reason: contradiction_reason,
        }
    }

    /// Scans memory content against known current codebase context to detect architectural contradictions.
    pub fn detect_contradiction(
        &self,
        memory_content: &str,
        current_context: &[&str],
    ) -> (bool, Option<String>) {
        let content_lower = memory_content.to_lowercase();

        // 1. Framework version contradiction patterns
        let version_checks = [
            ("vue 2", &["vue 3", "vue: ^3", "\"vue\": \"^3"][..]),
            (
                "react 17",
                &[
                    "react 18",
                    "react 19",
                    "\"react\": \"^18",
                    "\"react\": \"^19",
                ][..],
            ),
            ("react 18", &["react 19", "\"react\": \"^19"][..]),
            ("nuxt 2", &["nuxt 3", "\"nuxt\": \"^3"][..]),
            ("webpack", &["vite", "turbopack", "\"vite\": "][..]),
            ("npm", &["pnpm", "yarn", "bun"][..]),
            ("commonjs", &["esm", "\"type\": \"module\""][..]),
        ];

        for (legacy_term, modern_matches) in version_checks {
            if content_lower.contains(legacy_term) {
                for &modern in modern_matches {
                    for ctx in current_context {
                        if ctx.to_lowercase().contains(modern) {
                            return (
                                true,
                                Some(format!(
                                    "Memory references obsolete '{}', but project context indicates '{}'",
                                    legacy_term, modern
                                )),
                            );
                        }
                    }
                }
            }
        }

        (false, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_fresh_memory_evaluation() {
        let scorer = MemoryDecayScorer::default();
        let eval = scorer.evaluate(0.9, 0.95, Utc::now(), 5, false, None);

        assert!(eval.effective_score > 0.8);
        assert!(!eval.is_stale);
        assert!(eval.usage_boost_factor > 1.0);
    }

    #[test]
    fn test_aged_and_contradicted_memory() {
        let scorer = MemoryDecayScorer::default();
        let past = Utc::now() - Duration::days(60); // 4+ half-lives
        let eval = scorer.evaluate(
            0.8,
            0.9,
            past,
            0,
            true,
            Some("Contradicts active stack".to_string()),
        );

        assert!(eval.effective_score < 0.1);
        assert!(eval.is_stale);
    }

    #[test]
    fn test_framework_contradiction_detection() {
        let scorer = MemoryDecayScorer::default();
        let mem = "We are using Vue 2 with Options API for the dashboard.";
        let current_ctx = vec!["package.json: \"nuxt\": \"^3.21.0\", \"vue\": \"^3.5.43\""];

        let (is_contra, reason) = scorer.detect_contradiction(mem, &current_ctx);
        assert!(is_contra);
        assert!(reason.unwrap().to_lowercase().contains("vue 2"));
    }
}
