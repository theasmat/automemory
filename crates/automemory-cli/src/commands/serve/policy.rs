//! Who may do what over HTTP. Every route that changes something has a tier here, and a test fails when a
//! write route is added without one.
//!
//! * `Safe`: harmless in anyone's hands (read-only, derived from verified data, local, rebuildable). The
//!   dashboard API listens on localhost, so an agent with a shell can call these; keep them harmless.
//! * `Protected`: creates trusted knowledge, changes what agents may do, or sends data off the machine.
//!   The human password is required every time (see `require_human_header`).
//! * `Effect`: the handler decides by what it would do (a local model is Safe, a cloud model is Protected).
//! * Terminal-only actions (password, recovery phrase, `init`, `serve`, ...) have no route at all.

use axum::{response::IntoResponse, Json};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Safe,
    Protected,
    Effect,
}

impl Tier {
    fn label(self) -> &'static str {
        match self {
            Tier::Safe => "safe",
            Tier::Protected => "protected",
            Tier::Effect => "by effect",
        }
    }
}

/// (path as registered in the router, tier, what it does)
pub const WRITE_ROUTES: &[(&str, Tier, &str)] = &[
    ("/api/onboarding", Tier::Safe, "tour and welcome progress"),
    ("/api/demo", Tier::Protected, "load or remove sample data"),
    ("/api/mode", Tier::Protected, "safety mode"),
    ("/api/memories", Tier::Protected, "add a memory"),
    ("/api/library/file", Tier::Protected, "add, replace or remove a library file"),
    ("/api/library/revert", Tier::Protected, "restore an older version of a library file"),
    ("/api/library/sync", Tier::Safe, "write library copies and read kept-in-place files"),
    ("/api/library/link", Tier::Protected, "choose which files the library keeps in sync in place"),
    ("/api/continuity/import", Tier::Safe, "import new chat messages from agents already turned on"),
    ("/api/continuity/handoff", Tier::Safe, "write a hand-off with the brief preview"),
    ("/api/continuity/source/:agent", Tier::Protected, "allow or stop reading one agent's chats"),
    ("/api/continuity/history-sync/push", Tier::Protected, "send chat history to the sync remote"),
    ("/api/continuity/history-sync/pull", Tier::Safe, "fetch chat history from the sync remote"),
    ("/api/continuity/history", Tier::Protected, "delete imported chat history"),
    ("/api/continuity/items", Tier::Protected, "add to the brief"),
    ("/api/continuity/items/:id", Tier::Protected, "edit a brief item"),
    ("/api/continuity/items/:id/:action", Tier::Protected, "confirm, resolve or remove a brief item"),
    ("/api/rooms", Tier::Protected, "start a planning room"),
    ("/api/rooms/:id/note", Tier::Protected, "add a note to a planning room"),
    ("/api/rooms/:id/draft", Tier::Protected, "save a draft plan for a planning room"),
    ("/api/rooms/:id/decide", Tier::Protected, "decide a planning room yourself"),
    ("/api/rooms/:id/approve", Tier::Protected, "approve a planning room's plan"),
    ("/api/rooms/:id/reject", Tier::Protected, "reject a planning room's plan"),
    ("/api/rooms/:id/another-round", Tier::Protected, "send a planning room back for another round"),
    ("/api/rooms/:id/revise", Tier::Protected, "revise an approved planning room"),
    ("/api/rooms/:id/skip", Tier::Protected, "skip a stalled turn in a planning room"),
    ("/api/rooms/:id/end", Tier::Protected, "end a planning room without a plan"),
    ("/api/tasks/:id", Tier::Protected, "edit or cancel a room task"),
    ("/api/memories/archive_many", Tier::Protected, "archive several memories"),
    ("/api/memories/:id/archive", Tier::Protected, "archive a memory"),
    ("/api/memories/:id/update", Tier::Protected, "edit a memory"),
    ("/api/memories/:id/pin", Tier::Protected, "pin a memory as an invariant"),
    ("/api/memories/:id/veto", Tier::Protected, "veto a memory"),
    ("/api/memories/:id/restore", Tier::Protected, "restore an archived memory"),
    ("/api/memories/:id/revoke", Tier::Protected, "revoke a memory"),
    ("/api/decisions", Tier::Protected, "record a decision"),
    ("/api/failures/:id/resolve", Tier::Protected, "mark a problem fixed"),
    ("/api/sessions/:id/distill", Tier::Safe, "summarise a session"),
    ("/api/proposals/decide", Tier::Protected, "approve or reject a suggestion"),
    ("/api/proposals/decide_all", Tier::Protected, "approve or reject all suggestions"),
    ("/api/guardrails/rules", Tier::Protected, "add a blocking rule"),
    ("/api/guardrails/rules/:id", Tier::Protected, "delete a blocking rule"),
    ("/api/guardrails/rules/:id/toggle", Tier::Protected, "turn a rule on or off"),
    ("/api/guardrails/evaluate", Tier::Safe, "try a command against the rules"),
    ("/api/sync/trigger", Tier::Protected, "sync now (sends data to the remote)"),
    ("/api/sync/pull", Tier::Safe, "pull from the remote"),
    ("/api/llm", Tier::Protected, "LLM helper settings"),
    ("/api/llm/profile/:name", Tier::Protected, "remove an LLM profile"),
    ("/api/llm/key", Tier::Protected, "store or remove the LLM API key"),
    ("/api/llm/run", Tier::Effect, "run an LLM suggestion job (cloud sends text)"),
    ("/api/llm/run/cancel", Tier::Safe, "stop the LLM job"),
    ("/api/recall", Tier::Protected, "search mode and active model"),
    ("/api/recall/profile", Tier::Protected, "save a model profile"),
    ("/api/recall/profile/:name", Tier::Protected, "remove a model profile"),
    ("/api/recall/key", Tier::Protected, "store or remove an API key"),
    ("/api/recall/test", Tier::Effect, "test the model (cloud sends text)"),
    ("/api/recall/index", Tier::Effect, "index memories (cloud sends text)"),
    ("/api/recall/index/cancel", Tier::Safe, "stop indexing"),
    ("/api/recall/clean", Tier::Safe, "remove unused vectors"),
    ("/api/jobs/:id/cancel", Tier::Safe, "stop a running job"),
    ("/api/tools/doctor", Tier::Safe, "health check"),
    ("/api/tools/export-rules", Tier::Safe, "write the project's instruction files"),
    ("/api/tools/handoff", Tier::Safe, "write a hand-off"),
    ("/api/tools/docs-rescan", Tier::Safe, "rescan tracked documents"),
    ("/api/tools/eval", Tier::Safe, "run the eval suites"),
    ("/api/tools/backup", Tier::Safe, "back up the database"),
    ("/api/agents/connect", Tier::Protected, "connect an agent in this project"),
    ("/api/agents/disconnect", Tier::Protected, "remove an agent's registration"),
    ("/api/locks/release", Tier::Protected, "release a lock"),
    ("/api/locks/stop", Tier::Protected, "kill switch: release every lock"),
    ("/api/settings/locks", Tier::Protected, "lock settings"),
];

