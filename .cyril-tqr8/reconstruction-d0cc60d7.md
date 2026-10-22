## Summary and source boundary

The change adds two distinct input paths into existing notification/transcript infrastructure:

- A **global external-URL host callback**, acknowledged only after bounded domain-queue acceptance.
- A **session-scoped MCP connection-error notification**, retained as transcript content rather than interpreted as turn completion.

Presentation code escapes terminal controls, reveals new notices in main history, preserves canonical scoped errors in child/workflow streams, and reserves a separate scrollbar column.

This is a production-source reconstruction only. **No design-conformance verdict, runtime acceptance, or issue-completion judgment is made.**

### Exact refs and scope

Observed with `GIT_OPTIONAL_LOCKS=0`:

| Item | Observed value |
|---|---|
| Local branch | `fix/cyril-tqr8` |
| HEAD | `d0cc60d7662ced69a4a32c904e9800bea6990a22` |
| Configured remote default | `refs/remotes/origin/main` |
| `origin/main` | `03838a501dad8be140acd78dc3fff4cae35fafef` |
| HEAD/default merge-base | `03838a501dad8be140acd78dc3fff4cae35fafef` |
| Configured upstream | `origin/fix/cyril-tqr8` |
| Upstream commit | `d0cc60d7662ced69a4a32c904e9800bea6990a22` |

I derived the file set from the merge-base-to-HEAD Git diff, not an artifact allowlist. The unrestricted changed-path census showed no other production-source directories beyond the `crates/` changes below.

There are **11 changed production behavior/export files**, plus `crates/cyril/src/app.rs`, whose sole change is test-module registration. Six additional changed paths are dedicated tests/test probes.

No tracked source drift was reported by `git diff HEAD -- crates experiments Cargo.toml Cargo.lock`; no nonignored untracked paths were reported under `crates/` or `experiments/`. These are observations at inspection time, not a continuing freeze.

No builds, tests, linters, formatters, applications, authentication probes, or Git mutations were run. No prohibited artifact/document contents were read. Available tools exposed no LSP/AST interface; reconstruction used Git diffs, literal searches, and line-anchored source reads.

## 1. Changed interfaces and reconstructed ledger

Paths are relative to the target worktree. “Interface owner” identifies the production component owning the boundary, not an approved-design ledger designation.

