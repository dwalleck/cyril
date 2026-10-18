//! KAS auth responder (KAS-1 Part B, cyril-evwh; kiro-cli token source,
//! cyril-zbu7).
//!
//! Answers the `_kiro/auth/getAccessToken` server→client request KAS sends
//! under `--auth=acp-callback` — both spawn modes: the free path (the directly
//! spawned server asks its host) and the wrapper (kiro-cli forwards the
//! callback to its ACP client under `--auth-method client`, the pre-2.29.0
//! default). The token comes from kiro-cli itself, the way Kiro's own TUI gets
//! it (tui.js handler `Rsr`, every release since 2.6.1): run the hidden host
//! subcommand `kiro-cli chat _ get-kas-token`, appending `--force-refresh`
//! iff the request asks, and pass the JSON envelope's `data` fields through.
//! kiro-cli owns the OIDC refresh token and refreshes an expired access token
//! under its own cross-process refresh lock, so renewal stays single-flight
//! inside kiro-cli: cyril never does OIDC.
//!
//! The subcommand is hidden/internal, so a kiro-cli may lack it. ONLY when
//! kiro-cli refuses it as an unknown subcommand (clap's exact refusal — see
//! [`helper_refused`]) does cyril fall back to reading the stored token
//! ([`super::stored_token`], never renewed), remembering that per process and
//! warning the user once per connection that sessions will expire hourly
//! until kiro-cli is updated and cyril restarted. Every other failure is a hard, typed error.
//!
//! cyril is a **custodian** of the access token it relays: held for one
//! reply, redacted in `Debug`, never logged, never echoed into a diagnostic.

use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;

use crate::protocol::kas::version::{CaptureFailure, Captured, render, run_captured};
use crate::types::{AgentCommand, SpawnEnvironment};

/// The method string cyril receives for the wire request `_kiro/auth/getAccessToken`.
/// The ACP library strips the single leading `_` before dispatch (same as it
/// does for the `_kiro.dev/*` ext notifications cyril already handles as
/// `kiro.dev/*`), so the `_kiro` namespace arrives as `kiro`.
pub(crate) const GET_ACCESS_TOKEN_METHOD: &str = "kiro/auth/getAccessToken";

/// The actionable remediation for a logged-out / unrenewable credential.
pub(crate) const LOGIN_HINT: &str = "run `kiro-cli login`";

/// The hidden kiro-cli host subcommand (after the kiro-cli invocation
/// prefix) that resolves — and refreshes when expired — the KAS access token.
const GET_KAS_TOKEN_ARGS: [&str; 3] = ["chat", "_", "get-kas-token"];
/// Appended iff the KAS request carries `forceRefresh: true`.
const FORCE_REFRESH_FLAG: &str = "--force-refresh";
/// Bound on one helper run — the TUI's `Oc` default (30 s): kiro-cli may
/// refresh over the network.
const GET_KAS_TOKEN_DEADLINE: Duration = Duration::from_secs(30);
/// stderr lines quoted in a non-zero-exit diagnostic (the TUI quotes 1).
const STDERR_TAIL_LINES: usize = 2;
/// Cap on the quoted stderr tail and on a relayed error-envelope message, so
/// a runaway helper cannot flood the UI.
const DIAGNOSTIC_CHAR_CAP: usize = 400;

/// JSON-RPC error codes the responder replies with: server-defined for an
/// auth state the user must act on (kiro-cli's own error envelope — logged
/// out, refresh refused), internal for every helper/transport failure.
const JSONRPC_INTERNAL_ERROR: i32 = -32603;
const JSONRPC_AUTH_REQUIRED: i32 = -32000;

/// A redacted access-token wrapper: its `Debug` never prints the secret, so a
/// stray `{:?}` or a tracing of any struct containing it cannot leak the
/// credential (spec SC4 custodian).
pub(super) struct AccessToken(pub(super) String);

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessToken(***redacted***)")
    }
}

/// Where a served token came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenOrigin {
    /// kiro-cli's `chat _ get-kas-token` helper (the normal path).
    Helper,
    /// The stored-token fallback, because this kiro-cli lacks the helper
    /// subcommand; `kiro_cli_version` when `--version` answered.
    StoredFallback { kiro_cli_version: Option<String> },
}

/// The `data` of kiro-cli's `getKasToken` envelope — exactly the fields the
/// TUI relays to KAS: `{accessToken, expiresAt, profileArn}` required,
/// `authMethod` / `provider` optional (relayed only when present) — plus
/// where it came from (never relayed).
#[derive(Debug)]
pub(crate) struct KasToken {
    access_token: AccessToken,
    expires_at: String,
    profile_arn: String,
    auth_method: Option<String>,
    provider: Option<String>,
    origin: TokenOrigin,
}

impl KasToken {
    /// The token's expiry as kiro-cli reported it — not secret; logged so a
    /// renewal is observable without exposing the credential.
    pub(crate) fn expires_at(&self) -> &str {
        &self.expires_at
    }

    /// Where this token came from.
    pub(crate) fn origin(&self) -> &TokenOrigin {
        &self.origin
    }

    /// A token read by the stored-token fallback (origin filled in by the
    /// caller that knows the kiro-cli version).
    pub(super) fn from_store(
        access_token: AccessToken,
        expires_at: String,
        profile_arn: String,
    ) -> Self {
        Self {
            access_token,
            expires_at,
            profile_arn,
            auth_method: None,
            provider: None,
            origin: TokenOrigin::StoredFallback {
                kiro_cli_version: None,
            },
        }
    }
}

/// Why kiro-cli could not provide a KAS access token. Every variant is
/// actionable on its own: its `Display` ends with the remediation, and it
/// never carries the helper's stdout (which may hold a token).
#[derive(Debug, thiserror::Error)]
pub(crate) enum KasTokenError {
    /// No token source is bound for this connection (a non-KAS engine).
    #[error(
        "no kiro-cli token source is bound to this connection; restart cyril with the KAS engine"
    )]
    Unbound,
    /// kiro-cli could not be started.
    #[error(
        "could not run `{command}` to get the KAS access token: {source}; {remedy}, then retry"
    )]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
        /// Where this source expects kiro-cli — PATH and the TUI locations
        /// on the free path, the `--agent-command` kiro-cli on the wrapper.
        remedy: &'static str,
    },
    /// kiro-cli did not finish within the deadline (it was killed).
    #[error(
        "`{command}` did not answer within {secs} s; check the network connection to the \
         Kiro auth service, or another kiro-cli holding its token-refresh lock (kiro-cli logs \
         `refresh lock timed out (peer wedged?)`), and retry"
    )]
    Timeout { command: String, secs: u64 },
    /// Waiting for kiro-cli or reading its output failed.
    #[error("could not read the answer of `{command}`: {detail}; retry")]
    Io { command: String, detail: String },
    /// kiro-cli exited non-zero without an error envelope (or with a success
    /// envelope, which a failing exit makes untrustworthy).
    #[error(
        "`{command}` exited with {status}{stderr_tail}; {LOGIN_HINT} if you are logged out, \
         then retry"
    )]
    NonZeroExit {
        command: String,
        status: String,
        /// `""` or `": <last stderr lines>"`, capped.
        stderr_tail: String,
    },
    /// kiro-cli answered with its `{kind:"error", data:{message}}` envelope.
    #[error("kiro-cli could not provide a KAS access token: {message}{hint}")]
    ErrorEnvelope {
        message: String,
        /// `""` when `message` already names `kiro-cli login`, else the hint.
        hint: String,
    },
    /// kiro-cli's answer is not a recognizable envelope. `detail` names the
    /// shape problem only — never the raw line.
    #[error(
        "`{command}` returned an unrecognized answer ({detail}); update kiro-cli or report \
         this to cyril"
    )]
    Malformed { command: String, detail: String },
    /// The kiro-cli refused the helper itself as an unknown subcommand (clap
    /// exit 2, no stdout, `unrecognized subcommand 'get-kas-token'` or — before
    /// `chat _` existed — `unexpected argument 'get-kas-token'`). Internal:
    /// [`KasTokenSource::fetch`] turns it into the stored-token fallback.
    #[error(
        "`{command}` is not supported by this kiro-cli ({detail}); update kiro-cli so cyril \
         can renew the KAS access token"
    )]
    HelperMissing { command: String, detail: String },
    /// This kiro-cli lacks the helper and the stored-token fallback cannot
    /// serve (store absent/locked/corrupt, logged out, missing field).
    #[error(
        "this kiro-cli{version} lacks `chat _ get-kas-token` and the stored kiro token is not \
         usable: {detail}; update kiro-cli (or {LOGIN_HINT})"
    )]
    StoredUnservable {
        /// `""` or `" <version>"`.
        version: String,
        detail: String,
    },
    /// This kiro-cli lacks the helper and the stored token is expired — only
    /// kiro-cli can renew it.
    #[error(
        "kiro token expired and this kiro-cli{version} cannot renew it for cyril (it lacks \
         `chat _ get-kas-token`); {LOGIN_HINT}, or update kiro-cli and restart cyril to stop hourly expiry"
    )]
    StoredStale {
        /// `""` or `" <version>"`.
        version: String,
    },
    /// The relayed reply could not be serialized for KAS.
    #[error("could not build the getAccessToken reply: {0}; report this to cyril")]
    Reply(String),
}

