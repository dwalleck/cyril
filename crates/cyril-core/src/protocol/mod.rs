pub mod bridge;
pub(crate) mod client;
pub(crate) mod convert;
pub(crate) mod engine;
pub(crate) mod fingerprint;
/// Host-callback mediation (cyril-g9vt): the pure state machine behind the
/// bridge `run_loop`'s host-callback arm. See CONTEXT.md "Host-callback
/// mediation".
pub(crate) mod host_mediator;
pub(crate) mod identity;
/// KAS-engine support (free-path spawn discovery, auth responder). Gated behind
/// the `kas` cargo feature (ADR-0002); a default build links none of it.
#[cfg(feature = "kas")]
pub(crate) mod kas;
/// prove-it probe for cyril-dn91 (engine-vs-feature gating of host callbacks).
/// Characterization tests — see the module doc; repurposed into regression
/// fences by the cyril-dn91 build.
#[cfg(all(test, feature = "kas"))]
mod probe_dn91;
/// cyril-g9vt cheapest design falsifier (C13) — the uninhabited-channel arm
/// pattern the mediator ingress relies on. Test-only, BOTH feature configs
/// (the default build is the point).
#[cfg(test)]
mod probe_g9vt_c13;
pub(crate) mod transport;
/// Turn mediation (cyril-b4y4): the pure state machine behind the bridge
/// `run_loop`'s turn ownership — busy-guard, owner allocation, terminal
/// dispositions, companion ledger. See CONTEXT.md "Turn mediation".
pub(crate) mod turn_mediator;
