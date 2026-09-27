//! cyril-ulhs oracle bin: the NON-test `.expect()` sites (P2). They live in a bin target so
//! their lint errors cannot block compilation of the lib or of `tests/helper.rs`
//! (a failing lib build silently skips every test target — the first oracle draft's flaw).

/// P2 site — a NON-test `Option::expect`. Must lint with or without `allow-expect-in-tests`.
fn nontest_option_expect(v: Option<u8>) -> u8 {
    v.expect("oracle nontest option")
}

/// P2 site — a NON-test `Result::expect`.
fn nontest_result_expect(v: Result<u8, &'static str>) -> u8 {
    v.expect("oracle nontest result")
}

fn main() {
    let a = nontest_option_expect(std::env::args().next().map(|_| 1));
    let b = nontest_result_expect(std::env::args().next().map(|_| 1).ok_or("none"));
    println!("{}", a + b);
}
