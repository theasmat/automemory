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
                Regex::new(r"\b(ghp_[a-zA-Z0-9]{36}|github_pat_[a-zA-Z0-9_]{60,})\b").unwrap(),
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
                    new_obj.insert(k.clone(), serde_json::Value::String("[REDACTED_FIELD]".to_string()));
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
}
