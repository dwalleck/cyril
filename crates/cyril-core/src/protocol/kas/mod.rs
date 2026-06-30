//! KAS-engine support, gated behind the `kas` cargo feature (KAS-1, cyril-evwh).
//!
//! - [`discovery`] — free-path spawn resolution (Part A).
//! - [`auth`] — the `_kiro/auth/getAccessToken` custodian responder (Part B).
//! - [`version`] — wrapper version→flag + the `kiro-cli acp` command (Part B).
//! - [`host_io`] — the `fs/*` host-callback responders (KAS-5a, cyril-7bdu).

pub(crate) mod auth;
pub(crate) mod discovery;
pub(crate) mod host_io;
pub(crate) mod version;