| Path | Responsibility and changed interface | Interface owner |
|---|---|---|
| `crates/cyril-core/src/types/event.rs` | Adds public domain-event variants `ExternalUrlRequested { url: String }` and `McpConnectionError { message: String }`. Neither carries embedded session/server identity; routing identity belongs to `RoutedNotification`. Evidence: `:209-218`, `:502-504`, `:566-582`. | Core notification/envelope model |
| `crates/cyril-core/src/protocol/engine.rs` | Extends `Adapters` with optional `external_url: Option<ExternalUrlAdapter>`, defines the marker and canonical method name, derives handshake `openExternalUrl: true` from presence, and installs it in `KasEngine`. V2 still returns `Adapters::NONE`. Evidence: `:24`, `:35-64`, `:82-85`, `:131-134`, `:244-247`, `:322-334`. | `Engine::adapters` and `client_capabilities` |
| `crates/cyril-core/src/protocol/domain_mediator/host.rs` | Extends host-request dispatch with capability refusal, URL-argument parsing, typed callback construction, and the existing typed-response bridge. Extends `supports(adapters, family)` for `HostFamily::ExternalUrl`. Non-KAS builds explicitly refuse this known method. Evidence: `:45-66`, `:96-120`, `:226-244`, `:253-267`. | Host ingress/dispatch adapter |
| `crates/cyril-core/src/protocol/kas/callbacks.rs` | Adds `HostCallback::OpenExternalUrl`, `HostFamily::ExternalUrl`, and `OpenExternalUrlArgs::parse(&Value) -> Result<Self, &'static str>`. Owns required-field validation, global callback metadata, bounded notification delivery, and success/error reply completion. Evidence: `:36-40`, `:111-125`, `:175-191`, `:320-334`, `:426-440`. | Typed KAS callback adapter |
| `crates/cyril-core/src/protocol/convert/kas.rs` | Extends `session_info_to_notification(&SessionInfoUpdate) -> Option<Notification>` to recognize exactly `kind == "display_error"` and flat `errorType == "mcp_connection_error"`, require a nonblank string `message`, and preserve that message. Evidence: `:584-610`. | KAS wire-to-domain converter |
| `crates/cyril-ui/src/mcp_notice.rs` | New text-projection module. Public `terminal_safe(&str) -> impl Display`; crate-internal `characters(&str) -> impl Iterator<Item = char>` and escape predicate. Preserves ordinary characters and visibly escapes control/bidi characters through one shared iterator implementation. Evidence: `:5-15`, `:17-50`. | UI terminal-safe presentation policy |
| `crates/cyril-ui/src/lib.rs` | Publicly exposes `mcp_notice`; no additional runtime logic. Evidence: `:12`. | UI crate export surface |
| `crates/cyril-ui/src/state.rs` | Applies global/main notices; appends safe system messages; reveals main history by unfocusing child drill-in and resetting browse position. Existing public scoped application methods now retain the scoped event and add a session-labelled main-history presentation copy for MCP errors. Evidence: `:1130-1147`, `:1395-1417`, `:2088-2099`, `:2130-2142`. | `UiState` transcript and visibility coordination |
| `crates/cyril-ui/src/subagent_ui.rs` | Extends the shared `SubagentStream` notification reducer with canonical MCP-error transcript insertion, using existing chronological `push_system`. The arm does not change activity or clear tool tracking. Evidence: `:94-104`, `:154-160`. | Per-session stream reducer; storage/focus owned by `SubagentUiState` |
| `crates/cyril-ui/src/widgets/chat.rs` | Keeps public `render(..., &dyn TuiState, ...)`; adds private `content_and_gutter(Rect) -> (Rect, Option<Rect>)`. Main and focused-child rendering now use the same reserved content width for conversion, wrapping, line counting, and painting; scrollbar paints only in its gutter. Evidence: `:10-25`, `:32-93`, `:106-171`. | Chat viewport/rendering adapter |
| `crates/cyril/examples/test_bridge.rs` | Executable notification consumer, not merely a test module. Adds safe console rendering for both new event variants. Private `external_url_text(&str) -> impl Display` forwards URL formatting to the UI helper. Evidence: `:44-47`, `:64-65`, `:627-637`. | Example executable’s output adapter |
| `crates/cyril/src/app.rs` — **test-only delta** | Sole committed change registers `mod mcp_oauth` inside existing `#[cfg(test)] mod tests`; no production routing/application behavior changes in this file. Evidence: `:3236-3237`, `:10555-10557`, plus the committed single-line diff. | App test composition, not a new runtime interface |

### Important interface semantics

- URL validation is **shape validation**, not URI validation: it accepts any nonblank string, uses trimming only to test emptiness, and copies the original string without rewriting it. Errors are fixed text rather than reflected URL content: `crates/cyril-core/src/protocol/kas/callbacks.rs:180-189`.
- URL success means **queue acceptance**, not browser opening or authorization. Dispatch awaits `DomainChannels::enqueue`, then constructs `{"success": true}` only on success: `crates/cyril-core/src/protocol/kas/callbacks.rs:429-439`.
- MCP classification uses the **flat fields**; the converter does not use `displayError` as a fallback: `crates/cyril-core/src/protocol/convert/kas.rs:587-610`.
- “Durable” here means ordinary retained transcript messages, **not unlimited retention or new disk persistence**. Main insertion invokes the existing configured message limit: `crates/cyril-ui/src/state.rs:1395-1400`, `:3596-3607`.

## 2. Responsibility clusters and separating rule

