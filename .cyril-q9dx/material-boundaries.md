# Material-boundary inventory — cyril-q9dx

## Smallest question

Can the actual Cyril theme and rendering seams emit user as LightBlue, agent as
LightGreen, and system as LightMagenta in ANSI-16 while preserving every other
resolved role?

## Inventory

<!-- markdownlint-disable MD013 -->

| Category | Material? | Falsifying observation | Evidence surface |
| --- | --- | --- | --- |
| Representation and normalization | Yes | Applying the three semantic assignments changes any of the other 26 ANSI-16 roles, or a Ratatui named color does not survive as the expected `Color` variant. | Public `theme::resolve` output compared role by role with an independent specification-driven oracle. |
| Selection and visibility | Yes | The conversation renderer does not bind visible user, agent, and system labels to `theme.user`, `theme.agent`, and `theme.system`, or clipping removes a label from the pinned scene. | Ratatui `TestBackend` buffer for the production-shape 80×24 identity scene. |
| Mutable shared state | No | Speaker labels are rendered directly from the resolved `Theme`; the Markdown and syntax caches do not produce those labels. Cache-identity structure is owned by `cyril-x5xi`. | Excluded by `.cyril-q9dx/spec.md` and the renderer call path. |
| Ordering and concurrency | No | Theme resolution and one frame render are synchronous pure reads; no producer ordering, retry, cancellation, or interleaving changes speaker-role identity. | Excluded by `theme.rs:204-238` and the single-frame renderer seam. |
| Transport and serialization | No separate boundary | The feature adds no file, protocol, or process transport. The observable output is the in-process Ratatui buffer covered below. | Excluded by feature scope. |
| External library semantics | Yes | Ratatui `TestBackend` stores a different foreground than the named color supplied by the resolved theme. | Buffer-cell foreground inspection, independently compared with direct role-to-label source bindings. |

<!-- markdownlint-enable MD013 -->

The resolver probe covers representation and normalization. The rendering probe
covers selection, visibility, and Ratatui's observable buffer representation.
