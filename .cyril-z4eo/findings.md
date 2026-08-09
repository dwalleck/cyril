# cyril-z4eo prove-it-prototype findings

## Smallest question

Given two live `PermissionRequest` values shown before either is resolved, which request remains visible and which responder receives a terminal reply?

## Probe

`.cyril-z4eo/probe.rs` calls the public `UiState::show_approval` and `approval_confirm` APIs with two production-shape requests and real Tokio oneshot responders. `.cyril-z4eo/run-probe.sh` compiles it as a temporary `cyril-ui` example against the actual workspace.

Observed output:

```text
head1=second
first_after_resolution=closed
second_after_resolution=selected
head2=none
```

## Oracle

`.cyril-z4eo/oracle.py` independently inspects the ownership operations in `state.rs`: `show_approval` assigns directly into the single slot and `approval_confirm` takes that slot. From those operations it predicts the visible head and both oneshot states without calling `UiState`.

Oracle output was byte-for-byte identical to the probe output:

```text
head1=second
first_after_resolution=closed
second_after_resolution=selected
head2=none
```

## Agreement

Probe and oracle agree on all four observations. The current implementation is LIFO replacement, not FIFO presentation: request 2 destroys request 1's only sender, request 1 closes without a human decision, request 2 resolves, and no request remains.

## What I learned

The existing `approval_snapshot_is_independent` regression test explicitly codifies the dropped first responder as desirable snapshot independence. The fix must rewrite that fence rather than merely add another test, while preserving its distinct request-time snapshot invariant.

## Design constraints exposed by the probe

- Queue promotion belongs on terminal confirm and terminal cancel paths.
- Entering trust-selection phase and Esc back to option selection are not terminal; they must retain the same queue head.
- `approval()` and rendering can continue to expose only the active head, keeping modal priority unchanged.
- The queued `ApprovalState` values must retain ownership of every oneshot sender until each request is answered or `UiState` itself is dropped.
