//! "Always allow" on KAS (cyril-auxt): which scopes and resources the operator
//! may grant, and the grant the permission response echoes back.
//!
//! KAS persists an `allow_always` decision only from the response's
//! `_meta.kiro.consent{capability, scope, resource, workspaceRoot}`; it offers
//! no scope or folder choice on the wire, so the client composes them. Every
//! rule here mirrors Kiro's own TUI 2.32.0 (`kiro-tui-2.32.0.js`; byte offsets
//! in `.cyril-auxt/design.md`): the scope cycle (`Ov`), the resource rows
//! (`isr`, `Ub`), the custom-folder offer and its validation (`b2t`, `vde`,
//! `A2t`, `y2t`), and the rule for requests from another session (`_lt`,
//! `C8n`). The wire shape itself lives in `protocol::convert::kas`.

use crate::review::consent::PermissionConsent;
use crate::types::{
    PermissionOption, PermissionOptionId, PermissionOptionKind, PermissionResponse, SessionId,
    ToolCallId,
};

/// How long and how widely a grant applies. KAS writes `User` grants to
/// `~/.kiro/settings/permissions.yaml` and `Workspace` grants to the
/// workspace's own `permissions.yaml`; `Session` grants end with the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantScope {
    Session,
    Workspace,
    User,
}

impl GrantScope {
    /// Display and cycle order (`s3`).
    pub const ALL: [GrantScope; 3] = [Self::Session, Self::Workspace, Self::User];

    /// The next scope in the `s` cycle: session → workspace → user → session.
    pub fn next(self) -> Self {
        match self {
            Self::Session => Self::Workspace,
            Self::Workspace => Self::User,
            Self::User => Self::Session,
        }
    }

    /// The scope's name, both on screen (`Ol`) and on the wire.
    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Workspace => "workspace",
            Self::User => "user",
        }
    }

    /// The phrase the folder row and folder page use (`Cde`).
    pub fn folder_phrase(self) -> &'static str {
        match self {
            Self::Session => "for this session",
            Self::Workspace => "in this workspace",
            Self::User => "for your user",
        }
    }
}

/// What a grant covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantResource {
    /// Exactly the requested resource.
    Exact(String),
    /// A shell base-command pattern such as `git *`.
    Pattern(String),
    /// Everything under a folder the operator confirmed.
    Folder(AllowedFolder),
    /// Every use of the tool.
    WholeTool,
}

impl GrantResource {
    /// The `consent.resource` value KAS stores for this grant.
    pub fn wire(&self) -> String {
        match self {
            Self::Exact(resource) | Self::Pattern(resource) => resource.clone(),
            Self::Folder(folder) => folder.glob(),
            Self::WholeTool => "*".to_owned(),
        }
    }
}

/// A folder that passed [`FolderOffer::validate`]. Only validation constructs
/// one, so a folder grant can never carry an unchecked path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedFolder(String);

impl AllowedFolder {
    pub fn path(&self) -> &str {
        &self.0
    }

    /// `<folder>/**` — everything under the folder (`y2t`).
    pub fn glob(&self) -> String {
        if self.0.ends_with('/') {
            format!("{}**", self.0)
        } else {
            format!("{}/**", self.0)
        }
    }
}

/// The scope and resource an "Always allow" pick grants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlwaysAllowGrant {
    scope: GrantScope,
    resource: GrantResource,
}

impl AlwaysAllowGrant {
    pub fn new(scope: GrantScope, resource: GrantResource) -> Self {
        Self { scope, resource }
    }

    pub fn scope(&self) -> GrantScope {
        self.scope
    }

    pub fn resource(&self) -> &GrantResource {
        &self.resource
    }
}

/// One row of the scope page, in display order (`isr`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeChoice {
    /// `Trust "<resource>"` — exact match.
    Exact(String),
    /// `Trust "<cmd> *"` — shell base-command pattern.
    Pattern(String),
    /// `Allow a folder …` — opens the folder editor.
    Folder(FolderOffer),
    /// `Trust entire tool`.
    WholeTool,
}

impl ScopeChoice {
    /// The resource this row grants directly; `None` for the folder row,
    /// which needs the operator's folder first.
    pub fn resource(&self) -> Option<GrantResource> {
        match self {
            Self::Exact(resource) => Some(GrantResource::Exact(resource.clone())),
            Self::Pattern(resource) => Some(GrantResource::Pattern(resource.clone())),
            Self::Folder(_) => None,
            Self::WholeTool => Some(GrantResource::WholeTool),
        }
    }
}

/// Whether a request belongs to the main session (`Kb`): true when there is no
/// main session yet, the request names no session, or the two match.
pub fn is_main_session_request(request: &SessionId, main: Option<&SessionId>) -> bool {
    match main {
        None => true,
        Some(main) => main.as_str().is_empty() || request.as_str().is_empty() || main == request,
    }
}

/// The scope page's rows for `consent` at `scope`. `owned` is
/// [`is_main_session_request`] for the request.
pub fn scope_choices(
    consent: &PermissionConsent,
    scope: GrantScope,
    owned: bool,
) -> Vec<ScopeChoice> {
    let gated = gated_resource(consent);
    let mut choices = Vec::new();
    if let Some(exact) = gated.filter(|resource| *resource != "*") {
        choices.push(ScopeChoice::Exact(exact.to_owned()));
    }
    if let Some(pattern) =
        base_command_pattern(consent).filter(|pattern| Some(pattern.as_str()) != gated)
    {
        choices.push(ScopeChoice::Pattern(pattern));
    }
    if let Some(offer) =
        FolderOffer::for_consent(consent, owned).filter(|offer| offer.available_for(scope))
    {
        choices.push(ScopeChoice::Folder(offer));
    }
    choices.push(ScopeChoice::WholeTool);
    choices
}

