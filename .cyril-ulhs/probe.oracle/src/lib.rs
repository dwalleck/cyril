//! cyril-ulhs oracle library. The non-test production part is lint-clean so the lib builds and
//! every test target gets compiled. Test-shaped sites:
//!   - `tests` — `#[cfg(test)]` module carrying `#[expect(clippy::expect_used)]` (P3, the shape
//!     used at 17 sites in cyril);
//!   - `plain_tests` — `#[cfg(test)]` module with no attribute, both receiver forms (P1 comparison).

/// Lint-clean production function so the lib target compiles under `-D warnings`.
pub fn value(v: Option<u8>) -> u8 {
    v.unwrap_or_default()
}

/// P3 site — with `allow-expect-in-tests` the lint never fires inside, so the expectation should
/// go unfulfilled (`unfulfilled_lint_expectations`).
#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    #[test]
    fn cfg_test_option_expect() {
        let v: Option<u8> = super::value(Some(1)).checked_add(0);
        assert_eq!(v.expect("in cfg(test) mod"), 1);
    }
}

/// P1 comparison site — a `#[cfg(test)]` module WITHOUT any attribute, using both forms.
#[cfg(test)]
mod plain_tests {
    #[test]
    fn cfg_test_both_forms() {
        let o: Option<u8> = super::value(Some(1)).checked_add(0);
        let r: Result<u8, &'static str> = o.ok_or("none");
        assert_eq!(
            o.expect("plain cfg(test) option"),
            r.expect("plain cfg(test) result")
        );
    }
}
