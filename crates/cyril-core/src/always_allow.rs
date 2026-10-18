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

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
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
}
