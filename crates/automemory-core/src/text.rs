//! Small text helpers shared across the core.

/// Trimmed text cut to `max` characters with a trailing `…` when it was longer.
pub fn clip(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max { t.to_string() } else { format!("{}…", t.chars().take(max).collect::<String>().trim_end()) }
}

#[cfg(test)]
mod tests {
    use super::clip;

    #[test]
    fn clip_trims_and_marks_the_cut() {
        assert_eq!(clip("  short  ", 20), "short");
        assert_eq!(clip("abcdefghij", 5), "abcde…");
        assert_eq!(clip("abcde  fghij", 6), "abcde…", "no trailing space before the mark");
    }
}