/// The response an "Always allow" grant becomes (`C8n`).
///
/// A main-session grant is always sent with its echo. A grant on another
/// session's request is sent only when Kiro's TUI could record it for that
/// session (`_lt`); otherwise the request's `allow_once` option answers it
/// instead, without metadata — and when no `allow_once` is offered the grant
/// is sent as picked, as the TUI does.
pub fn resolve(
    consent: &PermissionConsent,
    owned: bool,
    options: &[PermissionOption],
    chosen: PermissionOptionId,
    grant: AlwaysAllowGrant,
) -> PermissionResponse {
    if !owned
        && !child_grant_allowed(consent, &grant)
        && let Some(once) = options
            .iter()
            .find(|option| option.kind == PermissionOptionKind::AllowOnce)
    {
        tracing::debug!(
            scope = grant.scope().label(),
            "always-allow on another session's request cannot be recorded for it; allowing once"
        );
        return PermissionResponse::Selected {
            option_id: once.id.clone(),
            trust_option: None,
        };
    }
    PermissionResponse::AlwaysAllow {
        option_id: chosen,
        grant,
    }
}

/// `_lt`: a grant on another session's request needs a capability, a
/// workspace root for workspace scope, and — unless it covers the whole tool —
/// a shell capability whose triggering resource is the resource itself.
fn child_grant_allowed(consent: &PermissionConsent, grant: &AlwaysAllowGrant) -> bool {
    let Some(capability) = consent.wire_capability() else {
        return false;
    };
    if grant.scope() == GrantScope::Workspace && workspace_root(consent).is_none() {
        return false;
    }
    if grant.resource() == &GrantResource::WholeTool {
        return true;
    }
    let resource = non_empty(consent.resource());
    let triggering = non_empty(consent.triggering_resource());
    is_shell_capability(capability)
        && triggering.is_none_or(|triggering| Some(triggering) == resource)
}

/// A permission request as the approval queue sees it: who asked, for which
/// tool call, and what it offers. Both the request just granted and each
/// request still waiting are described this way.
#[derive(Debug, Clone, Copy)]
pub struct QueuedRequest<'a> {
    pub session_id: &'a SessionId,
    pub tool_call_id: &'a ToolCallId,
    pub tool_title: &'a str,
    pub consent: Option<&'a PermissionConsent>,
    pub options: &'a [PermissionOption],
}

impl<'a> QueuedRequest<'a> {
    fn allow_once(&self) -> Option<&'a PermissionOptionId> {
        self.options
            .iter()
            .find(|option| option.kind == PermissionOptionKind::AllowOnce)
            .map(|option| &option.id)
    }

    fn offers_allow_always(&self) -> bool {
        self.options
            .iter()
            .any(|option| option.kind == PermissionOptionKind::AllowAlways)
    }

    fn wire_capability(&self) -> Option<&str> {
        self.consent
            .and_then(|consent| non_empty(consent.wire_capability()))
    }

    fn workspace_root(&self) -> Option<&str> {
        self.consent
            .and_then(|consent| consent.workspace_root())
            .and_then(|root| root.to_str())
            .filter(|root| !root.trim().is_empty())
    }
}

/// Which waiting approvals an "Always allow" answer also settles (`k8n`).
///
/// Kiro's TUI releases queued approvals in exactly two cases, and answers each
/// with its `allow_once` option:
/// - a whole-tool grant releases every queued request with the same
///   capability key — capability (shell names fold together), workspace root
///   and session (`Clt`);
/// - a grant recorded for another session (`_lt`) releases that session's
///   queued requests it matches (`D4`).
///
/// Exact, pattern and folder grants on the main session release nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueRelease {
    Nothing,
    SameCapability(CapabilityKey),
    ChildGrant(ChildGrant),
}

/// `Clt`: the key a whole-tool grant releases by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityKey {
    /// A KAS request naming its capability.
    Consent {
        class: String,
        workspace_root: Option<String>,
        session: String,
    },
    /// No consent capability: the tool id, else the tool call's title.
    Tool(String),
}

/// `_lt`'s grant record for a request from another session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildGrant {
    capability: String,
    matcher: ChildMatch,
    scope: GrantScope,
    owner: SessionId,
    /// The granted tool call, for a non-session scope: such a grant only
    /// settles that same operation (`operationId`).
    operation: Option<ToolCallId>,
    workspace_root: Option<String>,
}

/// `h8n`: how a child grant matches a resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildMatch {
    Whole,
    Exact(String),
    Pattern(String),
}

impl QueueRelease {
    /// What `response` — the answer just sent for `granted` — releases.
    pub fn for_answer(
        granted: &QueuedRequest<'_>,
        response: &PermissionResponse,
        main: Option<&SessionId>,
    ) -> Self {
        let PermissionResponse::AlwaysAllow { grant, .. } = response else {
            return Self::Nothing;
        };
        if !is_main_session_request(granted.session_id, main)
            && let Some(child) = ChildGrant::record(granted, grant)
        {
            return Self::ChildGrant(child);
        }
        if grant.resource() != &GrantResource::WholeTool {
            return Self::Nothing;
        }
        match capability_key(granted) {
            Some(key) => Self::SameCapability(key),
            None => Self::Nothing,
        }
    }

    /// The `allow_once` option to answer `queued` with, when this release
    /// covers it. A covered request that offers no `allow_once` stays queued.
    pub fn covers<'q>(
        &self,
        queued: &QueuedRequest<'q>,
        main: Option<&SessionId>,
    ) -> Option<&'q PermissionOptionId> {
        let covered = match self {
            Self::Nothing => false,
            Self::SameCapability(key) => capability_key(queued).as_ref() == Some(key),
            Self::ChildGrant(child) => child.matches(queued, main),
        };
        if covered { queued.allow_once() } else { None }
    }
}

