//! KAS-engine support, gated behind the `kas` cargo feature (KAS-1, cyril-evwh).
//!
//! - [`discovery`] — free-path spawn resolution (Part A).
//! - [`auth`] — the `_kiro/auth/getAccessToken` custodian responder (Part B).

pub(crate) mod auth;
pub(crate) mod discovery;
