pub fn truncate_str(s: &str, max_n: usize) -> String {
    const DOTS: &str = "...";
    let s_n = s.chars().count();
    let (n, max_n) = if s_n > max_n {
        (max_n.saturating_sub(DOTS.chars().count()), max_n)
    } else {
        (s_n, s_n)
    };
    s.chars().take(n).chain(DOTS.chars()).take(max_n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("おはよう。", 6), "おはよう。");
        assert_eq!(truncate_str("おはよう。", 5), "おはよう。");
        assert_eq!(truncate_str("おはよう。", 4), "お...");
        assert_eq!(truncate_str("おはよう。", 3), "...");
        assert_eq!(truncate_str("おはよう。", 2), "..");
        assert_eq!(truncate_str("おはよう。", 1), ".");
        assert_eq!(truncate_str("おはよう。", 0), "");
    }
}