| Cluster | Reconstructed separating rule | Source evidence |
|---|---|---|
| Domain event vocabulary | Describe an event independently of wire parsing and display policy; carry session identity in the envelope. | `crates/cyril-core/src/types/event.rs:209-218`, `:566-582` |
| Engine capability declaration | Decide whether a callback family is installed; derive advertisement from the same presence data used by dispatch. | `crates/cyril-core/src/protocol/engine.rs:35-64`, `:131-134`; `crates/cyril-core/src/protocol/domain_mediator/host.rs:253-267` |
| Host-request adaptation | Translate SDK extension requests into typed callbacks and translate typed outcomes back to SDK responses. | `crates/cyril-core/src/protocol/domain_mediator/host.rs:96-120`, `:226-244` |
| KAS callback semantics | Validate callback-specific parameters and execute domain delivery/reply ordering. | `crates/cyril-core/src/protocol/kas/callbacks.rs:180-189`, `:429-439` |
| KAS notification conversion | Interpret KAS metadata and produce domain events without rendering or transcript mutation. | `crates/cyril-core/src/protocol/convert/kas.rs:584-610` |
| Safe text projection | Convert untrusted characters to terminal-safe display, independently of routing, focus, and transcript ownership. | `crates/cyril-ui/src/mcp_notice.rs:8-15`, `:34-49` |
| Transcript ownership and attention | Keep canonical scoped history with its owner, while deliberately revealing a main-history notice. | `crates/cyril-ui/src/state.rs:1403-1417`, `:2088-2099`, `:2130-2142`; `crates/cyril-ui/src/subagent_ui.rs:154-160` |
| Viewport geometry | Ensure content cells and scrollbar cells do not overlap, independently of MCP-specific event semantics. | `crates/cyril-ui/src/widgets/chat.rs:10-21`, `:84-93`, `:161-171` |
| Executable output | Format domain notifications for a console consumer using the existing safe-text policy. | `crates/cyril/examples/test_bridge.rs:627-637` |

A notable distinction is that **canonical ownership and user attention are not the same decision**: scoped errors first enter the appropriate stream, then `UiState` adds a separately labelled main presentation and reveals it. The ownership decision itself remains in existing App routing, not in the new formatting helper: `crates/cyril/src/app.rs:1474-1506`; `crates/cyril-ui/src/state.rs:2095-2098`, `:2137-2140`.

## 3. Dependency directions and concrete adapters

### External URL path

```text
SDK extension request
  → host dispatch / engine capability gate
  → typed OpenExternalUrlArgs + HostCallback
  → existing host lifecycle mediator
  → callback dispatch
  → bounded DomainChannels
  → global RoutedNotification
  → existing App application
  → UiState system history + reveal policy
  → chat renderer
```

Concrete boundaries:

1. `host::dispatch_extension` canonicalizes the method, checks installed capability, parses parameters, and creates the typed reply channel:  
   `crates/cyril-core/src/protocol/domain_mediator/host.rs:52`, `:96-120`.
2. `accept` delegates lifecycle admission to `HostMediator`, and spawned jobs invoke callback dispatch:  
   `crates/cyril-core/src/protocol/domain_mediator/host.rs:272-307`.
3. The callback adapter enqueues a domain event before replying. `DomainChannels` uses bounded Tokio channels; send failure becomes an ACP internal error:  
   `crates/cyril-core/src/protocol/kas/callbacks.rs:429-439`;  
   `crates/cyril-core/src/protocol/domain_mediator/mod.rs:23-24`, `:149-165`.
4. `.into()` produces a global envelope with no session, turn, or origin:  
   `crates/cyril-core/src/types/event.rs:566-573`, `:604-607`.
5. Existing App application calls the UI reducer; the new UI arm appends and reveals the notice:  
   `crates/cyril/src/app.rs:1538-1541`;  
   `crates/cyril-ui/src/state.rs:1130-1137`, `:1403-1407`.

`ExternalUrlAdapter` is a **presence marker**, not a callable browser-opener abstraction. The actual adapter behavior lives in host dispatch and callback dispatch: `crates/cyril-core/src/protocol/engine.rs:82-85`; `crates/cyril-core/src/protocol/kas/callbacks.rs:429-439`.

### MCP-error path

```text
SDK session/update
  → Engine::convert_session_update
  → KAS session-info converter
  → scoped RoutedNotification
  → existing domain/turn forwarding
  → existing App ownership classification
      → main UiState
      → SubagentUiState / SubagentStream
      → WorkflowUiState / same SubagentStream
  → safe transcript content and main presentation
```