/// GET /api/policy: the table, for the lock icons in the dashboard.
pub async fn api_policy() -> impl IntoResponse {
    Json(json!(WRITE_ROUTES.iter().map(|(p, t, d)| json!({ "path": p, "tier": t.label(), "what": d })).collect::<Vec<_>>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route that can change something must be listed with a tier.
    #[test]
    fn every_write_route_has_a_tier() {
        let src = include_str!("routes/mod.rs");
        let mut missing = Vec::new();
        let mut seen = 0;
        for chunk in src.split(".route(").skip(1) {
            let Some(path) = chunk.split('"').nth(1) else { continue };
            // The route's own handlers end at the first closing `)` that balances the opening one.
            let mut depth = 1i32;
            let mut end = chunk.len();
            for (i, c) in chunk.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let handlers = &chunk[..end];
            let writes = ["post(", "put(", "delete(", "patch("].iter().any(|m| handlers.contains(m));
            if writes {
                seen += 1;
            }
            if writes && !WRITE_ROUTES.iter().any(|(p, _, _)| *p == path) {
                missing.push(path.to_string());
            }
        }
        assert!(seen >= 35, "the route scan found only {seen} write routes: the parser needs a fix");
        assert!(missing.is_empty(), "write routes without a tier in serve/policy.rs: {missing:?}");
    }

    #[test]
    fn safe_routes_never_create_trusted_knowledge() {
        for (path, tier, what) in WRITE_ROUTES {
            if *tier == Tier::Safe {
                for banned in ["memor", "decision", "approve", "mode", "rule", "key", "connect"] {
                    assert!(!what.contains(banned) || *what == "try a command against the rules" || *what == "summarise a session",
                        "{path} ({what}) is Safe but sounds like it creates trust or changes safety");
                }
            }
        }
    }
}