/// `Clt` for the KAS engine.
fn capability_key(request: &QueuedRequest<'_>) -> Option<CapabilityKey> {
    match request.wire_capability() {
        Some(capability) => Some(CapabilityKey::Consent {
            class: if is_shell_capability(capability) {
                "shell".to_owned()
            } else {
                capability.to_owned()
            },
            workspace_root: request.workspace_root().map(str::to_owned),
            session: request.session_id.as_str().to_owned(),
        }),
        None => request
            .consent
            .and_then(|consent| non_empty(consent.tool_id()))
            .or_else(|| non_empty(Some(request.tool_title)))
            .map(|tool| CapabilityKey::Tool(tool.to_owned())),
    }
}

impl ChildGrant {
    /// `_lt` + `h8n`: the record Kiro's TUI keeps for a grant on another
    /// session's request, or `None` when it would not record one.
    fn record(granted: &QueuedRequest<'_>, grant: &AlwaysAllowGrant) -> Option<Self> {
        let consent = granted.consent?;
        if !child_grant_allowed(consent, grant) || granted.session_id.as_str().is_empty() {
            return None;
        }
        let capability = granted.wire_capability()?.to_owned();
        let matcher = match grant.resource() {
            GrantResource::WholeTool => ChildMatch::Whole,
            resource => {
                let wire = resource.wire();
                let gated = gated_resource(consent);
                if base_command_pattern(consent).as_deref() == Some(wire.as_str())
                    && Some(wire.as_str()) != gated
                {
                    ChildMatch::Pattern(wire)
                } else {
                    ChildMatch::Exact(wire)
                }
            }
        };
        Some(Self {
            capability,
            matcher,
            scope: grant.scope(),
            owner: granted.session_id.clone(),
            operation: (grant.scope() != GrantScope::Session).then(|| granted.tool_call_id.clone()),
            workspace_root: granted.workspace_root().map(str::to_owned),
        })
    }

    /// `D4`: whether this grant settles `queued`.
    fn matches(&self, queued: &QueuedRequest<'_>, main: Option<&SessionId>) -> bool {
        if is_main_session_request(queued.session_id, main) {
            return false;
        }
        let Some(consent) = queued.consent else {
            return false;
        };
        let Some(capability) = queued.wire_capability() else {
            return false;
        };
        if capability != self.capability
            || queued.session_id != &self.owner
            || consent.is_explicit_ask()
            || !queued.offers_allow_always()
            || self
                .operation
                .as_ref()
                .is_some_and(|operation| operation != queued.tool_call_id)
        {
            return false;
        }
        let root = queued.workspace_root();
        if self.workspace_root.is_some() && self.workspace_root.as_deref() != root {
            return false;
        }
        if self.scope == GrantScope::Workspace
            && (root.is_none() || self.workspace_root.as_deref() != root)
        {
            return false;
        }
        let resource = match &self.matcher {
            ChildMatch::Whole => return true,
            ChildMatch::Exact(resource) | ChildMatch::Pattern(resource) => resource,
        };
        if !is_shell_capability(capability) {
            return false;
        }
        let requested = non_empty(consent.resource());
        let triggering = non_empty(consent.triggering_resource());
        if triggering.is_some_and(|triggering| Some(triggering) != requested) {
            return false;
        }
        let Some(target) = triggering.or(requested) else {
            return false;
        };
        match &self.matcher {
            ChildMatch::Pattern(_) => wildcard_matches(resource, target),
            _ => target == resource,
        }
    }
}

/// `y8n`: `*` matches any run of characters; everything else is literal.
fn wildcard_matches(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let [first, middle @ .., last] = parts.as_slice() else {
        return pattern == text;
    };
    if !text.starts_with(first) {
        return false;
    }
    let mut at = first.len();
    for part in middle {
        if part.is_empty() {
            continue;
        }
        match text[at..].find(part) {
            Some(found) => at += found + part.len(),
            None => return false,
        }
    }
    last.is_empty() || text[at..].ends_with(last)
}

/// The custom-folder offer for an `fs_write` request (`b2t`): the folder the
/// editor starts from, the file the folder must contain, and whether the
/// request came from inside a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderOffer {
    prefill: String,
    target: String,
    in_workspace: bool,
}

/// Why a typed folder cannot be granted (`vde`). The messages are Kiro's.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FolderError {
    #[error("Type a folder path.")]
    Empty,
    #[error("The folder contains unprintable characters.")]
    Unprintable,
    #[error("The folder cannot step up with \"..\".")]
    StepsUp,
    #[error("Spell out the folder instead of using ~.")]
    Tilde,
    #[error("The folder cannot start with \"#\".")]
    Hash,
    #[error("Type one folder, not a pattern; everything under it is included.")]
    Pattern,
    #[error("Type a folder inside the workspace, like src/MyPackage.")]
    NotInsideWorkspace,
    #[error("Type the full path of the folder.")]
    NotFullPath,
    #[error("The folder must contain {target}.")]
    MissingTarget { target: String },
}

impl FolderOffer {
    /// The offer for `consent`, or `None` when Kiro's TUI offers no folder:
    /// another session's request, a capability other than `fs_write`, an
    /// explicit ask, no resource or `*`, a glob, an absolute path inside a
    /// workspace, a path that steps up with `..`, or a file at the top.
    pub fn for_consent(consent: &PermissionConsent, owned: bool) -> Option<Self> {
        if !owned || consent.wire_capability() != Some("fs_write") || consent.is_explicit_ask() {
            return None;
        }
        let resource = gated_resource(consent)?;
        if resource.trim().is_empty() || resource == "*" || has_glob(resource) {
            return None;
        }
        let in_workspace = workspace_root(consent).is_some();
        if in_workspace && is_absolute(&resource.replace('\\', "/")) {
            return None;
        }
        let target = normalize_folder(resource)?;
        let prefill = parent_folder(&target);
        if prefill == "." {
            return None;
        }
        Some(Self {
            prefill,
            target,
            in_workspace,
        })
    }