- `KasEngine` delegates `SessionInfoUpdate` to the KAS converter: `crates/cyril-core/src/protocol/engine.rs:344-352`.
- Domain ingress restores the wire session ID around the converted event: `crates/cyril-core/src/protocol/domain_mediator/inbound.rs:122-145`.
- Existing App routing selects workflow, child, pending, or main before main application: `crates/cyril/src/app.rs:1474-1541`.
- Workflow streams reuse `SubagentStream`, so its new MCP-error arm serves both child and workflow history without a changed workflow module: `crates/cyril-ui/src/workflow_ui.rs:15-20`, `:33-42`.
- Pre-main scoped notifications remain buffered and replay after `SessionCreated`: `crates/cyril/src/app.rs:1508-1533`, `:1749-1752`.

### Presentation dependencies

- UI reducers depend on core notification types and on `mcp_notice`; the formatter itself depends only on standard formatting/character machinery: `crates/cyril-ui/src/subagent_ui.rs:3-5`, `:154-158`; `crates/cyril-ui/src/mcp_notice.rs:3-15`.
- Chat rendering depends on the existing read-only `TuiState` interface and concrete stream read access, not directly on the MCP converter/callbacks: `crates/cyril-ui/src/widgets/chat.rs:5`, `:25-29`, `:98-105`; `crates/cyril-ui/src/traits.rs:36-44`, `:145-146`.
- The example executable now consumes the UI crate’s public safe formatter: `crates/cyril/examples/test_bridge.rs:44-47`, `:633-637`.

## 4. Pass-through modules and hypothetical seams

- **No new whole pass-through module was found.** The new `mcp_notice` module owns actual character-selection and escaping policy: `crates/cyril-ui/src/mcp_notice.rs:12-15`, `:17-49`.
- `lib.rs` adds only public module exposure: `crates/cyril-ui/src/lib.rs:12`.
- The example’s `external_url_text` is a genuine **pass-through function**: its entire implementation delegates to `terminal_safe`. Its local tests target that forwarding boundary: `crates/cyril/examples/test_bridge.rs:44-60`.
- `characters` is not currently an independent cached-document adapter: repository search found its only consumer in `TerminalSafe::fmt`, and that call is visible at `crates/cyril-ui/src/mcp_notice.rs:44-48`. A future cached-panel use suggested by its comment is **hypothetical**, not demonstrated production integration.
- There is no new injected URL opener, authorization-completion interface, or credential-storage interface in this path. Its concrete execution ends in notification enqueue plus reply construction: `crates/cyril-core/src/protocol/kas/callbacks.rs:429-439`.
- Which parents are formally “protected,” and which seams were approved, is **undetermined from production code**.

## 5. Parent growth: wiring versus responsibility

| Parent | Reconstruction of growth |
|---|---|
| `protocol/engine.rs` | Capability vocabulary and derivation policy grow: a new presence field, marker, advertised key, and KAS installation. More than wiring, but within existing capability ownership. `:35-64`, `:82-85`, `:131-134`, `:322-334`. |
| `domain_mediator/host.rs` | Adds protocol adaptation and validation-error routing for a new callback family, reusing existing lifecycle and response machinery. `:96-120`, `:226-244`, `:253-267`. |
| `kas/callbacks.rs` | Adds real callback semantics: required URL shape, global scope, bounded delivery-before-acknowledgement. This is substantive responsibility, not merely a forwarding arm. `:180-189`, `:330-334`, `:429-439`. |
| `convert/kas.rs` | Adds a new wire-classification responsibility inside the existing converter. `:587-610`. |
| `cyril-ui/state.rs` | Adds substantive **attention policy**: unfocus a child, return main history to follow mode, and create scoped presentation copies. This exceeds simple enum-match wiring. `:1403-1417`, `:2095-2098`, `:2137-2140`. |
| `subagent_ui.rs` | Adds event handling within the existing stream reducer; chronology comes from existing `push_system`. `:94-104`, `:154-160`. |
| `widgets/chat.rs` | Adds generic geometry policy affecting all messages and live text, not an MCP-specific branch. `:10-21`, `:32-93`, `:106-171`. |
| `cyril-ui/lib.rs` | Export wiring only. `:12`. |
| `cyril/src/app.rs` | Test registration only; no production-parent growth in the committed delta. `:3236-3237`, `:10555-10557`. |
| `examples/test_bridge.rs` | Consumer output adaptation plus a small forwarding formatter; no new ownership or authorization state. `:44-47`, `:627-637`. |

