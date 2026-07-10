pub mod bridge;
pub(crate) mod client;
pub(crate) mod convert;
pub(crate) mod engine;
pub(crate) mod fingerprint;
/// KAS-engine support (free-path spawn discovery, auth responder). Gated behind
/// the `kas` cargo feature (ADR-0002); a default build links none of it.
#[cfg(feature = "kas")]
pub(crate) mod kas;
pub(crate) mod transport;
