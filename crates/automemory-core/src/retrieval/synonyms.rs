//! Developer-vocabulary synonyms for keyword recall: a question says "login session", the note says "JWT". No model, no network.
//! A group lists words that mean nearly the same in project notes; a query word in a group also looks for the others.
//! Built-in groups are general; a project adds its own under `[retrieval] synonyms = [["billing", "invoice"], ...]`.

use std::sync::OnceLock;

/// General groups. Words are lowercase stems (what the query side produces); prefix matching covers the rest.
const DEFAULTS: &[&[&str]] = &[
    &["database", "db", "sql", "postgres", "sqlite", "mysql", "datastore", "orm"],
    &["auth", "login", "signin", "authentication", "session", "jwt", "token", "credential", "password"],
    &["admin", "backoffice", "ops", "operator"],
    &["delete", "remove", "drop", "erase"],
    &["error", "exception", "failure", "fail", "crash", "bug"],
    &["slow", "latency", "performance", "perf", "timeout"],
    &["config", "configuration", "setting", "flag", "env"],
    &["deploy", "release", "ship", "rollout", "publish"],
    &["test", "spec", "check", "assert"],
    &["queue", "broker", "job", "worker", "background"],
    &["cache", "redis", "memoize"],
    &["secret", "key", "token", "credential"],
    &["email", "mail", "smtp", "message"],
    &["image", "photo", "picture", "asset", "media"],
    &["log", "logging", "trace", "telemetry"],
    &["review", "approval", "approve", "pr"],
    &["limit", "quota", "throttle", "ratelimit"],
    &["pay", "payment", "charge", "billing", "invoice", "refund"],
];

static GROUPS: OnceLock<Vec<Vec<String>>> = OnceLock::new();

fn groups() -> &'static Vec<Vec<String>> {
    GROUPS.get_or_init(|| {
        let mut all: Vec<Vec<String>> = DEFAULTS.iter().map(|g| g.iter().map(|w| w.to_string()).collect()).collect();
        if let Ok(cfg) = crate::config::GlobalConfig::load() {
            all.extend(cfg.retrieval.synonyms.iter().map(|g| g.iter().map(|w| w.to_lowercase()).collect()));
        }
        all
    })
}

/// The words that share a group with `term` (not `term` itself), without repeats.
pub fn related(term: &str) -> Vec<&'static str> {
    // `AUTOMEMORY_NO_SYNONYMS=1` switches expansion off, to measure what it adds (`automemory eval recall`).
    if std::env::var_os("AUTOMEMORY_NO_SYNONYMS").is_some() {
        return Vec::new();
    }
    let mut out: Vec<&'static str> = Vec::new();
    for g in groups().iter().filter(|g| g.iter().any(|w| w == term)) {
        for w in g {
            if w != term && !out.contains(&w.as_str()) {
                out.push(w);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_finds_its_group_but_not_itself() {
        let r = related("database");
        assert!(r.contains(&"postgres") && !r.contains(&"database"));
        assert!(related("zzz-unknown").is_empty());
    }
}