    /// The folder the editor starts with: the requested file's folder.
    pub fn prefill(&self) -> &str {
        &self.prefill
    }

    /// The requested resource, normalised; the granted folder must contain it.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Whether the folder row is offered at `scope` (`A2t`): a workspace grant
    /// needs a workspace, a user grant needs a path outside one.
    pub fn available_for(&self, scope: GrantScope) -> bool {
        match scope {
            GrantScope::Session => true,
            GrantScope::Workspace => self.in_workspace,
            GrantScope::User => !self.in_workspace,
        }
    }

    /// Check a typed folder the way Kiro's TUI does (`vde`).
    pub fn validate(&self, input: &str) -> Result<AllowedFolder, FolderError> {
        let typed = input.trim();
        if typed.is_empty() {
            return Err(FolderError::Empty);
        }
        if typed.chars().any(is_unprintable) {
            return Err(FolderError::Unprintable);
        }
        let folder = normalize_folder(typed).ok_or(FolderError::StepsUp)?;
        if folder.starts_with('~') {
            return Err(FolderError::Tilde);
        }
        if folder.starts_with('#') {
            return Err(FolderError::Hash);
        }
        if has_glob(&folder) {
            return Err(FolderError::Pattern);
        }
        let absolute = is_absolute(&folder);
        if (self.in_workspace || !absolute) && (absolute || folder == ".") {
            return Err(if self.in_workspace {
                FolderError::NotInsideWorkspace
            } else {
                FolderError::NotFullPath
            });
        }
        if !folder_contains(&folder, &self.target) {
            return Err(FolderError::MissingTarget {
                target: self.target.clone(),
            });
        }
        Ok(AllowedFolder(folder))
    }
}

/// `Pb`: the capabilities Kiro treats as shell commands.
fn is_shell_capability(capability: &str) -> bool {
    matches!(capability, "shell" | "exec" | "shell:exec")
}

/// `Ub`'s gated resource: the triggering resource, else the resource.
fn gated_resource(consent: &PermissionConsent) -> Option<&str> {
    non_empty(consent.triggering_resource()).or_else(|| non_empty(consent.resource()))
}

/// `Ub`'s pattern: `"<first> *"` for a multi-word shell command whose first
/// word is not `*`, an assignment, or a privilege/env wrapper.
fn base_command_pattern(consent: &PermissionConsent) -> Option<String> {
    if !consent.wire_capability().is_some_and(is_shell_capability) {
        return None;
    }
    let mut words = gated_resource(consent)?.split_whitespace();
    let first = words.next()?;
    words.next()?;
    if first == "*" || first.contains('=') || matches!(first, "sudo" | "doas" | "env") {
        return None;
    }
    Some(format!("{first} *"))
}

fn workspace_root(consent: &PermissionConsent) -> Option<&std::path::Path> {
    consent
        .workspace_root()
        .filter(|root| root.to_str().is_none_or(|root| !root.trim().is_empty()))
}

/// The TUI's `oo`: a string with non-whitespace content, kept as sent.
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

/// `g2t`: characters that make a path a pattern.
fn has_glob(path: &str) -> bool {
    path.contains(['*', '?', '[', ']', '{', '}', '!', '$'])
}

/// `dl` + the tab/newline check: control characters and invisible formatting
/// characters.
fn is_unprintable(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{200B}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{2028}'..='\u{202F}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// `jR`: `/…` or a drive path `C:/…`.
fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || drive_prefix(path).is_some()
}

