pub mod bridge;
#[expect(dead_code, reason = "will be consumed by bridge loop in a later task")]
pub(crate) mod convert;
#[expect(dead_code, reason = "will be consumed by bridge loop in a later task")]
pub(crate) mod transport;
