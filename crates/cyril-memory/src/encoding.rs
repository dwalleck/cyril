//! Fixed-width hexadecimal decoding shared by every identity and credential type.

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
}