/// `C:/` when `path` starts with a drive letter and a slash.
fn drive_prefix(path: &str) -> Option<&str> {
    let bytes = path.as_bytes();
    (bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/')
        .then(|| &path[..3])
}

/// A bare drive such as `C:`.
fn is_bare_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// `Sde`: drop trailing slashes, keeping `/` and turning `C:` into `C:/`.
fn trim_trailing_slashes(path: &str) -> String {
    let mut end = path.len();
    while end > 1 && path.as_bytes()[end - 1] == b'/' {
        end -= 1;
    }
    let trimmed = &path[..end];
    if is_bare_drive(trimmed) {
        format!("{trimmed}/")
    } else {
        trimmed.to_owned()
    }
}

/// `nsr` = `f2t(Sde(kde(path)))`: forward slashes, no trailing slash, `.` and
/// empty segments removed. `None` when a segment is `..`.
fn normalize_folder(path: &str) -> Option<String> {
    let path = trim_trailing_slashes(&path.replace('\\', "/"));
    let prefix = match drive_prefix(&path) {
        Some(drive) => drive,
        None if path.starts_with('/') => "/",
        None => "",
    };
    let mut segments = Vec::new();
    for segment in path[prefix.len()..].split('/') {
        match segment {
            "" | "." => {}
            ".." => return None,
            segment => segments.push(segment),
        }
    }
    if segments.is_empty() {
        return Some(if prefix.is_empty() {
            ".".to_owned()
        } else {
            prefix.to_owned()
        });
    }
    Some(format!("{prefix}{}", segments.join("/")))
}

/// `tsr`: the folder containing `path`; `.` for a top-level relative name.
fn parent_folder(path: &str) -> String {
    let path = trim_trailing_slashes(path);
    match path.rfind('/') {
        None => ".".to_owned(),
        Some(0) => "/".to_owned(),
        Some(index) => {
            let parent = &path[..index];
            if is_bare_drive(parent) {
                format!("{parent}/")
            } else {
                parent.to_owned()
            }
        }
    }
}

/// `rsr`: whether `folder` contains `target`.
fn folder_contains(folder: &str, target: &str) -> bool {
    if folder == "." {
        return !is_absolute(target);
    }
    let is_root = folder == "/"
        || is_bare_drive(folder)
        || (folder.len() == 3 && drive_prefix(folder).is_some());
    if is_root {
        let root = if folder.ends_with('/') {
            folder.to_owned()
        } else {
            format!("{folder}/")
        };
        return target.starts_with(&root);
    }
    target == folder || target.starts_with(&format!("{folder}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    struct Consent<'a> {
        capability: &'a str,
        resource: Option<&'a str>,
        triggering: Option<&'a str>,
        root: Option<&'a str>,
        explicit: bool,
    }

    fn shell(command: &str) -> Consent<'_> {
        Consent {
            capability: "shell",
            resource: Some(command),
            triggering: Some(command),
            root: Some("/ws"),
            explicit: false,
        }
    }

    fn write<'a>(path: &'a str, root: Option<&'static str>) -> Consent<'a> {
        Consent {
            capability: "fs_write",
            resource: Some(path),
            triggering: None,
            root,
            explicit: false,
        }
    }

    fn consent(spec: &Consent<'_>) -> PermissionConsent {
        PermissionConsent::new(
            Some(spec.capability),
            spec.resource.map(str::to_owned),
            spec.root.map(std::path::PathBuf::from),
            None,
            None,
            Vec::new(),
        )
        .with_triggering_resource(spec.triggering.map(str::to_owned))
        .with_explicit_ask(spec.explicit)
    }

    fn folder(prefill: &str, target: &str, in_workspace: bool) -> ScopeChoice {
        ScopeChoice::Folder(FolderOffer {
            prefill: prefill.to_owned(),
            target: target.to_owned(),
            in_workspace,
        })
    }

    fn exact(resource: &str) -> ScopeChoice {
        ScopeChoice::Exact(resource.to_owned())
    }

    fn pattern(resource: &str) -> ScopeChoice {
        ScopeChoice::Pattern(resource.to_owned())
    }

    // C3: rows per capability, mirroring `isr` / `Ub` / `b2t` / `A2t`.
    #[rstest]
    #[case::shell_multi_word(shell("git status --short"), GrantScope::Session, true,
        vec![exact("git status --short"), pattern("git *"), ScopeChoice::WholeTool])]
    #[case::shell_single_word(shell("ls"), GrantScope::User, true,
        vec![exact("ls"), ScopeChoice::WholeTool])]
    #[case::shell_sudo(shell("sudo rm x"), GrantScope::Session, true,
        vec![exact("sudo rm x"), ScopeChoice::WholeTool])]
    #[case::shell_env(shell("env A=1 make"), GrantScope::Session, true,
        vec![exact("env A=1 make"), ScopeChoice::WholeTool])]
    #[case::shell_assignment(shell("FOO=1 make all"), GrantScope::Session, true,
        vec![exact("FOO=1 make all"), ScopeChoice::WholeTool])]
    #[case::shell_star(shell("*"), GrantScope::Session, true, vec![ScopeChoice::WholeTool])]
    #[case::exec_capability(Consent { capability: "exec", ..shell("npm test") }, GrantScope::Session, true,
        vec![exact("npm test"), pattern("npm *"), ScopeChoice::WholeTool])]
    #[case::execute_is_not_a_shell_name(Consent { capability: "execute", ..shell("npm test") }, GrantScope::Session, true,
        vec![exact("npm test"), ScopeChoice::WholeTool])]
    #[case::triggering_wins(Consent { triggering: Some("cargo build"), ..shell("cargo *") }, GrantScope::Session, true,
        vec![exact("cargo build"), pattern("cargo *"), ScopeChoice::WholeTool])]
    #[case::write_in_workspace_session(write("src/a/x.txt", Some("/ws")), GrantScope::Session, true,
        vec![exact("src/a/x.txt"), folder("src/a", "src/a/x.txt", true), ScopeChoice::WholeTool])]
    #[case::write_in_workspace_workspace(write("src/a/x.txt", Some("/ws")), GrantScope::Workspace, true,
        vec![exact("src/a/x.txt"), folder("src/a", "src/a/x.txt", true), ScopeChoice::WholeTool])]
    #[case::write_in_workspace_user(write("src/a/x.txt", Some("/ws")), GrantScope::User, true,
        vec![exact("src/a/x.txt"), ScopeChoice::WholeTool])]
    #[case::write_outside_user(write("/tmp/out/x.txt", None), GrantScope::User, true,
        vec![exact("/tmp/out/x.txt"), folder("/tmp/out", "/tmp/out/x.txt", false), ScopeChoice::WholeTool])]
    #[case::write_outside_workspace(write("/tmp/out/x.txt", None), GrantScope::Workspace, true,
        vec![exact("/tmp/out/x.txt"), ScopeChoice::WholeTool])]
    #[case::write_absolute_in_workspace(write("/ws/src/x.txt", Some("/ws")), GrantScope::Session, true,
        vec![exact("/ws/src/x.txt"), ScopeChoice::WholeTool])]
    #[case::write_at_top(write("x.txt", Some("/ws")), GrantScope::Session, true,
        vec![exact("x.txt"), ScopeChoice::WholeTool])]
    #[case::write_at_root(write("/x.txt", None), GrantScope::Session, true,
        vec![exact("/x.txt"), folder("/", "/x.txt", false), ScopeChoice::WholeTool])]
    #[case::write_backslashes(write("src\\a\\x.txt", Some("/ws")), GrantScope::Session, true,
        vec![exact("src\\a\\x.txt"), folder("src/a", "src/a/x.txt", true), ScopeChoice::WholeTool])]
    #[case::write_steps_up(write("../x/y.txt", Some("/ws")), GrantScope::Session, true,
        vec![exact("../x/y.txt"), ScopeChoice::WholeTool])]
    #[case::write_glob(write("src/*.txt", Some("/ws")), GrantScope::Session, true,
        vec![exact("src/*.txt"), ScopeChoice::WholeTool])]
    #[case::write_explicit(Consent { explicit: true, ..write("src/a/x.txt", Some("/ws")) }, GrantScope::Session, true,
        vec![exact("src/a/x.txt"), ScopeChoice::WholeTool])]
    #[case::write_other_session(write("src/a/x.txt", Some("/ws")), GrantScope::Session, false,
        vec![exact("src/a/x.txt"), ScopeChoice::WholeTool])]
    #[case::no_resource(Consent { resource: None, ..write("", None) }, GrantScope::Session, true,
        vec![ScopeChoice::WholeTool])]
    fn scope_rows_mirror_the_tui(
        #[case] spec: Consent<'_>,
        #[case] scope: GrantScope,
        #[case] owned: bool,
        #[case] expected: Vec<ScopeChoice>,
    ) {
        assert_eq!(scope_choices(&consent(&spec), scope, owned), expected);
    }

    fn in_workspace_offer() -> FolderOffer {
        FolderOffer {
            prefill: "src/a".to_owned(),
            target: "src/a/x.txt".to_owned(),
            in_workspace: true,
        }
    }

    fn outside_offer() -> FolderOffer {
        FolderOffer {
            prefill: "/tmp/out".to_owned(),
            target: "/tmp/out/x.txt".to_owned(),
            in_workspace: false,
        }
    }

    // C4: `vde` validation, with Kiro's exact messages.
    #[rstest]
    #[case::empty(in_workspace_offer(), "   ", "Type a folder path.")]
    #[case::zero_width(
        in_workspace_offer(),
        "src\u{200B}",
        "The folder contains unprintable characters."
    )]
    #[case::control(
        in_workspace_offer(),
        "sr\u{7}c",
        "The folder contains unprintable characters."
    )]
    #[case::inner_tab(
        in_workspace_offer(),
        "s\trc",
        "The folder contains unprintable characters."
    )]
    #[case::steps_up(
        in_workspace_offer(),
        "src/../src",
        "The folder cannot step up with \"..\"."
    )]
    #[case::tilde(outside_offer(), "~/out", "Spell out the folder instead of using ~.")]
    #[case::hash(in_workspace_offer(), "#src", "The folder cannot start with \"#\".")]
    #[case::glob(
        in_workspace_offer(),
        "src/*",
        "Type one folder, not a pattern; everything under it is included."
    )]
    #[case::absolute_in_workspace(
        in_workspace_offer(),
        "/ws/src",
        "Type a folder inside the workspace, like src/MyPackage."
    )]
    #[case::dot_in_workspace(
        in_workspace_offer(),
        "./",
        "Type a folder inside the workspace, like src/MyPackage."
    )]
    #[case::relative_outside(outside_offer(), ".", "Type the full path of the folder.")]
    #[case::sibling(in_workspace_offer(), "src/b", "The folder must contain src/a/x.txt.")]
    #[case::prefix_is_not_a_parent(
        in_workspace_offer(),
        "sr",
        "The folder must contain src/a/x.txt."
    )]
    #[case::outside_sibling(
        outside_offer(),
        "/tmp/other",
        "The folder must contain /tmp/out/x.txt."
    )]
    fn folder_validation_refuses_like_the_tui(
        #[case] offer: FolderOffer,
        #[case] typed: &str,
        #[case] message: &str,
    ) {
        let refused = offer.validate(typed).expect_err("folder must be refused");
        assert_eq!(refused.to_string(), message);
    }

    #[rstest]
    #[case::prefill(in_workspace_offer(), "src/a", "src/a/**")]
    #[case::parent(in_workspace_offer(), " src/ ", "src/**")]
    #[case::backslashes(in_workspace_offer(), "src\\a\\", "src/a/**")]
    #[case::dot_segments(in_workspace_offer(), "./src/./a", "src/a/**")]
    #[case::outside(outside_offer(), "/tmp", "/tmp/**")]
    #[case::root(outside_offer(), "/", "/**")]
    #[case::relative_outside_containing(FolderOffer { prefill: "a".into(), target: "a/b.txt".into(), in_workspace: false }, "a", "a/**")]
    fn folder_validation_accepts_like_the_tui(
        #[case] offer: FolderOffer,
        #[case] typed: &str,
        #[case] glob: &str,
    ) {
        let folder = offer.validate(typed).expect("folder must be accepted");
        assert_eq!(GrantResource::Folder(folder).wire(), glob);
    }

    #[test]
    fn drive_paths_normalise_like_the_tui() {
        assert_eq!(
            normalize_folder("C:\\Users\\me\\").as_deref(),
            Some("C:/Users/me")
        );
        assert_eq!(normalize_folder("C:").as_deref(), Some("C:/"));
        assert_eq!(parent_folder("C:/x.txt"), "C:/");
        assert!(folder_contains("C:/", "C:/x.txt"));
        assert!(!folder_contains("C:/Users", "C:/Users2/x"));
    }

    fn options(kinds: &[(&str, PermissionOptionKind)]) -> Vec<PermissionOption> {
        kinds
            .iter()
            .map(|(id, kind)| PermissionOption {
                id: PermissionOptionId::new(*id),
                label: (*id).to_owned(),
                kind: *kind,
                is_destructive: false,
            })
            .collect()
    }

    fn kas_options() -> Vec<PermissionOption> {
        options(&[
            ("accept", PermissionOptionKind::AllowOnce),
            ("always-accept", PermissionOptionKind::AllowAlways),
            ("reject", PermissionOptionKind::RejectOnce),
        ])
    }

    fn grant(scope: GrantScope, resource: GrantResource) -> AlwaysAllowGrant {
        AlwaysAllowGrant::new(scope, resource)
    }

    enum Sent {
        Grant,
        Once,
    }

    // C5: `_lt` / `C8n` — what an "Always allow" pick sends.
    #[rstest]
    #[case::main_session_fs_exact(write("src/x.txt", Some("/ws")), true,
        grant(GrantScope::User, GrantResource::Exact("src/x.txt".into())), kas_options(), Sent::Grant)]
    #[case::child_whole_tool(
        write("src/x.txt", Some("/ws")),
        false,
        grant(GrantScope::Session, GrantResource::WholeTool),
        kas_options(),
        Sent::Grant
    )]
    #[case::child_shell_exact(shell("npm test"), false,
        grant(GrantScope::User, GrantResource::Exact("npm test".into())), kas_options(), Sent::Grant)]
    #[case::child_shell_pattern(shell("npm test"), false,
        grant(GrantScope::Session, GrantResource::Pattern("npm *".into())), kas_options(), Sent::Grant)]
    #[case::child_fs_exact(write("src/x.txt", Some("/ws")), false,
        grant(GrantScope::Session, GrantResource::Exact("src/x.txt".into())), kas_options(), Sent::Once)]
    #[case::child_triggering_differs(Consent { triggering: Some("npm test"), ..shell("npm *") }, false,
        grant(GrantScope::Session, GrantResource::Exact("npm test".into())), kas_options(), Sent::Once)]
    #[case::child_workspace_without_root(Consent { root: None, ..shell("npm test") }, false,
        grant(GrantScope::Workspace, GrantResource::WholeTool), kas_options(), Sent::Once)]
    #[case::child_no_allow_once(write("src/x.txt", Some("/ws")), false,
        grant(GrantScope::Session, GrantResource::Exact("src/x.txt".into())),
        options(&[("always-accept", PermissionOptionKind::AllowAlways)]), Sent::Grant)]
    fn always_allow_resolves_like_the_tui(
        #[case] spec: Consent<'_>,
        #[case] owned: bool,
        #[case] picked: AlwaysAllowGrant,
        #[case] options: Vec<PermissionOption>,
        #[case] sent: Sent,
    ) {
        let response = resolve(
            &consent(&spec),
            owned,
            &options,
            PermissionOptionId::new("always-accept"),
            picked.clone(),
        );
        match (sent, response) {
            (Sent::Grant, PermissionResponse::AlwaysAllow { option_id, grant }) => {
                assert_eq!(option_id.as_str(), "always-accept");
                assert_eq!(grant, picked);
            }
            (
                Sent::Once,
                PermissionResponse::Selected {
                    option_id,
                    trust_option,
                },
            ) => {
                assert_eq!(option_id.as_str(), "accept");
                assert_eq!(trust_option, None);
            }
            (_, other) => panic!("unexpected response {other:?}"),
        }
    }

    #[rstest]
    #[case::no_main(None, "a", true)]
    #[case::same(Some("a"), "a", true)]
    #[case::empty_request(Some("a"), "", true)]
    #[case::other(Some("a"), "b", false)]
    fn main_session_ownership(
        #[case] main: Option<&str>,
        #[case] request: &str,
        #[case] owned: bool,
    ) {
        let main = main.map(SessionId::new);
        assert_eq!(
            is_main_session_request(&SessionId::new(request), main.as_ref()),
            owned
        );
    }

    #[test]
    fn scope_cycles_session_workspace_user() {
        assert_eq!(GrantScope::Session.next(), GrantScope::Workspace);
        assert_eq!(GrantScope::Workspace.next(), GrantScope::User);
        assert_eq!(GrantScope::User.next(), GrantScope::Session);
        assert_eq!(
            GrantScope::ALL.map(GrantScope::label),
            ["session", "workspace", "user"]
        );
    }

    // --- Queue release (`k8n` / `Clt` / `_lt` / `D4`) -----------------------

    /// One permission request in the queue, owned so tests can borrow it.
    struct Pending {
        session: SessionId,
        tool_call: ToolCallId,
        consent: Option<PermissionConsent>,
        options: Vec<PermissionOption>,
    }

    impl Pending {
        fn view(&self) -> QueuedRequest<'_> {
            QueuedRequest {
                session_id: &self.session,
                tool_call_id: &self.tool_call,
                tool_title: "tool",
                consent: self.consent.as_ref(),
                options: &self.options,
            }
        }
    }

    fn pending(session: &str, tool_call: &str, spec: Consent<'_>) -> Pending {
        Pending {
            session: SessionId::new(session),
            tool_call: ToolCallId::new(tool_call),
            consent: Some(consent(&spec)),
            options: kas_options(),
        }
    }

    fn no_allow_always(mut request: Pending) -> Pending {
        request
            .options
            .retain(|option| option.kind != PermissionOptionKind::AllowAlways);
        request
    }

    fn no_allow_once(mut request: Pending) -> Pending {
        request
            .options
            .retain(|option| option.kind != PermissionOptionKind::AllowOnce);
        request
    }

    fn sent(scope: GrantScope, resource: GrantResource) -> PermissionResponse {
        PermissionResponse::AlwaysAllow {
            option_id: PermissionOptionId::new("always-accept"),
            grant: grant(scope, resource),
        }
    }

    fn exact_sent(scope: GrantScope, resource: &str) -> PermissionResponse {
        sent(scope, GrantResource::Exact(resource.into()))
    }

    fn other_root(spec: Consent<'_>) -> Consent<'_> {
        Consent {
            root: Some("/other"),
            ..spec
        }
    }

    fn explicit(spec: Consent<'_>) -> Consent<'_> {
        Consent {
            explicit: true,
            ..spec
        }
    }

    // The coverage matrix: what an "Always allow" answer to `granted` settles
    // among waiting requests. Main session is "main"; "step-1"/"step-2" are
    // other sessions (workflow steps).
    #[rstest]
    // Main session: only a whole-tool grant releases, by capability key.
    #[case::main_whole_same_capability(
        pending("main", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("main", "t2", shell("cargo build")),
        true
    )]
    #[case::main_whole_exec_folds_into_shell(pending("main", "t1", shell("npm test")),
        sent(GrantScope::User, GrantResource::WholeTool),
        pending("main", "t2", Consent { capability: "exec", ..shell("ls -la") }), true)]
    #[case::main_whole_other_capability(
        pending("main", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("main", "t2", write("src/x.txt", Some("/ws"))),
        false
    )]
    #[case::main_whole_other_root(
        pending("main", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("main", "t2", other_root(shell("npm test"))),
        false
    )]
    #[case::main_whole_other_session(
        pending("main", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("step-1", "t2", shell("npm test")),
        false
    )]
    #[case::main_exact_releases_nothing(
        pending("main", "t1", shell("npm test")),
        exact_sent(GrantScope::User, "npm test"),
        pending("main", "t2", shell("npm test")),
        false
    )]
    #[case::main_pattern_releases_nothing(pending("main", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::Pattern("npm *".into())),
        pending("main", "t2", shell("npm run x")), false)]
    // Another session: the recorded child grant releases what it matches.
    #[case::child_exact_same_command(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::Session, "npm test"),
        pending("step-1", "t2", shell("npm test")),
        true
    )]
    #[case::child_exact_other_command(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::Session, "npm test"),
        pending("step-1", "t2", shell("npm run x")),
        false
    )]
    #[case::child_pattern_matching(pending("step-1", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::Pattern("npm *".into())),
        pending("step-1", "t2", shell("npm run build")), true)]
    #[case::child_pattern_other_command(pending("step-1", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::Pattern("npm *".into())),
        pending("step-1", "t2", shell("cargo build")), false)]
    #[case::child_whole_same_capability(
        pending("step-1", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("step-1", "t2", shell("cargo build")),
        true
    )]
    #[case::child_whole_other_capability(
        pending("step-1", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("step-1", "t2", write("src/x.txt", Some("/ws"))),
        false
    )]
    #[case::child_user_scope_other_operation(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::User, "npm test"),
        pending("step-1", "t2", shell("npm test")),
        false
    )]
    #[case::child_user_scope_same_operation(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::User, "npm test"),
        pending("step-1", "t1", shell("npm test")),
        true
    )]
    #[case::child_other_child_session(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::Session, "npm test"),
        pending("step-2", "t2", shell("npm test")),
        false
    )]
    #[case::child_main_session_request(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::Session, "npm test"),
        pending("main", "t2", shell("npm test")),
        false
    )]
    #[case::child_explicit_ask(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::Session, "npm test"),
        pending("step-1", "t2", explicit(shell("npm test"))),
        false
    )]
    #[case::child_no_allow_always_offered(
        pending("step-1", "t1", shell("npm test")),
        exact_sent(GrantScope::Session, "npm test"),
        no_allow_always(pending("step-1", "t2", shell("npm test"))),
        false
    )]
    #[case::child_workspace_other_root(
        pending("step-1", "t1", shell("npm test")),
        sent(GrantScope::Workspace, GrantResource::WholeTool),
        pending("step-1", "t1", other_root(shell("npm test"))),
        false
    )]
    #[case::child_session_other_root(
        pending("step-1", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        pending("step-1", "t2", other_root(shell("cargo build"))),
        false
    )]
    // Answers that grant nothing release nothing.
    #[case::allow_once_answer(pending("main", "t1", shell("npm test")),
        PermissionResponse::Selected { option_id: PermissionOptionId::new("accept"), trust_option: None },
        pending("main", "t2", shell("npm test")), false)]
    #[case::covered_without_allow_once_stays(
        pending("main", "t1", shell("npm test")),
        sent(GrantScope::Session, GrantResource::WholeTool),
        no_allow_once(pending("main", "t2", shell("cargo build"))),
        false
    )]
    fn queued_requests_released_like_the_tui(
        #[case] granted: Pending,
        #[case] answer: PermissionResponse,
        #[case] queued: Pending,
        #[case] released: bool,
    ) {
        let main = SessionId::new("main");
        let release = QueueRelease::for_answer(&granted.view(), &answer, Some(&main));
        let answered = release.covers(&queued.view(), Some(&main));
        assert_eq!(answered.is_some(), released, "release {release:?}");
        if let Some(option) = answered {
            assert_eq!(option.as_str(), "accept", "answered with allow_once");
        }
    }

    #[rstest]
    #[case::plain("npm test", "npm test", true)]
    #[case::trailing_star("npm *", "npm run build", true)]
    #[case::star_needs_prefix("npm *", "pnpm i", false)]
    #[case::middle("git * --force", "git push origin --force", true)]
    #[case::middle_missing("git * --force", "git push", false)]
    #[case::no_star_differs("npm test", "npm test x", false)]
    fn wildcard_matches_like_the_tui(
        #[case] pattern: &str,
        #[case] text: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(wildcard_matches(pattern, text), expected);
    }
}
