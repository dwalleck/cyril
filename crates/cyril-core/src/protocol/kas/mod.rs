//! KAS-engine support, gated behind the `kas` cargo feature (KAS-1, cyril-evwh).
//!
//! - [`discovery`] — free-path spawn resolution (Part A).
//! - [`auth`] — the `_kiro/auth/getAccessToken` custodian responder (Part B).
//! - [`version`] — wrapper version→flag + the `kiro-cli acp` command (Part B).
//! - [`host_io`] — the bare-ACP `fs/*` host-callback responders (KAS-5a, cyril-7bdu).
//! - [`host_shell`] — startup shell resolution and command rendering (cyril-6bol).
//! - [`kiro_fs`] — the `_kiro/fs/*` superset dialect (cyril-kf2g).
//! - [`terminal_io`] — the `terminal/*` host-callback responders (KAS-5b, cyril-ufie).
//! - [`settings`] — the `_meta.kiro.settings` (AgentSettings) handshake (cyril-nhzw).

use agent_client_protocol as acp;

pub(crate) mod auth;
/// Typed host callbacks for the mediation seam (cyril-g9vt). Test-staged
/// until the slice-3 loop wiring becomes its first production consumer.
#[cfg(test)]
pub(crate) mod callbacks;
pub(crate) mod discovery;
pub(crate) mod hooks;
pub(crate) mod host_io;
pub(crate) mod host_shell;
pub(crate) mod kiro_fs;
pub(crate) mod settings;
pub(crate) mod terminal_io;
pub(crate) mod version;

/// Wrap a JSON value as an ACP ext response.
///
/// Every `_kiro/*` responder ends with the same serialize → `RawValue` →
/// `ExtResponse` dance, so it lives here rather than once per module.
///
/// Note what must NOT appear in `value`: KAS treats a `message` key on a
/// capability response as an **error channel** (`if (typed.message) throw`,
/// carved from `acp-port-adapters.ts`), so a success payload that happens to
/// carry one is read as a failure. Errors travel as JSON-RPC errors instead.
pub(crate) fn json_ext_response(value: &serde_json::Value) -> acp::Result<acp::ExtResponse> {
    let body = serde_json::to_string(value)
        .map_err(|e| acp::Error::new(-32603, format!("serialize ext reply: {e}")))?;
    let raw = serde_json::value::RawValue::from_string(body)
        .map_err(|e| acp::Error::new(-32603, format!("ext reply raw value: {e}")))?;
    Ok(acp::ExtResponse::new(raw.into()))
}