## 6. Tests reaching past interfaces

These are source observations, not test-run results.

### Internal coupling / white-box access

1. **Bounded-acknowledgement test bypasses the SDK and lifecycle owner.** It constructs `DomainChannels`, fills/drains its receiver, constructs `DispatchCtx` and `HostCallback` directly, calls callback dispatch, and polls the future/reply channel. This intentionally reaches internal queue/callback boundaries rather than testing solely through the wire interface:  
   `crates/cyril-core/src/protocol/domain_mediator/tests/oauth.rs:67-130`.

2. **Wire harness directly composes internal owners.** It constructs `DomainChannels`, `DispatchCtx`, `HostMediator`, `host::run`, and the test SDK runtime. Requests themselves travel through the SDK, but fixture construction reaches private/crate-private composition boundaries:  
   `crates/cyril-core/src/protocol/domain_mediator/tests/external_url_wire.rs:19-55`, `:57-99`.

3. **Capability probe knows concrete engine representation.** It directly initializes `KasEngine { hooks_mode, ..Default::default() }` and compares serialized capabilities with the internal `host::supports` predicate:  
   `crates/cyril-core/src/protocol/probe_dn91.rs:19-64`, `:110-121`.

4. **App tests are descendant-module white boxes.** They invoke private notification/draw methods, mutate `redraw_needed`, and inspect private `session`, `ui_state`, and pending-notification storage:  
   `crates/cyril/src/app/tests/mcp_oauth.rs:6-9`, `:27-48`, `:169-193`.  
   Their registration under the parent’s `#[cfg(test)]` module explains this access: `crates/cyril/src/app.rs:3236-3237`, `:10556`.

5. **Conversion test couples to diagnostic representation.** One assertion compares `format!("{converted:?}")` with an exact variant/debug string rather than matching the domain variant:  
   `crates/cyril-core/src/protocol/domain_mediator/tests/mcp_error.rs:37-43`.

### Tests remaining at behavioral/read interfaces

- New state tests generally drive public notification/input/focus methods and inspect read-only state rather than assigning private state fields:  
  `crates/cyril-ui/src/state.rs:6234-6398`.
- The new canonical child-error test uses `apply_notification`, `streams()`, stream getters, and message kinds:  
  `crates/cyril-ui/src/subagent_ui.rs:486-547`.
- Renderer regression tests call public `render` with real `UiState` or `MockTuiState` and inspect `TestBackend` cells. They do **not** call the new private geometry helper directly:  
  `crates/cyril-ui/src/widgets/chat.rs:2483-2528`, `:2531-2603`, `:2606-2759`.
- The real-state focused-error renderer test explicitly re-enters drill-in through `focus_subagent` after notice delivery, rather than changing focus representation directly:  
  `crates/cyril-ui/src/widgets/chat.rs:2626-2645`.

### Changed test-only paths, excluded from the production ledger

- `crates/cyril-core/src/protocol/domain_mediator/tests/external_url_wire.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/mcp_error.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/mod.rs`
- `crates/cyril-core/src/protocol/domain_mediator/tests/oauth.rs`
- `crates/cyril-core/src/protocol/probe_dn91.rs`
- `crates/cyril/src/app/tests/mcp_oauth.rs`

Feature separation is explicit: wire tests are registered generally; MCP conversion/runtime tests require `kas`; the probe requires both test and `kas`: `crates/cyril-core/src/protocol/domain_mediator/tests/mod.rs:1-8`; `crates/cyril-core/src/protocol/mod.rs:19-20`.