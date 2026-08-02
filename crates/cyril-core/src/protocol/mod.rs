pub mod bridge;
pub(crate) mod client;
pub(crate) mod convert;
pub(crate) mod engine;
pub(crate) mod fingerprint;
pub(crate) mod identity;
/// KAS-engine support (free-path spawn discovery, auth responder). Gated behind
/// the `kas` cargo feature (ADR-0002); a default build links none of it.
#[cfg(feature = "kas")]
pub(crate) mod kas;
pub(crate) mod transport;
/// Turn mediation (cyril-b4y4). Test-only until slice 5 wires it into the
/// bridge `run_loop` — the module's first production consumer — keeping the
/// zero-`#[allow]` rule intact while the state machine is built and fenced.
#[cfg(test)]
pub(crate) mod turn_mediator;
