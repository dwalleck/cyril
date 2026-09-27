//! cyril-ulhs oracle integration test: mirrors `crates/cyril-ui/tests/picker_active_marker.rs` —
//! NON-`#[test]` helper functions in a `tests/` crate calling `Option::expect` (the site cyril's
//! probe step A shows firing at picker line 70) and `Result::expect` with both a `&str` error and
//! the picker's exact `io::Error` type (the sites at picker lines 48/57 that stay silent), plus
//! the same forms directly inside a `#[test]` fn. Values come through `ulhs_oracle::value` so
//! `unnecessary_literal_unwrap` does not muddy the picture.

use std::io;

fn helper_option_expect(v: Option<u8>) -> u8 {
    v.expect("helper option")
}

fn helper_result_str_expect(v: Result<u8, &'static str>) -> u8 {
    v.expect("helper result str")
}

fn helper_result_io_expect(v: io::Result<u8>) -> u8 {
    v.expect("helper result io")
}

#[test]
fn via_helpers() {
    let v = Some(ulhs_oracle::value(Some(1)));
    assert_eq!(
        helper_option_expect(v),
        helper_result_str_expect(v.ok_or("none"))
    );
    assert_eq!(
        helper_result_io_expect(v.ok_or_else(|| io::Error::other("none"))),
        1
    );
}

#[test]
fn direct_in_test_fn() {
    let o = Some(ulhs_oracle::value(Some(1)));
    let r: Result<u8, &'static str> = o.ok_or("none");
    assert_eq!(o.expect("direct option"), r.expect("direct result"));
}