impl KasTokenError {
    /// The JSON-RPC error the responder returns to KAS.
    fn to_acp(&self) -> acp::Error {
        let code = match self {
            Self::ErrorEnvelope { .. } | Self::StoredStale { .. } => JSONRPC_AUTH_REQUIRED,
            _ => JSONRPC_INTERNAL_ERROR,
        };
        acp::Error::new(code, self.to_string())
    }

    fn error_envelope(message: &str) -> Self {
        let message = cap(message.trim());
        let hint = if message.contains("kiro-cli login") {
            String::new()
        } else {
            format!("; {LOGIN_HINT}")
        };
        Self::ErrorEnvelope { message, hint }
    }
}

/// Truncate a diagnostic to [`DIAGNOSTIC_CHAR_CAP`] chars (char-boundary safe).
fn cap(text: &str) -> String {
    match text.char_indices().nth(DIAGNOSTIC_CHAR_CAP) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

/// How cyril asks kiro-cli for the KAS access token: the invocation prefix
/// that runs the right kiro-cli (on the free path the one
/// [`resolve_free_kiro_cli`] finds — PATH, then the Kiro TUI's install
/// locations — which bundle selection's version probe also asks; on the
/// wrapper path the agent command's own kiro-cli, through the WSL launcher
/// when it routes through one) and the environment it runs under.
/// Calls are serialized PROCESS-wide per resolved kiro-cli binary + args
/// ([`lock_key`]; a second request — from this connection or any other in
/// the process, e.g. the workbench reviewer runtime — waits for the first
/// helper to finish), so cyril never runs two token helpers for one kiro-cli
/// at once: OIDC refresh tokens are single-use, and racing renewals can log
/// the user out.
#[derive(Debug)]
pub(crate) struct KasTokenSource {
    kiro_cli: AgentCommand,
    environment: SpawnEnvironment,
    /// Process-wide serialization + missing-helper cache key ([`lock_key`]).
    lock_key: String,
    deadline: Duration,
    /// The remediation a spawn failure names (see [`KasTokenError::Spawn`]).
    spawn_remedy: &'static str,
    store: StoreLocation,
    /// Set once the stored-token fallback warning has been surfaced for this
    /// connection, so it is shown once, not on every request.
    fallback_warned: std::sync::atomic::AtomicBool,
}

/// Where the stored-token fallback looks for kiro-cli's credential store.
#[derive(Debug)]
enum StoreLocation {
    /// kiro-cli's data dir resolved from cyril's environment (production).
    FromEnvironment,
    /// A fixture store (tests never touch the real one).
    #[cfg(test)]
    At(std::path::PathBuf),
}

/// kiro-cli's file name on this platform (`kiro-cli.exe` on Windows).
fn kiro_cli_file_name() -> String {
    format!("kiro-cli{}", std::env::consts::EXE_SUFFIX)
}

/// Spawn-failure remediation on the free path (where [`resolve_free_kiro_cli`]
/// looks).
#[cfg(not(windows))]
const FREE_PATH_REMEDY: &str = "install kiro-cli on PATH (cyril also looks in ~/.local/bin, \
     /usr/local/bin and /opt/homebrew/bin)";
#[cfg(windows)]
const FREE_PATH_REMEDY: &str = "install kiro-cli so that kiro-cli.exe is on PATH";
/// Spawn-failure remediation when the kiro-cli comes from `--agent-command`.
const AGENT_COMMAND_REMEDY: &str = "check the kiro-cli named by --agent-command";

/// The Kiro TUI's last-resort kiro-cli locations (tui.js 2.32.0 `ku`):
/// `~/.local/bin/kiro-cli`, `/usr/local/bin/kiro-cli`,
/// `/opt/homebrew/bin/kiro-cli`. None on Windows: those literal paths name
/// the extension-less unix binary (the TUI probes them as-is), while the
/// Windows MSI's `kiro-cli.exe` is reached through PATH.
const TUI_FALLBACK_DIRS: [&str; 2] = ["/usr/local/bin", "/opt/homebrew/bin"];

/// Locate the kiro-cli the free path asks for the KAS token. Pure: PATH,
/// home and the executable probe are injected, so tests fake the host.
///
/// Order: the first executable `kiro-cli` in PATH order (absolute entries
/// only — the TUI's `YM` skips relative ones), then — only when PATH has
/// none — the TUI's install locations ([`TUI_FALLBACK_DIRS`], after
/// `<home>/.local/bin`), else the bare name `kiro-cli` (its spawn failure is
/// the actionable error). PATH comes first, unlike the TUI's `ku` (which
/// tries `~/.local/bin/<its launcher's basename>` before PATH because the Rust
/// host hands it that binary), so the gate and responder ask the same
/// kiro-cli the free path's version probe asks.
fn resolve_free_kiro_cli(
    path_var: Option<&std::ffi::OsStr>,
    home: Option<&std::path::Path>,
    cwd: Option<&std::path::Path>,
    is_executable: impl Fn(&std::path::Path) -> bool,
) -> AgentCommand {
    let name = kiro_cli_file_name();
    let on_path = path_var
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute() && !under_cwd(dir, cwd))
        .map(|dir| dir.join(&name));
    let fallbacks: Vec<std::path::PathBuf> = if cfg!(windows) {
        Vec::new()
    } else {
        home.map(|h| h.join(".local/bin/kiro-cli"))
            .into_iter()
            .chain(
                TUI_FALLBACK_DIRS
                    .iter()
                    .map(|dir| std::path::Path::new(dir).join("kiro-cli")),
            )
            .collect()
    };
    for candidate in on_path.chain(fallbacks) {
        if is_executable(&candidate) {
            match candidate.to_str() {
                Some(found) => return AgentCommand::new(found),
                None => {
                    tracing::debug!(candidate = %candidate.display(), "non-UTF-8 kiro-cli path skipped")
                }
            }
        }
    }
    AgentCommand::new("kiro-cli")
}

/// `true` when a PATH entry is the working directory or (unless the working
/// directory is a filesystem root) inside it — the TUI's `YM` skips those so
/// a binary planted in the workspace cannot shadow kiro-cli. Lexical
/// (component-wise) and, on Windows, case-insensitive like `YM`'s lowercasing;
/// the TUI additionally compares realpaths.
fn under_cwd(dir: &std::path::Path, cwd: Option<&std::path::Path>) -> bool {
    let Some(cwd) = cwd else {
        return false;
    };
    let fold = |p: &std::path::Path| -> std::path::PathBuf {
        if cfg!(windows) {
            std::path::PathBuf::from(p.to_string_lossy().to_lowercase())
        } else {
            p.to_path_buf()
        }
    };
    let (dir, cwd) = (fold(dir), fold(cwd));
    dir == cwd || (cwd.parent().is_some() && dir.starts_with(&cwd))
}

/// A regular file the current user may execute — the TUI's `y1e`: on unix
/// `access(path, X_OK)` for the current (real) user, as Node's
/// `fs.accessSync(…, X_OK)` does; on Windows any file (`y1e` skips the
/// execute check there).
fn is_executable_file(path: &std::path::Path) -> bool {
    let is_file = std::fs::metadata(path).is_ok_and(|meta| meta.is_file());
    #[cfg(unix)]
    {
        is_file && nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).is_ok()
    }
    #[cfg(not(unix))]
    {
        is_file
    }
}

