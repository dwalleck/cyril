//! Fixed-width hexadecimal decoding and bounded text previews shared by every
//! identity, credential, and record type.

/// Decode `value` as exactly `N` bytes of hexadecimal.
///
/// Returns `None` for non-hex input or any other length; callers map that
/// into their own typed error. One definition keeps lesson ids, project ids,
/// content hashes, and the admin credential on the same parsing rule.
pub(crate) fn decode_fixed_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    hex::decode_to_slice(value, &mut bytes).ok()?;
    Some(bytes)
}

/// Cut `content` to at most `limit` Unicode scalar values.
///
/// Returns the retained text and the number of scalars dropped. A cut is
/// marked with a trailing `…` that counts against `limit`, so the result
/// never exceeds `limit` scalars; a zero limit retains nothing.
pub(crate) fn bounded_text(content: &str, limit: usize) -> (String, usize) {
    let total = content.chars().count();
    if total <= limit {
        return (content.to_owned(), 0);
    }
    if limit == 0 {
        return (String::new(), total);
    }
    let kept = limit - 1;
    let mut text: String = content.chars().take(kept).collect();
    text.push('…');
    (text, total - kept)
}

/// [`bounded_text`] without the dropped count, for previews.
pub(crate) fn bounded_preview(content: &str, limit: usize) -> String {
    bounded_text(content, limit).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_hex_rejects_wrong_length_and_non_hex() {
        assert_eq!(decode_fixed_hex::<2>("0a0b"), Some([0x0a, 0x0b]));
        assert_eq!(decode_fixed_hex::<2>("0a0b0c"), None);
        assert_eq!(decode_fixed_hex::<2>("0a0"), None);
        assert_eq!(decode_fixed_hex::<2>("zz0b"), None);
        assert_eq!(decode_fixed_hex::<0>(""), Some([]));
    }

    #[test]
    fn bounded_text_counts_scalars_and_marks_cuts() {
        assert_eq!(bounded_text("abc", 3), ("abc".to_owned(), 0));
        assert_eq!(bounded_text("abcd", 3), ("ab…".to_owned(), 2));
        assert_eq!(bounded_text("héllo wörld", 4), ("hél…".to_owned(), 8));
        assert_eq!(bounded_text("abc", 0), (String::new(), 3));
        assert_eq!(bounded_text("", 0), (String::new(), 0));
        assert_eq!(bounded_text("ab", 1), ("…".to_owned(), 2));
        assert_eq!(bounded_preview("abcd", 3), "ab…");
    }
}
