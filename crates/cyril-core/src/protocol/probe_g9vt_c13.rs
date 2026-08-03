//! cyril-g9vt cheapest design falsifier (C13): the mediator ingress pattern —
//! a bounded mpsc whose item is UNINHABITED in a default build — compiles with
//! an exhaustively-empty match arm and drains as closed. This is the ADR-0002
//! posture the run_loop host-callback arm is designed on; if this failed to
//! compile or the recv arm misbehaved, the cfg strategy would be wrong before
//! any mediator code exists. Test-only module, both feature configs.

#![allow(clippy::unwrap_used)]

/// Stand-in for the default build's `HostCallback`: uninhabited, so a channel
/// of it can exist while no value can ever cross it.
enum NeverCallback {}

#[tokio::test]
async fn uninhabited_channel_arm_compiles_and_recv_is_none() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<NeverCallback>(8);
    drop(tx); // no producer can exist — a default build constructs none
    if let Some(cb) = rx.recv().await {
        // The match on the item type is exhaustive with ZERO arms — the
        // compile-time fact the run_loop arm relies on.
        match cb {}
    }
}
