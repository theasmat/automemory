//! Deterministic summaries (no model): L0 is a title and one sentence, L1 an outline with the lines that matter most.
const L0_CHARS: usize = 400;
const L1_CHARS: usize = 3200;

use crate::text::clip;

fn heading(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes).then(|| t[hashes..].trim()).filter(|h| !h.is_empty())
}

fn first_sentence(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.find(|c| matches!(c, '.' | '!' | '?')) {
        Some(i) if i + 1 >= 8 => flat[..=i].to_string(),
        _ => flat,
    }
}

/// Title (first heading, else the file name) and the first sentence of the first paragraph that is not a heading.
pub fn l0(path: &str, content: &str) -> String {
    let title = content.lines().find_map(heading).map(String::from).unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string());
    let para = content.split("\n\n").map(str::trim).find(|p| !p.is_empty() && heading(p.lines().next().unwrap_or("")).is_none() && !p.starts_with("```") && !p.starts_with("---"));
    match para {
        Some(p) => clip(&format!("{title}: {}", first_sentence(p)), L0_CHARS),
        None => clip(&title, L0_CHARS),
    }
}

/// Headings with the first line under each, plus every line that states a rule or a to-do.
pub fn l1(content: &str) -> String {
    let mut out = String::new();
    let mut after_heading = false;
    let key = |l: &str| {
        let t = l.trim_start_matches(['-', '*', ' ']).to_lowercase();
        ["decision", "decided", "rejected", "must", "todo", "[ ]", "do not", "never", "next step"].iter().any(|k| t.starts_with(k))
    };
    for line in content.lines() {
        let t = line.trim();
        if let Some(h) = heading(t) {
            out.push_str(&format!("# {h}\n"));
            after_heading = true;
        } else if !t.is_empty() && !t.starts_with("```") && (after_heading || key(t)) {
            out.push_str(&format!("  {}\n", clip(t, 200)));
            after_heading = false;
        }
        if out.len() >= L1_CHARS {
            break;
        }
    }
    clip(&out, L1_CHARS)
}

/// Pieces for search: one per heading section (long sections are split at about 1500 characters). (heading, text)
pub fn chunks(content: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur = (String::new(), String::new());
    let flush = |cur: &mut (String, String), out: &mut Vec<(String, String)>| {
        if !cur.1.trim().is_empty() {
            out.push((cur.0.clone(), std::mem::take(&mut cur.1)));
        }
        cur.1.clear();
    };
    for line in content.lines() {
        if let Some(h) = heading(line) {
            flush(&mut cur, &mut out);
            cur.0 = h.to_string();
        }
        cur.1.push_str(line);
        cur.1.push('\n');
        if cur.1.len() > 1500 && line.trim().is_empty() {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Cart PRD\n\nA shopper keeps items between visits. Carts expire after 30 days.\n\n## Decisions\n- Decision: store carts in SQLite\n- Rejected: server sessions\n\n## Open\nWhat about guests?\n\n- TODO: write the cleanup job\n";

    #[test]
    fn l0_is_a_title_and_one_sentence() {
        assert_eq!(l0("plans/cart.md", DOC), "Cart PRD: A shopper keeps items between visits.");
        assert_eq!(l0("notes/api.txt", "just words without a heading"), "api.txt: just words without a heading");
        assert_eq!(l0("x.md", ""), "x.md");
    }

    #[test]
    fn l1_keeps_headings_and_the_lines_that_matter() {
        let t = l1(DOC);
        assert!(t.contains("# Cart PRD") && t.contains("# Decisions") && t.contains("Decision: store carts in SQLite") && t.contains("TODO: write the cleanup job"), "{t}");
        assert!(t.len() < DOC.len() + 40);
    }

    #[test]
    fn chunks_follow_headings() {
        let c = chunks(DOC);
        assert_eq!(c.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), vec!["Cart PRD", "Decisions", "Open"]);
    }
}
