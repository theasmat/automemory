//! The heuristics. Plain phrase matching on redacted chat text; each one is cautious (it would rather miss than guess).
use super::{Draft, DraftContext, Extractor};
use crate::continuity::ItemKind;
use regex::Regex;
use std::sync::OnceLock;

/// Sentences of 15..=300 characters.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut cur = String::new();
        let chars: Vec<char> = line.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            cur.push(*c);
            if matches!(c, '.' | '!' | '?') && chars.get(i + 1).map_or(true, |n| n.is_whitespace()) {
                out.push(std::mem::take(&mut cur));
            }
        }
        out.push(cur);
    }
    out.into_iter().map(|s| s.trim().trim_start_matches(['-', '*', '•', ' ']).trim().to_string()).filter(|s| (15..=300).contains(&s.chars().count())).collect()
}

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).unwrap())
}

/// Text before "because ..." and the reason after it.
fn split_reason(s: &str) -> (String, Option<String>) {
    static R: OnceLock<Regex> = OnceLock::new();
    match re(&R, r"(?i)\s*(?:,\s*)?\b(?:because|since|as it would|so\b)\s+").find(s) {
        Some(m) if m.start() > 10 && s.len() - m.end() > 5 => (s[..m.start()].trim_end_matches([',', ';']).to_string(), Some(s[m.end()..].trim_end_matches('.').to_string())),
        _ => (s.trim_end_matches('.').to_string(), None),
    }
}

/// Messages of one role across sessions, newest first, each message's sentences in order.
fn hits<'a>(ctx: &'a DraftContext, roles: &'a [&str], pat: &'a Regex) -> impl Iterator<Item = (String, String)> + 'a {
    ctx.chats.iter().flat_map(move |(s, msgs)| {
        msgs.iter().rev().filter(move |m| roles.contains(&m.role.as_str())).flat_map(move |m| sentences(&m.text).into_iter().filter(|x| pat.is_match(x)).map(move |x| (s.id.clone(), x)))
    })
}

fn draft(kind: ItemKind, (session, s): (String, String), split: bool) -> Draft {
    let (text, reason) = if split { split_reason(&s) } else { (s.trim_end_matches('.').to_string(), None) };
    Draft { kind, text, reason, session }
}

/// The first thing the person asked for in the newest session.
pub struct Goal;
impl Extractor for Goal {
    fn id(&self) -> &'static str { "goal" }
    fn cap(&self) -> usize { 1 }
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft> {
        for (s, msgs) in &ctx.chats {
            if let Some(m) = msgs.iter().find(|m| m.role == "user") {
                let first = sentences(&m.text).into_iter().next();
                if let Some(f) = first {
                    return vec![draft(ItemKind::Goal, (s.id.clone(), f), false)];
                }
            }
        }
        vec![]
    }
}

pub struct Constraints;
impl Extractor for Constraints {
    fn id(&self) -> &'static str { "constraints" }
    fn cap(&self) -> usize { 5 }
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft> {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = re(&R, r"(?i)^(keep it|keep the|must |only use|always |never |no [a-z]+ (framework|dependency|dependencies|library|libraries))|\b(must not|has to stay|no new dependenc)");
        hits(ctx, &["user"], r).map(|h| draft(ItemKind::Constraint, h, false)).collect()
    }
}

pub struct Rejections;
impl Extractor for Rejections {
    fn id(&self) -> &'static str { "rejections" }
    fn cap(&self) -> usize { 6 }
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft> {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = re(&R, r"(?i)\b(rule[sd]? (them |it |this |that )?out|ruled out|instead of|rather than|don'?t use|do not use|not going to use|let'?s not|scrap(ped)? (the|that|this)|drop(ped)? the (idea|approach|plan))\b");
        hits(ctx, &["user", "assistant"], r).map(|h| draft(ItemKind::Rejected, h, true)).collect()
    }
}

pub struct Decisions;
impl Extractor for Decisions {
    fn id(&self) -> &'static str { "decisions" }
    fn cap(&self) -> usize { 6 }
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft> {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = re(&R, r"(?i)\b(let'?s (use|go with|do)|we('ll| will| decided to| chose to) (use|go with|store|keep)|decided (to|on)|go(ing)? with|we chose|i chose)\b");
        // A sentence that also rules something out is already a rejection.
        hits(ctx, &["user"], r).filter(|(_, s)| !s.to_lowercase().contains("instead of") && !s.to_lowercase().contains("rather than")).map(|h| draft(ItemKind::Decision, h, true)).collect()
    }
}

pub struct Next;
impl Extractor for Next {
    fn id(&self) -> &'static str { "next" }
    fn cap(&self) -> usize { 5 }
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft> {
        static R: OnceLock<Regex> = OnceLock::new();
        static BOX: OnceLock<Regex> = OnceLock::new();
        let r = re(&R, r"(?i)^(next step|next up|todo|still to do|remaining)\s*:\s*|^\[ \]\s");
        let mut out: Vec<Draft> = hits(ctx, &["assistant"], r)
            .map(|(s, t)| (s, r.replace(&t, "").to_string()))
            .filter(|(_, t)| t.chars().count() >= 12)
            .map(|h| draft(ItemKind::Next, h, false))
            .collect();
        // Unchecked boxes in plan files the agent kept (Antigravity artifacts).
        let b = re(&BOX, r"^\[ \]\s+(.+)$");
        for (s, msgs) in &ctx.chats {
            for m in msgs.iter().rev().filter(|m| m.role == "artifact") {
                for line in m.text.lines().map(|l| l.trim().trim_start_matches(['-', '*']).trim()) {
                    if let Some(c) = b.captures(line) {
                        out.push(draft(ItemKind::Next, (s.id.clone(), c[1].trim().to_string()), false));
                    }
                }
            }
        }
        out
    }
}

/// A question the person asked last that nothing answered.
pub struct Open;
impl Extractor for Open {
    fn id(&self) -> &'static str { "open" }
    fn cap(&self) -> usize { 3 }
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft> {
        let mut out = Vec::new();
        for (s, msgs) in &ctx.chats {
            let Some(last) = msgs.iter().rev().find(|m| m.role == "user" || m.role == "assistant") else { continue };
            if last.role == "user" && last.text.trim_end().ends_with('?') {
                if let Some(q) = sentences(&last.text).into_iter().rev().find(|x| x.ends_with('?')) {
                    out.push(draft(ItemKind::Open, (s.id.clone(), q), false));
                }
            }
        }
        out
    }
}
