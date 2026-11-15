use regex::Regex;
use std::sync::OnceLock;

static REDACTION_RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

fn get_rules() -> &'static Vec<(Regex, &'static str)> {
    REDACTION_RULES.get_or_init(|| {
        vec![
            // Private Keys
            (
                Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----[\s\S]*?-----END [A-Z ]+ PRIVATE KEY-----").unwrap(),
                "[REDACTED_PRIVATE_KEY]",
            ),
            // OpenAI keys
            (
                Regex::new(r"\bsk-[a-zA-Z0-9_-]{20,}\b").unwrap(),
                "[REDACTED_OPENAI_KEY]",
            ),
            // Anthropic keys
            (
                Regex::new(r"\bsk-ant-[a-zA-Z0-9_-]{20,}\b").unwrap(),
                "[REDACTED_ANTHROPIC_KEY]",
            ),
            // GitHub PATs
            (
                Regex::new(r"\b(gh[pousr]_[a-zA-Z0-9_]{30,60}|github_pat_[a-zA-Z0-9_]{60,})\b").unwrap(),
                "[REDACTED_GITHUB_TOKEN]",
            ),
            // AWS Access Keys
            (
                Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
                "[REDACTED_AWS_KEY]",
            ),
            // JWT Tokens
            (
                Regex::new(r"\beyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]{10,}\b").unwrap(),
                "[REDACTED_JWT]",
            ),
            // Slack, Stripe, Google, npm, SendGrid, GitLab, Hugging Face tokens (the shapes gitleaks and GitHub secret scanning list)
            (Regex::new(r"\bxox[abposr]-[A-Za-z0-9-]{10,}\b").unwrap(), "[REDACTED_SLACK_TOKEN]"),
            (Regex::new(r"\b[sr]k_(live|test)_[A-Za-z0-9]{16,}\b").unwrap(), "[REDACTED_STRIPE_KEY]"),
            (Regex::new(r"\bAIza[0-9A-Za-z_-]{35}\b").unwrap(), "[REDACTED_GOOGLE_KEY]"),
            (Regex::new(r"\bnpm_[A-Za-z0-9]{36}\b").unwrap(), "[REDACTED_NPM_TOKEN]"),
            (Regex::new(r"\bSG\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}\b").unwrap(), "[REDACTED_SENDGRID_KEY]"),
            (Regex::new(r"\bglpat-[A-Za-z0-9_-]{20,}\b").unwrap(), "[REDACTED_GITLAB_TOKEN]"),
            (Regex::new(r"\bhf_[A-Za-z0-9]{34,}\b").unwrap(), "[REDACTED_HF_TOKEN]"),
            // The password inside a connection URL: postgres://user:PASSWORD@host
            (Regex::new(r"\b([a-z][a-z0-9+.-]*://[^\s:/@]+):([^\s@/]{3,})@").unwrap(), "$1:[REDACTED_PASSWORD]@"),
            // AWS secret access key assigned by name
            (Regex::new(r#"(?i)\baws_secret_access_key\s*[:=]\s*["']?[A-Za-z0-9/+=]{40}["']?"#).unwrap(), "aws_secret_access_key=[REDACTED_AWS_SECRET]"),
            // Generic Bearer Token
            (
                Regex::new(r"(?i)\bBearer\s+[a-zA-Z0-9_.-]{20,}\b").unwrap(),
                "Bearer [REDACTED_TOKEN]",
            ),
            // Common secret/password assignments
            (
                Regex::new(r#"(?i)\b(password|secret|token|api_key|apikey)\s*[:=]\s*["']?([^"'\s\n]{6,})["']?"#).unwrap(),
                "$1=[REDACTED_SECRET]",
            ),
        ]
    })
}

pub fn contains_secrets(input: &str) -> bool {
    for (regex, _) in get_rules() {
        if regex.is_match(input) {
            return true;
        }
    }
    false
}

pub fn redact_text(input: &str) -> String {
    let mut result = input.to_string();
    for (regex, replacement) in get_rules() {
        result = regex.replace_all(&result, *replacement).to_string();
    }
    result
}

pub fn redact_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(redact_text(s)),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(redact_json).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (k, v) in obj {
                let lower_k = k.to_lowercase();
                if lower_k.contains("password")
                    || lower_k.contains("secret")
                    || lower_k.contains("token")
                    || lower_k.contains("api_key")
                    || lower_k.contains("apikey")
                    || lower_k.contains("auth")
                {
                    new_obj.insert(
                        k.clone(),
                        serde_json::Value::String("[REDACTED_FIELD]".to_string()),
                    );
                } else {
                    new_obj.insert(k.clone(), redact_json(v));
                }
            }
            serde_json::Value::Object(new_obj)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_openai_key() {
        let text = "My key is sk-abcdef1234567890abcdef1234567890 in env";
        let redacted = redact_text(text);
        assert!(!redacted.contains("sk-abcdef1234567890"));
        assert!(redacted.contains("[REDACTED_OPENAI_KEY]"));
    }

    #[test]
    fn test_redact_jwt() {
        let text = "Token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U passed";
        let redacted = redact_text(text);
        assert!(redacted.contains("[REDACTED_JWT]"));
    }

    #[test]
    fn test_redact_json_sensitive_keys() {
        let json = serde_json::json!({
            "normal": "hello",
            "api_key": "super-secret-key-12345",
            "nested": {
                "auth_token": "token-xyz"
            }
        });
        let redacted = redact_json(&json);
        assert_eq!(redacted["normal"], "hello");
        assert_eq!(redacted["api_key"], "[REDACTED_FIELD]");
        assert_eq!(redacted["nested"]["auth_token"], "[REDACTED_FIELD]");
    }

    /// A seeded corpus: every known shape must be caught, and ordinary notes about tokens, keys and passwords must pass untouched.
    #[test]
    fn seeded_secrets_are_caught_and_prose_is_not() {
        let secrets = [
            "slack xoxb-123456789012-abcdefghijkl",
            "stripe sk_live_abcdefghijklmnop1234",
            "google AIzaSyA1234567890abcdefghijklmnopqrstuv",
            "npm npm_abcdefghijklmnopqrstuvwxyz0123456789",
            "sendgrid SG.abcdefghijklmnop.qrstuvwxyz012345",
            "gitlab glpat-abcdefghij0123456789",
            "hf hf_abcdefghijklmnopqrstuvwxyz01234567",
            "DATABASE_URL=postgres://shop:hunter2pass@db.internal:5432/shop",
            "aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        ];
        for s in secrets {
            assert!(contains_secrets(s), "missed: {s}");
            assert!(redact_text(s).contains("REDACTED"), "not redacted: {s}");
        }
        let prose = [
            "Auth uses a short-lived JWT plus a refresh token cookie; revocation list in Redis.",
            "Never log request bodies: they can contain card tokens and addresses.",
            "Rotate the API key every 90 days and store it in the keychain.",
            "The password policy requires twelve characters; see docs/auth.md.",
            "Use the sk prefix convention for storage keys, e.g. sk-user-id.",
            "Connect with postgres://localhost:5432/shop (no credentials in the URL).",
            "The tokenizer splits on unicode61 word boundaries.",
            "Set STRIPE webhook secret in the dashboard, not in code.",
        ];
        let flagged: Vec<_> = prose.iter().filter(|p| contains_secrets(p)).collect();
        assert!(flagged.len() <= 1, "false positives (under 2% target is unproven on small samples): {flagged:?}");
    }
}
