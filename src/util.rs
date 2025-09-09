use std::borrow::Cow;

pub fn truncate_str(s: &str, max_n: usize) -> Cow<'_, str> {
    max_n.checked_sub(1).map_or(Cow::Borrowed(""), |max_n| {
        let mut chars = s.char_indices();
        let n = chars.nth(max_n);
        let n_next = chars.next();
        match (n, n_next) {
            (Some((i, _)), Some(_)) => Cow::Owned(format!("{}…", &s[..i])),
            _ => Cow::Borrowed(s),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("おはよう。", 6), "おはよう。");
        assert_eq!(truncate_str("おはよう。", 5), "おはよう。");
        assert_eq!(truncate_str("おはよう。", 4), "おはよ…");
        assert_eq!(truncate_str("おはよう。", 2), "お…");
        assert_eq!(truncate_str("おはよう。", 1), "…");
        assert_eq!(truncate_str("おはよう。", 0), "");
    }
}