/// The process-wide lock key for a kiro-cli invocation: the RESOLVED binary
/// (an absolute program as-is, a bare name looked up on `path_var` like
/// [`resolve_free_kiro_cli`], then canonicalized through `canonical`) plus
/// the invocation's args — so different spellings of one binary (Free's
/// `/usr/local/bin/kiro-cli` vs a Wrapper's bare `kiro-cli`) share a lock,
/// while a WSL launcher with different distro args does not. A program that
/// cannot be resolved keys on its spelling (logged).
fn lock_key(
    kiro_cli: &AgentCommand,
    path_var: Option<&std::ffi::OsStr>,
    is_executable: impl Fn(&std::path::Path) -> bool,
    canonical: impl Fn(&std::path::Path) -> Option<std::path::PathBuf>,
) -> String {
    let program = std::path::Path::new(kiro_cli.program());
    let resolved = if program.is_absolute() {
        Some(program.to_path_buf())
    } else if program.components().count() == 1 {
        let name = match program.extension() {
            Some(_) => program.as_os_str().to_os_string(),
            None => {
                let mut name = program.as_os_str().to_os_string();
                name.push(std::env::consts::EXE_SUFFIX);
                name
            }
        };
        path_var
            .into_iter()
            .flat_map(std::env::split_paths)
            .filter(|dir| dir.is_absolute())
            .map(|dir| dir.join(&name))
            .find(|candidate| is_executable(candidate))
    } else {
        None
    };
    let shown = match resolved {
        Some(path) => match canonical(&path) {
            Some(real) => real.display().to_string(),
            None => path.display().to_string(),
        },
        None => {
            tracing::debug!(
                program = kiro_cli.program(),
                "kiro-cli lock key falls back to the program's spelling"
            );
            kiro_cli.program().to_string()
        }
    };
    std::iter::once(shown.as_str())
        .chain(kiro_cli.args().iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Process-wide serialization of token-helper runs, one async lock per
/// kiro-cli invocation (keyed by [`lock_key`]: the resolved binary + args).
mod helper_gate {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    type Gates = Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

    fn gates() -> &'static Gates {
        static GATES: OnceLock<Gates> = OnceLock::new();
        GATES.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// The lock every source for `key` shares (created on first use; the
    /// sync map lock is never held across an await).
    pub(super) fn for_invocation(key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut map = gates()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(map.entry(key.to_string()).or_default())
    }
}

/// Per-process memory of kiro-cli invocations that lack the helper (keyed by
/// the rendered invocation prefix), with the version `--version` reported —
/// so the helper is probed once per process, not on every request.
mod missing_helper {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
        static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// `Some(version)` when `key` is known to lack the helper.
    pub(super) fn lookup(key: &str) -> Option<Option<String>> {
        let map = cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(key).cloned()
    }

    pub(super) fn remember(key: String, version: Option<String>) {
        let mut map = cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.insert(key, version);
    }
}

impl KasTokenSource {
    /// A source running `kiro_cli` (the invocation prefix, e.g. `kiro-cli` or
    /// `wsl -d Ubuntu kiro-cli`) under `environment`.
    pub(crate) fn new(kiro_cli: AgentCommand, environment: SpawnEnvironment) -> Self {
        let path_var = environment.effective_var(std::ffi::OsStr::new("PATH"));
        let lock_key = lock_key(&kiro_cli, path_var.as_deref(), is_executable_file, |p| {
            match std::fs::canonicalize(p) {
                Ok(real) => Some(real),
                Err(e) => {
                    tracing::debug!(path = %p.display(), error = %e, "kiro-cli path not canonicalized for its lock key");
                    None
                }
            }
        });
        Self {
            kiro_cli,
            environment,
            lock_key,
            deadline: GET_KAS_TOKEN_DEADLINE,
            spawn_remedy: AGENT_COMMAND_REMEDY,
            store: StoreLocation::FromEnvironment,
            fallback_warned: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The free-path source: the kiro-cli [`resolve_free_kiro_cli`] finds in
    /// cyril's own environment (PATH, then the Kiro TUI's install
    /// locations), under the inherited environment.
    pub(crate) fn on_path() -> Self {
        let path_var = std::env::var_os("PATH");
        let home = crate::kiro_agent_config::home_dir();
        let cwd = match std::env::current_dir() {
            Ok(cwd) => Some(cwd),
            Err(e) => {
                tracing::debug!(error = %e, "no current dir; PATH entries are not checked against it");
                None
            }
        };
        Self {
            spawn_remedy: FREE_PATH_REMEDY,
            ..Self::new(
                resolve_free_kiro_cli(
                    path_var.as_deref(),
                    home.as_deref(),
                    cwd.as_deref(),
                    is_executable_file,
                ),
                SpawnEnvironment::Inherit,
            )
        }
    }

    #[cfg(test)]
    fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    /// The full helper invocation: prefix + `chat _ get-kas-token`, plus
    /// `--force-refresh` iff `force_refresh`.
    fn invocation(&self, force_refresh: bool) -> AgentCommand {
        let mut args = self.kiro_cli.args().to_vec();
        args.extend(GET_KAS_TOKEN_ARGS.iter().map(|a| a.to_string()));
        if force_refresh {
            args.push(FORCE_REFRESH_FLAG.to_string());
        }
        AgentCommand::new(self.kiro_cli.program()).with_args(args)
    }

    /// The kiro-cli invocation prefix this source runs — also what bundle
    /// selection's version probe asks on the free path.
    pub(crate) fn kiro_cli(&self) -> &AgentCommand {
        &self.kiro_cli
    }

    /// The environment the helper (and that probe) run under.
    pub(crate) fn environment(&self) -> &SpawnEnvironment {
        &self.environment
    }

    /// `true` the first time this connection asks — the fallback warning is
    /// surfaced once per connection, not on every request.
    fn take_fallback_warning(&self) -> bool {
        !self
            .fallback_warned
            .swap(true, std::sync::atomic::Ordering::SeqCst)
    }

    #[cfg(test)]
    pub(crate) fn with_store(mut self, db: std::path::PathBuf) -> Self {
        self.store = StoreLocation::At(db);
        self
    }

    /// Get the KAS token: run the helper once and interpret its answer. Only
    /// when this kiro-cli refuses the helper as an unknown subcommand
    /// ([`KasTokenError::HelperMissing`]) does it fall back to the stored
    /// token — remembered per process, so the helper is not re-probed on
    /// every request. Every other failure is returned as-is.
    pub(crate) async fn fetch(&self, force_refresh: bool) -> Result<KasToken, KasTokenError> {
        let key = self.lock_key.clone();
        let gate = helper_gate::for_invocation(&key);
        let _serialized = gate.lock().await;
        if let Some(version) = missing_helper::lookup(&key) {
            return self.stored_fallback(version, force_refresh).await;
        }
        match self.run_helper(force_refresh).await {
            Err(KasTokenError::HelperMissing { command, detail }) => {
                let version = match crate::protocol::kas::version::invocation_version(
                    &self.kiro_cli,
                    &self.environment,
                )
                .await
                {
                    Ok(version) => Some(version),
                    Err(e) => {
                        tracing::debug!(error = %e, "kiro-cli version unavailable for the fallback warning");
                        None
                    }
                };
                tracing::warn!(
                    %command,
                    %detail,
                    version = version.as_deref(),
                    "kiro-cli lacks get-kas-token; falling back to the stored kiro token"
                );
                missing_helper::remember(key, version.clone());
                self.stored_fallback(version, force_refresh).await
            }
            other => other,
        }
    }

    /// The stored-token fallback ([`super::stored_token`]), read off the
    /// single-threaded bridge executor.
    async fn stored_fallback(
        &self,
        version: Option<String>,
        force_refresh: bool,
    ) -> Result<KasToken, KasTokenError> {
        if force_refresh {
            // The stored token cannot be renewed by cyril: the forced refresh
            // KAS asked for is not honoured on the fallback path.
            tracing::debug!(
                "getAccessToken forceRefresh not honoured: this kiro-cli lacks get-kas-token, serving the stored token"
            );
        }
        let shown = match &version {
            Some(v) => format!(" {v}"),
            None => String::new(),
        };
        let db = match &self.store {
            StoreLocation::FromEnvironment => crate::protocol::kas::discovery::default_store_path(),
            #[cfg(test)]
            StoreLocation::At(db) => Some(db.clone()),
        }
        .ok_or_else(|| KasTokenError::StoredUnservable {
            version: shown.clone(),
            detail: "cannot locate kiro-cli's data directory (no home / local data directory)"
                .to_string(),
        })?;
        let read = tokio::task::spawn_blocking(move || {
            super::stored_token::read_stored_token(&db, super::stored_token::now_epoch())
        })
        .await
        .map_err(|e| KasTokenError::StoredUnservable {
            version: shown.clone(),
            detail: format!("stored-token read task failed: {e}"),
        })?;
        match read {
            Ok(mut token) => {
                token.origin = TokenOrigin::StoredFallback {
                    kiro_cli_version: version,
                };
                Ok(token)
            }
            Err(super::stored_token::StoredTokenError::Unservable(detail)) => {
                Err(KasTokenError::StoredUnservable {
                    version: shown,
                    detail,
                })
            }
            Err(super::stored_token::StoredTokenError::Stale) => {
                Err(KasTokenError::StoredStale { version: shown })
            }
        }
    }

    /// Run the helper once and interpret its answer.
    async fn run_helper(&self, force_refresh: bool) -> Result<KasToken, KasTokenError> {
        let invocation = self.invocation(force_refresh);
        let command = render(&invocation);
        let captured = run_captured(&invocation, &self.environment, self.deadline)
            .await
            .map_err(|failure| match failure {
                CaptureFailure::Spawn(source) => KasTokenError::Spawn {
                    command: command.clone(),
                    source,
                    remedy: self.spawn_remedy,
                },
                CaptureFailure::DeadlineElapsed { .. } => KasTokenError::Timeout {
                    command: command.clone(),
                    secs: self.deadline.as_secs(),
                },
                CaptureFailure::StdioUnavailable(pipe) => KasTokenError::Io {
                    command: command.clone(),
                    detail: format!("{pipe} was not captured"),
                },
                CaptureFailure::Io { error, .. } => KasTokenError::Io {
                    command: command.clone(),
                    detail: error.to_string(),
                },
            })?;
        interpret(&command, &captured)
    }
}

/// One parsed kiro-cli envelope.
#[derive(Debug)]
enum Envelope {
    Token(KasToken),
    Error(String),
}

/// Interpret a finished helper run. Precedence (TUI `Oc`/`Rsr` parity where
/// it is safe): an error envelope wins regardless of exit status; a success
/// envelope is trusted only with a zero exit (a failing exit makes it
/// untrustworthy — fail closed, where the TUI merely warns); no envelope is
/// a non-zero-exit diagnostic when the exit failed, else malformed.
fn interpret(command: &str, captured: &Captured) -> Result<KasToken, KasTokenError> {
    let parsed = parse_envelope(&captured.stdout);
    match (parsed, captured.status.success()) {
        (Ok(Envelope::Error(message)), success) => {
            if success {
                tracing::debug!("kiro-cli error envelope arrived with a zero exit");
            }
            Err(KasTokenError::error_envelope(&message))
        }
        (Ok(Envelope::Token(token)), true) => Ok(token),
        (Err(_), false) if helper_refused(captured) => Err(KasTokenError::HelperMissing {
            command: command.to_string(),
            detail: stderr_tail(&captured.stderr)
                .trim_start_matches(": ")
                .to_string(),
        }),
        (Ok(Envelope::Token(_)), false) | (Err(_), false) => Err(KasTokenError::NonZeroExit {
            command: command.to_string(),
            status: captured.status.to_string(),
            stderr_tail: stderr_tail(&captured.stderr),
        }),
        (Err(detail), true) => Err(KasTokenError::Malformed {
            command: command.to_string(),
            detail,
        }),
    }
}

/// clap's usage-error exit code.
const CLAP_USAGE_EXIT: i32 = 2;

/// `true` when kiro-cli refused the helper itself as unknown — the exact shape
/// clap produces (observed 2026-10-18, `.cyril-zbu7/receipts/`): usage exit
/// 2, no stdout, and a stderr line naming `get-kas-token` as an `unrecognized
/// subcommand` (kiro-cli 2.5.1; 2.32.0 for any unknown `chat _` sibling) or an
/// `unexpected argument` (2.4.1, before `chat _` existed). Anything else —
/// other clap errors, any stdout at all, another exit code — is NOT a missing
/// helper and stays a hard error.
fn helper_refused(captured: &Captured) -> bool {
    if captured.status.code() != Some(CLAP_USAGE_EXIT)
        || !String::from_utf8_lossy(&captured.stdout).trim().is_empty()
    {
        return false;
    }
    String::from_utf8_lossy(&captured.stderr)
        .lines()
        .map(str::trim)
        .any(|line| {
            line == "error: unrecognized subcommand 'get-kas-token'"
                || line == "error: unexpected argument 'get-kas-token' found"
        })
}

/// `": <last lines>"` of stderr, capped, or `""` when stderr is empty.
fn stderr_tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let start = lines.len().saturating_sub(STDERR_TAIL_LINES);
    match lines.get(start..) {
        Some(tail) if !tail.is_empty() => format!(": {}", cap(&tail.join(" | "))),
        _ => String::new(),
    }
}

/// The JSON type name of `value`, for shape diagnostics that never echo the
/// value itself.
fn type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Parse the helper's stdout: the LAST non-empty line is the envelope (the TUI
/// `_1e` rule — earlier lines are tolerated noise). `Err` describes the shape
/// problem without quoting the line (it may contain a token).
fn parse_envelope(stdout: &[u8]) -> Result<Envelope, String> {
    let text = std::str::from_utf8(stdout).map_err(|_| "output is not UTF-8".to_string())?;
    let line = text
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .ok_or_else(|| "no output".to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|_| "the last output line is not JSON".to_string())?;
    let serde_json::Value::Object(envelope) = value else {
        return Err(format!(
            "the envelope is a JSON {}, not an object",
            type_name(&value)
        ));
    };
    let kind = match envelope.get("kind") {
        Some(serde_json::Value::String(kind)) => kind.as_str(),
        Some(other) => return Err(format!("`kind` is a {}, not a string", type_name(other))),
        None => return Err("`kind` is missing".to_string()),
    };
    let data = match envelope.get("data") {
        Some(serde_json::Value::Object(data)) => data,
        Some(other) => return Err(format!("`data` is a {}, not an object", type_name(other))),
        None => return Err("`data` is missing".to_string()),
    };
    match kind {
        "getKasToken" => token_from(data).map(Envelope::Token),
        "error" => match data.get("message") {
            Some(serde_json::Value::String(message)) => Ok(Envelope::Error(message.clone())),
            Some(other) => Err(format!(
                "error `message` is a {}, not a string",
                type_name(other)
            )),
            None => Err("error envelope has no `message`".to_string()),
        },
        _ => Err("unexpected envelope kind".to_string()),
    }
}

/// Build a [`KasToken`] from the `getKasToken` data object. Required fields
/// must be non-empty strings (an empty `profileArn` 400s at the backend, an
/// empty token or expiry is unusable); optional fields must be strings when
/// present (`null` counts as absent). Unknown keys are ignored by name (the
/// TUI relays only these five), with a debug breadcrumb.
fn token_from(data: &serde_json::Map<String, serde_json::Value>) -> Result<KasToken, String> {
    let required = |key: &str| -> Result<String, String> {
        match data.get(key) {
            Some(serde_json::Value::String(s)) if !s.is_empty() => Ok(s.clone()),
            Some(serde_json::Value::String(_)) => Err(format!("`{key}` is empty")),
            Some(other) => Err(format!("`{key}` is a {}, not a string", type_name(other))),
            None => Err(format!("`{key}` is missing")),
        }
    };
    let optional = |key: &str| -> Result<Option<String>, String> {
        match data.get(key) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
            Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
            Some(other) => Err(format!("`{key}` is a {}, not a string", type_name(other))),
        }
    };
    const KNOWN: [&str; 5] = [
        "accessToken",
        "expiresAt",
        "profileArn",
        "authMethod",
        "provider",
    ];
    let unknown: Vec<&str> = data
        .keys()
        .map(String::as_str)
        .filter(|k| !KNOWN.contains(k))
        .collect();
    if !unknown.is_empty() {
        tracing::debug!(?unknown, "getKasToken data keys not relayed to KAS");
    }
    Ok(KasToken {
        access_token: AccessToken(required("accessToken")?),
        expires_at: required("expiresAt")?,
        profile_arn: required("profileArn")?,
        auth_method: optional("authMethod")?,
        provider: optional("provider")?,
        origin: TokenOrigin::Helper,
    })
}

/// Build the `{accessToken, expiresAt, profileArn, authMethod?, provider?}`
/// ExtResponse KAS expects — the TUI's exact relay shape.
fn build_response(token: &KasToken) -> Result<acp::ExtResponse, String> {
    let mut body = serde_json::Map::new();
    body.insert(
        "accessToken".into(),
        serde_json::Value::String(token.access_token.0.clone()),
    );
    body.insert(
        "expiresAt".into(),
        serde_json::Value::String(token.expires_at.clone()),
    );
    body.insert(
        "profileArn".into(),
        serde_json::Value::String(token.profile_arn.clone()),
    );
    if let Some(auth_method) = &token.auth_method {
        body.insert(
            "authMethod".into(),
            serde_json::Value::String(auth_method.clone()),
        );
    }
    if let Some(provider) = &token.provider {
        body.insert(
            "provider".into(),
            serde_json::Value::String(provider.clone()),
        );
    }
    let raw = serde_json::value::RawValue::from_string(serde_json::Value::Object(body).to_string())
        .map_err(|e| format!("serialize getAccessToken reply: {e}"))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

/// Whether a `getAccessToken` request asks for a forced refresh: its params
/// carry `forceRefresh: true`. Absent/`null`/`false` (KAS 0.66.x always sends
/// `{}`) is not forced; any other value is not forced either, with a warning —
/// the flag spends kiro-cli's refresh token, so only an explicit `true` may.
pub(crate) fn force_refresh_requested(params: &serde_json::Value) -> bool {
    match params.get("forceRefresh") {
        Some(serde_json::Value::Bool(forced)) => *forced,
        None | Some(serde_json::Value::Null) => false,
        Some(other) => {
            tracing::warn!(
                kind = type_name(other),
                "getAccessToken forceRefresh is not a boolean; not forcing a refresh"
            );
            false
        }
    }
}

/// Answer `_kiro/auth/getAccessToken`: run kiro-cli's token helper through
/// `source` (re-run on EVERY request, so a renewal or a mid-session `kiro-cli
/// login` is served on the next request) and relay its data. The `Err`
/// carries both the JSON-RPC error for KAS and the typed reason for the UI;
/// every failure is warn-logged — KAS surfaces a failed callback as a
/// mute/opaque turn (cyril-l7tw), so the log line is the cyril-side
/// breadcrumb. `Ok` also carries the stored-token fallback warning the first
/// time this connection serves from the fallback (`None` otherwise), for the
/// caller to surface once.
pub(crate) async fn respond_get_access_token(
    source: Option<&KasTokenSource>,
    force_refresh: bool,
) -> Result<(acp::ExtResponse, Option<String>), (acp::Error, KasTokenError)> {
    let fail = |error: KasTokenError| {
        tracing::warn!(error = %error, force_refresh, "getAccessToken failed");
        (error.to_acp(), error)
    };
    let source = source.ok_or_else(|| fail(KasTokenError::Unbound))?;
    let token = source.fetch(force_refresh).await.map_err(fail)?;
    tracing::info!(
        expires_at = token.expires_at(),
        force_refresh,
        origin = ?token.origin(),
        "getAccessToken served"
    );
    // Build first: a failed build must not consume the one-time warning.
    let response = build_response(&token).map_err(|detail| fail(KasTokenError::Reply(detail)))?;
    let warning = match token.origin() {
        TokenOrigin::Helper => None,
        TokenOrigin::StoredFallback { kiro_cli_version } => source
            .take_fallback_warning()
            .then(|| fallback_warning(kiro_cli_version.as_deref())),
    };
    Ok((response, warning))
}

/// The one-time user-visible warning for the stored-token fallback.
fn fallback_warning(kiro_cli_version: Option<&str>) -> String {
    let version = match kiro_cli_version {
        Some(v) => format!(" ({v})"),
        None => String::new(),
    };
    format!(
        "this kiro-cli{version} lacks `chat _ get-kas-token`, so cyril cannot renew the KAS \
         access token: it is serving the stored kiro token, and KAS sessions will expire with \
         it (about hourly) until kiro-cli is updated and cyril restarted"
    )
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use rstest::rstest;

    /// A success envelope line as kiro-cli 2.32.0 prints it (live shape,
    /// 2026-10-18: `{accessToken, expiresAt, profileArn, provider}`).
    const OK_LINE: &str = r#"{"kind":"getKasToken","data":{"accessToken":"AT-secret","expiresAt":"2026-10-18T19:47:39.083490355Z","profileArn":"arn:aws:codewhisperer:us-east-1:1:profile/X","provider":"Enterprise"}}"#;

    /// A finished run with the given exit code and output.
    fn captured(code: i32, stdout: &str, stderr: &str) -> Captured {
        Captured {
            status: exit_status(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[cfg(unix)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code << 8)
    }

    #[cfg(windows)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }

    fn body(resp: &acp::ExtResponse) -> serde_json::Value {
        serde_json::from_str(resp.0.get()).unwrap()
    }

    // C3: a success envelope relays exactly the TUI's fields, unchanged —
    // optional fields only when present.
    #[rstest]
    #[case::provider_only(OK_LINE, serde_json::json!({
        "accessToken": "AT-secret",
        "expiresAt": "2026-10-18T19:47:39.083490355Z",
        "profileArn": "arn:aws:codewhisperer:us-east-1:1:profile/X",
        "provider": "Enterprise",
    }))]
    #[case::all_five(
        r#"{"kind":"getKasToken","data":{"accessToken":"AT","expiresAt":"2027-01-01T00:00:00Z","profileArn":"arn:p","authMethod":"social","provider":"Github","futureKey":1}}"#,
        serde_json::json!({
            "accessToken": "AT",
            "expiresAt": "2027-01-01T00:00:00Z",
            "profileArn": "arn:p",
            "authMethod": "social",
            "provider": "Github",
        })
    )]
    #[case::required_only_with_null_optional(
        r#"{"kind":"getKasToken","data":{"accessToken":"AT","expiresAt":"2027-01-01T00:00:00Z","profileArn":"arn:p","authMethod":null}}"#,
        serde_json::json!({"accessToken": "AT", "expiresAt": "2027-01-01T00:00:00Z", "profileArn": "arn:p"})
    )]
    #[case::noise_lines_then_envelope_crlf(
        "warming up\r\n\r\nstill here\r\n{\"kind\":\"getKasToken\",\"data\":{\"accessToken\":\"AT\",\"expiresAt\":\"E\",\"profileArn\":\"arn:p\"}}\r\n\r\n",
        serde_json::json!({"accessToken": "AT", "expiresAt": "E", "profileArn": "arn:p"})
    )]
    fn success_envelope_relays_the_data_fields(
        #[case] stdout: &str,
        #[case] expected: serde_json::Value,
    ) {
        let token = interpret("kiro-cli chat _ get-kas-token", &captured(0, stdout, ""))
            .expect("success envelope");
        assert_eq!(body(&build_response(&token).unwrap()), expected);
    }

    /// The variant name of a [`KasTokenError`], for table assertions.
    fn variant(e: &KasTokenError) -> &'static str {
        match e {
            KasTokenError::Unbound => "Unbound",
            KasTokenError::Spawn { .. } => "Spawn",
            KasTokenError::Timeout { .. } => "Timeout",
            KasTokenError::Io { .. } => "Io",
            KasTokenError::NonZeroExit { .. } => "NonZeroExit",
            KasTokenError::ErrorEnvelope { .. } => "ErrorEnvelope",
            KasTokenError::Malformed { .. } => "Malformed",
            KasTokenError::Reply(_) => "Reply",
            KasTokenError::HelperMissing { .. } => "HelperMissing",
            KasTokenError::StoredUnservable { .. } => "StoredUnservable",
            KasTokenError::StoredStale { .. } => "StoredStale",
        }
    }

    // C2: every failure shape maps to its own typed error — never a token,
    // never a default — and no diagnostic echoes the secret.
    #[rstest]
    #[case::error_envelope_exit_1(1, r#"{"kind":"error","data":{"message":"You are not logged in. Please log in with `kiro-cli login`."}}"#, "", "ErrorEnvelope", "You are not logged in")]
    #[case::error_envelope_exit_0(
        0,
        r#"{"kind":"error","data":{"message":"refresh refused"}}"#,
        "",
        "ErrorEnvelope",
        "kiro-cli login"
    )]
    #[case::nonzero_without_envelope(3, "", "boom: network down\n", "NonZeroExit", "network down")]
    #[case::nonzero_with_success_envelope(2, OK_LINE, "", "NonZeroExit", "exited with")]
    #[case::not_json(0, "AT-secret is the token", "", "Malformed", "not JSON")]
    #[case::empty_stdout(0, "\n\n", "", "Malformed", "no output")]
    #[case::unknown_kind(
        0,
        r#"{"kind":"getTelemetryCredentials","data":{"accessToken":"AT-secret"}}"#,
        "",
        "Malformed",
        "unexpected envelope kind"
    )]
    #[case::array_envelope(0, "[1,2]", "", "Malformed", "array")]
    #[case::data_not_object(
        0,
        r#"{"kind":"getKasToken","data":"AT-secret"}"#,
        "",
        "Malformed",
        "`data` is a string"
    )]
    #[case::missing_token(
        0,
        r#"{"kind":"getKasToken","data":{"expiresAt":"E","profileArn":"arn:p"}}"#,
        "",
        "Malformed",
        "`accessToken` is missing"
    )]
    #[case::empty_token(
        0,
        r#"{"kind":"getKasToken","data":{"accessToken":"","expiresAt":"E","profileArn":"arn:p"}}"#,
        "",
        "Malformed",
        "`accessToken` is empty"
    )]
    #[case::missing_arn(
        0,
        r#"{"kind":"getKasToken","data":{"accessToken":"AT-secret","expiresAt":"E"}}"#,
        "",
        "Malformed",
        "`profileArn` is missing"
    )]
    #[case::numeric_expiry(0, r#"{"kind":"getKasToken","data":{"accessToken":"AT-secret","expiresAt":12,"profileArn":"arn:p"}}"#, "", "Malformed", "`expiresAt` is a number")]
    #[case::object_provider(0, r#"{"kind":"getKasToken","data":{"accessToken":"AT-secret","expiresAt":"E","profileArn":"arn:p","provider":{"x":"AT-secret"}}}"#, "", "Malformed", "`provider` is a object")]
    #[case::error_without_message(
        1,
        r#"{"kind":"error","data":{}}"#,
        "",
        "NonZeroExit",
        "exited with"
    )]
    fn failure_shapes_map_to_typed_errors(
        #[case] code: i32,
        #[case] stdout: &str,
        #[case] stderr: &str,
        #[case] expected_variant: &str,
        #[case] expected_text: &str,
    ) {
        let error = interpret(
            "kiro-cli chat _ get-kas-token",
            &captured(code, stdout, stderr),
        )
        .expect_err("must not yield a token");
        assert_eq!(variant(&error), expected_variant, "{error}");
        let shown = error.to_string();
        assert!(shown.contains(expected_text), "{shown}");
        assert!(!shown.contains("AT-secret"), "secret echoed: {shown}");
        assert!(!format!("{error:?}").contains("AT-secret"), "{error:?}");
    }

    // C2: the error-envelope hint is never doubled, and appended when absent.
    #[test]
    fn error_envelope_hint_appears_exactly_once() {
        for message in [
            "You are not logged in. Please log in with `kiro-cli login`.",
            "refresh token revoked",
        ] {
            let shown = KasTokenError::error_envelope(message).to_string();
            assert_eq!(shown.matches("kiro-cli login").count(), 1, "{shown}");
        }
    }

    // C2: the KAS-facing code separates an auth state the user must act on
    // (error envelope) from helper/transport failures.
    #[test]
    fn acp_codes_separate_auth_state_from_helper_failure() {
        assert_eq!(
            KasTokenError::error_envelope("logged out").to_acp().code,
            acp::Error::new(JSONRPC_AUTH_REQUIRED, "").code
        );
        let timeout = KasTokenError::Timeout {
            command: "kiro-cli chat _ get-kas-token".into(),
            secs: 30,
        };
        assert_eq!(
            timeout.to_acp().code,
            acp::Error::new(JSONRPC_INTERNAL_ERROR, "").code
        );
        assert!(timeout.to_acp().message.contains("30 s"));
    }

    // C4: `--force-refresh` is appended iff the request asks, after the
    // invocation prefix — a plain kiro-cli and a WSL-launcher prefix alike.
    #[rstest]
    #[case::plain(&["kiro-cli"], false, &["chat", "_", "get-kas-token"])]
    #[case::plain_forced(&["kiro-cli"], true, &["chat", "_", "get-kas-token", "--force-refresh"])]
    #[case::wsl_forced(&["wsl", "-d", "Ubuntu", "kiro-cli"], true, &["-d", "Ubuntu", "kiro-cli", "chat", "_", "get-kas-token", "--force-refresh"])]
    fn invocation_appends_force_refresh_iff_requested(
        #[case] prefix: &[&str],
        #[case] force: bool,
        #[case] expected_args: &[&str],
    ) {
        let source = KasTokenSource::new(
            AgentCommand::new(prefix[0])
                .with_args(prefix[1..].iter().map(|a| a.to_string()).collect()),
            SpawnEnvironment::Inherit,
        );
        let invocation = source.invocation(force);
        assert_eq!(invocation.program(), prefix[0]);
        assert_eq!(invocation.args(), expected_args);
    }

    // C4: only an explicit `forceRefresh: true` forces a refresh.
    #[rstest]
    #[case::kas_066_empty_params(serde_json::json!({}), false)]
    #[case::explicit_true(serde_json::json!({"forceRefresh": true}), true)]
    #[case::explicit_false(serde_json::json!({"forceRefresh": false}), false)]
    #[case::null(serde_json::json!({"forceRefresh": null}), false)]
    #[case::string_true(serde_json::json!({"forceRefresh": "true"}), false)]
    #[case::params_null(serde_json::Value::Null, false)]
    fn force_refresh_only_on_explicit_true(
        #[case] params: serde_json::Value,
        #[case] forced: bool,
    ) {
        assert_eq!(force_refresh_requested(&params), forced);
    }

    // C11 custodian: Debug of the token (and of a response-bearing struct)
    // never prints the secret.
    #[test]
    fn kas_token_debug_is_redacted() {
        let token = interpret("k", &captured(0, OK_LINE, "")).unwrap();
        let dbg = format!("{token:?}");
        assert!(!dbg.contains("AT-secret"), "token leaked in Debug: {dbg}");
        assert!(dbg.contains("redacted"));
    }

    /// A fake kiro-cli runnable on this host without exec'ing a freshly
    /// written binary: a POSIX `sh` script on unix, a `.cmd` batch file on
    /// Windows (std runs batch files through cmd.exe). The script journals
    /// its argv to `argv.txt` beside it. Deliberately NOT cfg-gated: the
    /// platform-neutral process tests below use it on every host.
    pub(crate) fn fake_kiro_cli(
        dir: &std::path::Path,
        posix_body: &str,
        cmd_body: &str,
    ) -> AgentCommand {
        if cfg!(windows) {
            let script = dir.join("fake-kiro-cli.cmd");
            std::fs::write(
                &script,
                format!(
                    "@echo off\r\necho %* > \"%~dp0argv.txt\"\r\n{}\r\n",
                    cmd_body.replace('\n', "\r\n")
                ),
            )
            .unwrap();
            AgentCommand::new(script.to_str().unwrap())
        } else {
            let script = dir.join("fake-kiro-cli.sh");
            std::fs::write(
                &script,
                format!("printf '%s ' \"$@\" > \"$(dirname \"$0\")/argv.txt\"\n{posix_body}\n"),
            )
            .unwrap();
            AgentCommand::new("/bin/sh").with_args(vec![script.to_str().unwrap().to_string()])
        }
    }

    // C4 + C3 at the process seam: the forced request reaches kiro-cli as
    // `chat _ get-kas-token --force-refresh`, and the relayed reply carries
    // the helper's data unchanged.
    #[tokio::test]
    async fn fetch_runs_the_helper_with_force_refresh_and_relays_data() {
        let dir = tempfile::tempdir().unwrap();
        let envelope = r#"{"kind":"getKasToken","data":{"accessToken":"AT-proc","expiresAt":"2099-01-01T00:00:00Z","profileArn":"arn:p","authMethod":"IdC"}}"#;
        let source = KasTokenSource::new(
            fake_kiro_cli(
                dir.path(),
                &format!("printf '%s\\n' '{envelope}'"),
                &format!("echo {envelope}"),
            ),
            SpawnEnvironment::Inherit,
        );
        let (reply, warning) = respond_get_access_token(Some(&source), true)
            .await
            .expect("helper answers");
        assert_eq!(warning, None, "the helper path never warns");
        assert_eq!(
            body(&reply),
            serde_json::json!({
                "accessToken": "AT-proc",
                "expiresAt": "2099-01-01T00:00:00Z",
                "profileArn": "arn:p",
                "authMethod": "IdC",
            })
        );
        let argv = std::fs::read_to_string(dir.path().join("argv.txt")).unwrap();
        assert_eq!(argv.trim(), "chat _ get-kas-token --force-refresh");
    }

    // C10 (amendment): only clap's refusal of the helper itself — usage exit
    // 2, no stdout, a stderr line naming `get-kas-token` as an unrecognized
    // subcommand (2.5.1) or unexpected argument (2.4.1) — is a missing helper.
    // Every other failure stays a hard error (no fallback).
    #[rstest]
    #[case::unrecognized_subcommand_2_5_1(
        2,
        "",
        "error: unrecognized subcommand 'get-kas-token'\n\nUsage: kiro-cli-chat chat _ [OPTIONS] <COMMAND>\n\nFor more information, try '--help'.\n",
        "HelperMissing"
    )]
    #[case::unexpected_argument_2_4_1(
        2,
        "",
        "error: unexpected argument 'get-kas-token' found\n\nUsage: kiro-cli-chat chat [OPTIONS] [INPUT]\n",
        "HelperMissing"
    )]
    #[case::crlf_stderr(
        2,
        "",
        "error: unrecognized subcommand 'get-kas-token'\r\n",
        "HelperMissing"
    )]
    #[case::force_flag_unknown(
        2,
        "",
        "error: unexpected argument '--force-refresh' found\n",
        "NonZeroExit"
    )]
    #[case::other_subcommand_unknown(2, "", "error: unrecognized subcommand '_'\n", "NonZeroExit")]
    #[case::same_text_wrong_exit(
        1,
        "",
        "error: unrecognized subcommand 'get-kas-token'\n",
        "NonZeroExit"
    )]
    #[case::same_text_with_stdout(
        2,
        "partial",
        "error: unrecognized subcommand 'get-kas-token'\n",
        "NonZeroExit"
    )]
    #[case::error_envelope_logged_out(1, r#"{"kind":"error","data":{"message":"You are not logged in. Please log in with `kiro-cli login`."}}"#, "", "ErrorEnvelope")]
    #[case::legacy_2_6_envelope(1, r#"{"success":false,"error":"You are not logged in. Please log in with `kiro-cli login`."}"#, "", "NonZeroExit")]
    #[case::malformed_json(0, "not json", "", "Malformed")]
    fn missing_helper_is_detected_precisely(
        #[case] code: i32,
        #[case] stdout: &str,
        #[case] stderr: &str,
        #[case] expected_variant: &str,
    ) {
        let error = interpret(
            "kiro-cli chat _ get-kas-token",
            &captured(code, stdout, stderr),
        )
        .expect_err("never a token");
        assert_eq!(variant(&error), expected_variant, "{error}");
    }

    /// A fake kiro-cli WITHOUT the helper: answers `--version` with
    /// `kiro-cli 2.5.1`, refuses everything else exactly like clap does
    /// (exit 2, stderr only), and appends one line to `calls.txt` per helper
    /// attempt.
    pub(crate) fn kiro_cli_without_helper(dir: &std::path::Path) -> AgentCommand {
        fake_kiro_cli(
            dir,
            "d=\"$(dirname \"$0\")\"\nif [ \"$1\" = \"--version\" ]; then printf 'kiro-cli 2.5.1\\n'; exit 0; fi\necho call >> \"$d/calls.txt\"\nprintf \"error: unrecognized subcommand 'get-kas-token'\\n\\nUsage: kiro-cli-chat chat _ [OPTIONS] <COMMAND>\\n\" >&2\nexit 2",
            "if \"%1\"==\"--version\" (\n  echo kiro-cli 2.5.1\n  exit /b 0\n)\necho call >> \"%~dp0calls.txt\"\necho error: unrecognized subcommand 'get-kas-token' 1>&2\nexit /b 2",
        )
    }

    /// A fixture credential store holding one builder-id token expiring at
    /// `expires_at` and an active profile.
    pub(crate) fn store_with_token(dir: &std::path::Path, expires_at: &str) -> std::path::PathBuf {
        let db = dir.join("data.sqlite3");
        let conn = rusqlite::Connection::open(&db).unwrap();
        // Table/row names are split so this file never holds the joined
        // forms the `credential_store_token_rows_only_in_the_fallback` fence
        // scans for.
        conn.execute_batch(concat!(
            "CREATE TABLE auth",
            "_kv (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT);"
        ))
        .unwrap();
        let row = serde_json::json!({"access_token": "AT-stored", "expires_at": expires_at});
        conn.execute(
            concat!("INSERT INTO auth", "_kv VALUES (?1, ?2)"),
            [concat!("kirocli:", "odic:token"), &row.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO state VALUES ('api.codewhisperer.profile', ?1)",
            [r#"{"arn":"arn:aws:codewhisperer:us-east-1:1:profile/S"}"#],
        )
        .unwrap();
        db
    }

    // C10: a kiro-cli without the helper → the stored token is served, the
    // user is warned ONCE (with the version), and the helper is probed once
    // per process — the second request goes straight to the fallback.
    #[tokio::test]
    async fn missing_helper_falls_back_to_the_stored_token_and_warns_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_token(dir.path(), "2099-01-01T00:00:00Z");
        let source = KasTokenSource::new(
            kiro_cli_without_helper(dir.path()),
            SpawnEnvironment::Inherit,
        )
        .with_store(store);

        let (reply, warning) = respond_get_access_token(Some(&source), false)
            .await
            .expect("fallback serves the stored token");
        assert_eq!(
            body(&reply),
            serde_json::json!({
                "accessToken": "AT-stored",
                "expiresAt": "2099-01-01T00:00:00Z",
                "profileArn": "arn:aws:codewhisperer:us-east-1:1:profile/S",
            })
        );
        let warning = warning.expect("first fallback warns");
        assert!(
            warning.contains("lacks `chat _ get-kas-token`"),
            "{warning}"
        );
        assert!(warning.contains("2.5.1"), "names the version: {warning}");
        assert!(warning.contains("hourly"), "{warning}");

        let (_, again) = respond_get_access_token(Some(&source), true)
            .await
            .expect("still served");
        assert_eq!(again, None, "the warning is not repeated per request");
        let calls = std::fs::read_to_string(dir.path().join("calls.txt")).unwrap();
        assert_eq!(calls.lines().count(), 1, "helper re-probed: {calls:?}");
    }

    // C10: on the fallback, an expired stored token is a hard, actionable
    // auth error (only kiro-cli could renew it) — never a known-bad reply.
    #[tokio::test]
    async fn missing_helper_with_expired_store_is_a_hard_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_token(dir.path(), "2020-01-01T00:00:00Z");
        let source = KasTokenSource::new(
            kiro_cli_without_helper(dir.path()),
            SpawnEnvironment::Inherit,
        )
        .with_store(store);
        let (acp_error, error) = respond_get_access_token(Some(&source), false)
            .await
            .expect_err("expired stored token");
        assert_eq!(variant(&error), "StoredStale", "{error}");
        assert_eq!(
            acp_error.code,
            acp::Error::new(JSONRPC_AUTH_REQUIRED, "").code
        );
        let shown = error.to_string();
        assert!(
            shown.contains("kiro-cli login") && shown.contains("update kiro-cli"),
            "{shown}"
        );
    }

    // C10: any OTHER helper failure never falls back, even with a perfectly
    // servable store next to it.
    #[tokio::test]
    async fn other_helper_failures_never_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_with_token(dir.path(), "2099-01-01T00:00:00Z");
        let logged_out = r#"{"kind":"error","data":{"message":"You are not logged in. Please log in with `kiro-cli login`."}}"#;
        let source = KasTokenSource::new(
            fake_kiro_cli(
                dir.path(),
                &format!("printf '%s\\n' '{logged_out}'\nexit 1"),
                &format!("echo {logged_out}\nexit /b 1"),
            ),
            SpawnEnvironment::Inherit,
        )
        .with_store(store);
        let (_, error) = respond_get_access_token(Some(&source), false)
            .await
            .expect_err("logged out is a hard error");
        assert_eq!(variant(&error), "ErrorEnvelope", "{error}");
    }

    // C9: two concurrent requests through one source never overlap — the
    // fake helper takes a lock dir (atomic mkdir), lingers, and fails if
    // another instance already holds it, so an overlap surfaces as an error.
    #[tokio::test]
    async fn fetch_serializes_concurrent_requests() {
        let dir = tempfile::tempdir().unwrap();
        let envelope = r#"{"kind":"getKasToken","data":{"accessToken":"AT","expiresAt":"2099-01-01T00:00:00Z","profileArn":"arn:p"}}"#;
        let source = KasTokenSource::new(
            fake_kiro_cli(
                dir.path(),
                &format!(
                    "d=\"$(dirname \"$0\")/held\"\nmkdir \"$d\" || exit 9\nsleep 0.3\nrmdir \"$d\"\nprintf '%s\\n' '{envelope}'"
                ),
                &format!(
                    "mkdir \"%~dp0held\" || exit /b 9\nping -n 2 127.0.0.1 > NUL\nrmdir \"%~dp0held\"\necho {envelope}"
                ),
            ),
            SpawnEnvironment::Inherit,
        );
        let (first, second) = tokio::join!(source.fetch(false), source.fetch(false));
        assert!(first.is_ok(), "{first:?}");
        assert!(second.is_ok(), "{second:?}");
    }

    /// A fake helper that takes a lock dir beside it (atomic mkdir), lingers,
    /// and exits 9 if another instance already holds it — so any overlap of
    /// two runs surfaces as an error.
    fn overlap_detecting_kiro_cli(dir: &std::path::Path) -> AgentCommand {
        let envelope = r#"{"kind":"getKasToken","data":{"accessToken":"AT","expiresAt":"2099-01-01T00:00:00Z","profileArn":"arn:p"}}"#;
        fake_kiro_cli(
            dir,
            &format!(
                "d=\"$(dirname \"$0\")/held\"\nmkdir \"$d\" || exit 9\nsleep 0.3\nrmdir \"$d\"\nprintf '%s\\n' '{envelope}'"
            ),
            &format!(
                "mkdir \"%~dp0held\" || exit /b 9\nping -n 2 127.0.0.1 > NUL\nrmdir \"%~dp0held\"\necho {envelope}"
            ),
        )
    }

    /// An absolute fixture path on this platform (`/a/b` or `C:\a\b`). Fully
    /// fake: nothing here reads the real environment or filesystem.
    fn abs(rel: &str) -> std::path::PathBuf {
        if cfg!(windows) {
            std::path::PathBuf::from(format!("C:\\{}", rel.replace('/', "\\")))
        } else {
            std::path::PathBuf::from(format!("/{rel}"))
        }
    }

    /// A fake executable probe true only for `present`.
    fn executables(present: Vec<std::path::PathBuf>) -> impl Fn(&std::path::Path) -> bool {
        move |p: &std::path::Path| present.iter().any(|x| x == p)
    }

    fn path_var(dirs: &[std::path::PathBuf]) -> std::ffi::OsString {
        std::env::join_paths(dirs).unwrap()
    }

    // C7b (PR #173 review): a kiro-cli on PATH wins, in PATH order — even
    // when a TUI fallback location also has one; relative PATH entries are
    // skipped (TUI `YM` parity).
    #[test]
    fn free_kiro_cli_prefers_path_in_order_and_skips_relative_entries() {
        let name = kiro_cli_file_name();
        let first = abs("opt/first").join(&name);
        let second = abs("opt/second").join(&name);
        let home = abs("home/u");
        let path = std::env::join_paths([
            std::path::PathBuf::from("relative/bin"),
            abs("opt/first"),
            abs("opt/second"),
        ])
        .unwrap();
        let found = resolve_free_kiro_cli(
            Some(&path),
            Some(&home),
            None,
            executables(vec![
                std::path::PathBuf::from("relative/bin").join(&name),
                second.clone(),
                first.clone(),
                home.join(".local/bin/kiro-cli"),
            ]),
        );
        assert_eq!(found.program(), first.to_str().unwrap());
        assert!(found.args().is_empty());
    }

    // C7b: with no kiro-cli on PATH, the Kiro TUI's install locations are
    // tried in its order — `~/.local/bin`, `/usr/local/bin`,
    // `/opt/homebrew/bin` (tui.js 2.32.0 `ku`). Windows has none (the TUI's
    // literal paths name the extension-less unix binary; the MSI's
    // kiro-cli.exe is found on PATH), so it keeps the bare name there.
    #[rstest]
    #[case::home_local_bin(&["HOME/.local/bin/kiro-cli", "/usr/local/bin/kiro-cli", "/opt/homebrew/bin/kiro-cli"], "HOME/.local/bin/kiro-cli")]
    #[case::usr_local_bin(&["/usr/local/bin/kiro-cli", "/opt/homebrew/bin/kiro-cli"], "/usr/local/bin/kiro-cli")]
    #[case::homebrew(&["/opt/homebrew/bin/kiro-cli"], "/opt/homebrew/bin/kiro-cli")]
    #[case::nowhere(&[], "kiro-cli")]
    fn free_kiro_cli_falls_back_to_the_tui_locations(
        #[case] present: &[&str],
        #[case] expected_unix: &str,
    ) {
        let home = abs("home/u");
        let expand = |p: &str| match p.strip_prefix("HOME/") {
            Some(rest) => home.join(rest),
            None => std::path::PathBuf::from(p),
        };
        let found = resolve_free_kiro_cli(
            Some(&path_var(&[abs("usr/bin")])),
            Some(&home),
            None,
            executables(present.iter().map(|p| expand(p)).collect()),
        );
        let expected = if cfg!(windows) {
            "kiro-cli".to_string()
        } else {
            expand(expected_unix).to_str().unwrap().to_string()
        };
        assert_eq!(found.program(), expected);
    }

    // C7b: PATH entries at or under the working directory are skipped (TUI
    // `YM`), so a kiro-cli planted in the workspace cannot shadow the real
    // one; a filesystem-root cwd only excludes itself.
    #[test]
    fn free_kiro_cli_skips_path_entries_inside_the_cwd() {
        let name = kiro_cli_file_name();
        let ws = abs("work/ws");
        let real = abs("opt/kiro").join(&name);
        let path = path_var(&[ws.join("bin"), ws.clone(), abs("opt/kiro")]);
        let found = resolve_free_kiro_cli(
            Some(&path),
            None,
            Some(&ws),
            executables(vec![
                ws.join("bin").join(&name),
                ws.join(&name),
                real.clone(),
            ]),
        );
        assert_eq!(found.program(), real.to_str().unwrap());

        let root = abs("");
        let at_root = abs("usr/bin").join(&name);
        let found = resolve_free_kiro_cli(
            Some(&path_var(&[abs("usr/bin")])),
            None,
            Some(&root),
            executables(vec![at_root.clone()]),
        );
        assert_eq!(found.program(), at_root.to_str().unwrap());
    }

    // Review 3 (C9c): the lock key is the RESOLVED binary + args — an
    // absolute spelling and a bare name found on PATH share a key, as do two
    // paths that canonicalize to one file; different args (another WSL
    // distro) do not; an unresolvable relative spelling keys on itself.
    #[test]
    fn lock_key_unifies_spellings_of_one_binary() {
        let name = kiro_cli_file_name();
        let dir = abs("usr/local/bin");
        let real = dir.join(&name);
        let link = abs("home/u/.local/bin").join(&name);
        let present = executables(vec![real.clone(), link.clone()]);
        let canonical = |p: &std::path::Path| -> Option<std::path::PathBuf> {
            Some(if p == link.as_path() {
                real.clone()
            } else {
                p.to_path_buf()
            })
        };
        let key = |cmd: AgentCommand, path: &[std::path::PathBuf]| {
            lock_key(&cmd, Some(&path_var(path)), &present, canonical)
        };
        let absolute = key(AgentCommand::new(real.to_str().unwrap()), &[]);
        assert_eq!(
            key(AgentCommand::new("kiro-cli"), std::slice::from_ref(&dir)),
            absolute
        );
        assert_eq!(
            key(AgentCommand::new(link.to_str().unwrap()), &[]),
            absolute
        );
        assert_eq!(
            key(AgentCommand::new("kiro-cli"), &[abs("home/u/.local/bin")]),
            absolute,
            "a symlinked PATH hit canonicalizes to the same binary"
        );

        let wsl = |distro: &str| {
            AgentCommand::new(real.to_str().unwrap()).with_args(vec![
                "-d".to_string(),
                distro.to_string(),
                "kiro-cli".to_string(),
            ])
        };
        assert_ne!(key(wsl("Ubuntu"), &[]), key(wsl("Debian"), &[]));
        assert!(key(wsl("Ubuntu"), &[]).ends_with(" -d Ubuntu kiro-cli"));

        assert_eq!(
            key(AgentCommand::new("bin/kiro-cli"), &[dir]),
            "bin/kiro-cli",
            "an unresolvable relative spelling keys on itself"
        );
    }

    // Review 3 (C9c) at the process seam: two sources naming ONE binary by
    // different spellings (absolute vs bare name on the source's PATH) never
    // run the helper concurrently.
    #[cfg(unix)]
    #[tokio::test]
    async fn fetch_serializes_across_spellings_of_one_binary() {
        let dir = tempfile::tempdir().unwrap();
        let fake = overlap_detecting_kiro_cli(dir.path()); // `/bin/sh <script>`
        let sh = std::fs::canonicalize(fake.program()).unwrap();
        let sh_dir = sh.parent().unwrap().to_path_buf();
        let absolute = KasTokenSource::new(
            AgentCommand::new(sh.to_str().unwrap()).with_args(fake.args().to_vec()),
            SpawnEnvironment::Inherit,
        );
        let mut env = std::collections::BTreeMap::new();
        env.insert(std::ffi::OsString::from("PATH"), sh_dir.into_os_string());
        let bare = KasTokenSource::new(
            AgentCommand::new(sh.file_name().unwrap().to_str().unwrap())
                .with_args(fake.args().to_vec()),
            SpawnEnvironment::Replace(env),
        );
        let (first, second) = tokio::join!(absolute.fetch(false), bare.fetch(false));
        assert!(first.is_ok(), "{first:?}");
        assert!(second.is_ok(), "{second:?}");
    }

    // Review 3 (nit): on Windows the cwd comparison ignores case (TUI `YM`
    // lowercases); on unix case matters.
    #[test]
    fn under_cwd_case_handling_follows_the_platform() {
        let ws = abs("Work/WS");
        let entry = abs("work/ws/bin");
        assert_eq!(under_cwd(&entry, Some(&ws)), cfg!(windows));
        assert!(under_cwd(&abs("Work/WS/bin"), Some(&ws)));
        assert!(!under_cwd(&abs("Work/other"), Some(&ws)));
    }

    // Review 3 (nit): executability is the current user's `access(X_OK)` on
    // unix — a file whose only execute bit belongs to nobody we are is not
    // executable; any regular file counts on Windows (TUI `y1e`).
    #[test]
    fn is_executable_file_follows_access_x_ok() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("kiro-cli");
        std::fs::write(&file, "").unwrap();
        assert!(
            !is_executable_file(dir.path()),
            "a directory is never executable"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert!(!is_executable_file(&file));
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
            assert!(is_executable_file(&file));
            // Executable for "others" only: not for the owner running cyril
            // (unless root, which access(2) treats as able to execute).
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o601)).unwrap();
            let root = nix::unistd::Uid::effective().is_root();
            assert_eq!(is_executable_file(&file), root);
        }
        #[cfg(not(unix))]
        assert!(is_executable_file(&file));
    }

    // C7b: no PATH and no home still resolve (to the bare name, whose spawn
    // failure names where cyril looked).
    #[test]
    fn free_kiro_cli_without_path_or_home_is_the_bare_name() {
        assert_eq!(
            resolve_free_kiro_cli(None, None, None, executables(Vec::new())).program(),
            "kiro-cli"
        );
    }

    // C9b (PR #173 review): serialization is PROCESS-wide per kiro-cli
    // invocation — two connections in one cyril process (e.g. the main
    // bridge and the workbench reviewer runtime) asking the same kiro-cli
    // never run its single-use-refresh-token helper concurrently.
    #[tokio::test]
    async fn fetch_serializes_across_sources_sharing_an_invocation() {
        let dir = tempfile::tempdir().unwrap();
        let invocation = overlap_detecting_kiro_cli(dir.path());
        let a = KasTokenSource::new(invocation.clone(), SpawnEnvironment::Inherit);
        let b = KasTokenSource::new(invocation, SpawnEnvironment::Inherit);
        let (first, second) = tokio::join!(a.fetch(false), b.fetch(false));
        assert!(first.is_ok(), "{first:?}");
        assert!(second.is_ok(), "{second:?}");
    }

    // C2: a helper that never answers is killed at the deadline and reported
    // as a timeout — the responder does not hang the bridge.
    #[tokio::test]
    async fn fetch_times_out_on_a_wedged_helper() {
        let dir = tempfile::tempdir().unwrap();
        let source = KasTokenSource::new(
            fake_kiro_cli(dir.path(), "exec sleep 30", "ping -n 31 127.0.0.1 > NUL"),
            SpawnEnvironment::Inherit,
        )
        .with_deadline(Duration::from_millis(500));
        let started = std::time::Instant::now();
        let (acp_error, error) = respond_get_access_token(Some(&source), false)
            .await
            .expect_err("a wedged helper must time out");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(variant(&error), "Timeout", "{error}");
        assert_eq!(
            acp_error.code,
            acp::Error::new(JSONRPC_INTERNAL_ERROR, "").code
        );
    }

    // C2: an absent kiro-cli is a spawn failure naming the command and how
    // to fix it; no source at all is Unbound.
    #[tokio::test]
    async fn fetch_reports_spawn_failure_and_unbound() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-kiro-cli");
        let source = KasTokenSource::new(
            AgentCommand::new(missing.to_str().unwrap()),
            SpawnEnvironment::Inherit,
        );
        let (_, error) = respond_get_access_token(Some(&source), false)
            .await
            .expect_err("missing binary");
        assert_eq!(variant(&error), "Spawn", "{error}");
        let shown = error.to_string();
        assert!(shown.contains("get-kas-token"), "{shown}");
        // A wrapper-style source names the --agent-command kiro-cli; only
        // the free path (whose gate test checks it) names PATH.
        assert!(shown.contains("--agent-command"), "{shown}");
        assert!(!shown.contains("on PATH"), "{shown}");

        let (_, error) = respond_get_access_token(None, false)
            .await
            .expect_err("no source");
        assert_eq!(variant(&error), "Unbound");
    }

    // C1 fence (cyril-zbu7 + amendment): kiro-cli's credential-store token
    // rows are read ONLY by the stored-token fallback module — the normal
    // path asks kiro-cli. The needles are assembled at compile time so this
    // file's source never contains the joined forms.
    #[test]
    fn credential_store_token_rows_only_in_the_fallback() {
        fn scan(dir: &std::path::Path, needles: &[&str], hits: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("readable dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    scan(&path, needles, hits);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && std::fs::read_to_string(&path)
                        .is_ok_and(|s| needles.iter().any(|n| s.contains(n)))
                {
                    hits.push(path);
                }
            }
        }
        let needles = [
            concat!("kirocli:", "odic:token"),
            concat!("kirocli:", "social:token"),
            concat!("auth", "_kv"),
        ];
        let crates_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ parent dir");
        let mut hits = Vec::new();
        scan(crates_root, &needles, &mut hits);
        let fallback = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/protocol/kas/stored_token.rs");
        assert_eq!(
            hits,
            vec![fallback],
            "credential-store token rows read outside the fallback module"
        );
    }

    // Executable slice-7 fence (dcc6 review F7c): the retired SSO-cache token
    // path must never be consulted again anywhere in the workspace's crates.
    // The needle is assembled at compile time so this file's source never
    // contains the joined form.
    #[test]
    fn sso_token_path_never_resurrected() {
        fn scan(dir: &std::path::Path, needle: &str, hits: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("readable dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    scan(&path, needle, hits);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && std::fs::read_to_string(&path).is_ok_and(|s| s.contains(needle))
                {
                    hits.push(path);
                }
            }
        }
        let needle = concat!("kiro-auth", "-token");
        let crates_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crates/ parent dir");
        let mut hits = Vec::new();
        scan(crates_root, needle, &mut hits);
        assert!(hits.is_empty(), "SSO token path resurrected in: {hits:?}");
    }
}
