use std::fmt;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::encoding::decode_fixed_hex;

pub const MAX_LESSON_CHARS: usize = 2_000;
/// Budget for one first-prompt context block. Typed as the wire carries it:
/// `context` requests send `max_chars` as `u16`, and this is the only cap.
pub const MAX_CONTEXT_CHARS: u16 = 4_000;
/// Maximum characters of lesson content carried by one `list` row. Rows at
/// this length are truncated previews, not full lesson text.
pub const LESSON_PREVIEW_CHARS: usize = 160;
const CONTEXT_HEADER: &str = "<CYRIL_LESSONS trust=\"user_explicit_instruction\">\nThese are explicit project instructions taught by the user. Follow them unless the current user request supersedes them.\n";
const CONTEXT_FOOTER: &str = "</CYRIL_LESSONS>";
const REDACTED: &str = "[REDACTED]";

/// `key[:=]value` assignments. Group 1 is the key, group 2 the separator,
/// group 3 the candidate value; the value is only redacted when it looks like
/// a credential (see [`looks_like_credential`]) so that prose such as
/// `password: use the vault` survives intact.
static ASSIGNMENT_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"(?i-u:\b(password|passwd|token|secret|api_key|apikey|access_key|private_key)([ \t\r\n]*[:=][ \t\r\n]*))([^ \t\r\n]+)",
    )
});
// Token patterns consume the whole `[A-Za-z0-9_-]` run after the recognized
// prefix instead of ending on `\b`: a credential glued to a suffix such as
// `ghp_…_old` is still a credential and must not leak because `_` is a word
// character.
static GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r"(?-u:\b(?:ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})[A-Za-z0-9_-]*)")
});
static AWS_ACCESS_KEY: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?-u:\bAKIA[A-Z0-9]{16}[A-Za-z0-9_-]*)"));
static OPENAI_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?-u:\bsk-[A-Za-z0-9_-]{20,})"));
// A PEM block missing its END line (a truncated paste) is redacted to the end
// of the text rather than stored verbatim.
static PEM_PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----(?:.*?-----END [A-Z0-9 ]*PRIVATE KEY-----|.*)",
    )
});
/// Characters that make an assignment value credential-shaped even without a
/// digit. Sentence punctuation (`.`, `,`, `'`, `)`) is deliberately absent.
const CREDENTIAL_PUNCTUATION: &[char] = &[
    '_', '-', '/', '+', '=', '@', '#', '$', '%', '^', '&', '*', '~',
];
const CREDENTIAL_LENGTH_ALONE: usize = 16;

fn compile_regex(pattern: &str) -> Regex {
    Regex::new(pattern)
        .unwrap_or_else(|error| panic!("hard-coded secret regex is invalid: {error}"))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LessonId([u8; 16]);

impl LessonId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl std::str::FromStr for LessonId {
    type Err = LessonIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        decode_fixed_hex::<16>(value)
            .map(Self)
            .ok_or(LessonIdParseError)
    }
}

impl fmt::Debug for LessonId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "LessonId({self})")
    }
}

impl fmt::Display for LessonId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("lesson identity must be exactly 32 hexadecimal characters")]
pub struct LessonIdParseError;

/// Where a lesson came from. Only explicit user teaching exists today; the
/// column and vocabulary stay so a future source is an additive change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonProvenance {
    UserExplicit,
}

impl LessonProvenance {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UserExplicit => "user_explicit",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Option<Self> {
        match value {
            "user_explicit" => Some(Self::UserExplicit),
            _ => None,
        }
    }
}

/// How a lesson is presented to the model. Only instructions exist today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonTrust {
    Instruction,
}

impl LessonTrust {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Instruction => "instruction",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Option<Self> {
        match value {
            "instruction" => Some(Self::Instruction),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonStatus {
    Active,
    Invalidated,
}

impl LessonStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Invalidated => "invalidated",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "invalidated" => Some(Self::Invalidated),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LessonText {
    redacted: String,
    content_hash: [u8; 32],
}

impl LessonText {
    pub fn new(input: &str) -> Result<Self, LessonError> {
        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        let normalized = normalized.trim();
        if normalized.is_empty() {
            return Err(LessonError::Empty);
        }
        if normalized
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        {
            return Err(LessonError::ControlCharacter);
        }
        let actual = normalized.chars().count();
        if actual > MAX_LESSON_CHARS {
            return Err(LessonError::TooLong {
                actual,
                max: MAX_LESSON_CHARS,
            });
        }
        let redacted = redact(normalized);
        // Redaction can grow the text (`token=x` becomes `token=[REDACTED]`),
        // and the redacted form is what gets stored and re-validated by the
        // runtime, so the cap is enforced on it too — otherwise the client
        // accepts what the runtime then rejects as an opaque wire error.
        let redacted_chars = redacted.chars().count();
        if redacted_chars > MAX_LESSON_CHARS {
            return Err(LessonError::TooLongRedacted {
                actual: redacted_chars,
                max: MAX_LESSON_CHARS,
            });
        }
        let content_hash = Sha256::digest(redacted.as_bytes()).into();
        Ok(Self {
            redacted,
            content_hash,
        })
    }

    pub fn redacted(&self) -> &str {
        &self.redacted
    }

    pub const fn content_hash(&self) -> [u8; 32] {
        self.content_hash
    }

    /// Rebuild a lesson from a stored row.
    ///
    /// The stored invariant is integrity only: `content_hash` must be the
    /// SHA-256 of the stored bytes. The current redactor is then re-applied
    /// to the content when serving, so a row written by an older, looser
    /// redactor is served redacted (self-healing) instead of being rejected
    /// — "current regexes leave the text unchanged" is a property of the
    /// binary, not of the data, and must not be treated as a stored invariant.
    pub(crate) fn from_stored(
        content: String,
        content_hash: [u8; 32],
    ) -> Result<Self, LessonError> {
        let actual: [u8; 32] = Sha256::digest(content.as_bytes()).into();
        if actual != content_hash {
            return Err(LessonError::CorruptStored);
        }
        let healed = Self::new(&content).map_err(|error| {
            tracing::warn!(error = %error, "stored lesson failed shape validation");
            LessonError::CorruptStored
        })?;
        Ok(Self {
            redacted: healed.redacted,
            content_hash,
        })
    }
}

impl fmt::Debug for LessonText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LessonText")
            .field("redacted", &"[REDACTED]")
            .field("content_hash", &hex::encode(self.content_hash))
            .finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LessonError {
    #[error("lesson text cannot be empty")]
    Empty,
    #[error("lesson text contains a forbidden control character")]
    ControlCharacter,
    #[error("lesson text has {actual} characters; maximum is {max}")]
    TooLong { actual: usize, max: usize },
    #[error("lesson text expands to {actual} characters after secret redaction; maximum is {max}")]
    TooLongRedacted { actual: usize, max: usize },
    #[error("stored lesson text or hash is corrupt")]
    CorruptStored,
}

/// One active lesson considered for a first-prompt context block. Only what
/// rendering needs: newest-first ordering and the redacted text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextLesson {
    sequence: u64,
    text: LessonText,
}

impl ContextLesson {
    pub(crate) const fn new(sequence: u64, text: LessonText) -> Self {
        Self { sequence, text }
    }

    pub(crate) fn rendered_line_chars(&self) -> usize {
        3 + self.text.redacted().chars().count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextBlock {
    text: String,
    omitted_count: usize,
}

impl ContextBlock {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }

    pub(crate) fn from_wire(text: String, omitted_count: usize) -> Result<Self, LessonError> {
        if text.chars().count() > usize::from(MAX_CONTEXT_CHARS)
            || !text.starts_with(CONTEXT_HEADER)
            || !text.ends_with(CONTEXT_FOOTER)
        {
            return Err(LessonError::CorruptStored);
        }
        Ok(Self {
            text,
            omitted_count,
        })
    }
}

/// Render the newest lessons that fit whole inside `budget` characters.
///
/// `eligible_count` is the total number of active lessons the caller knows
/// about (it may exceed `candidates.len()` when the caller stopped reading
/// early); the difference is reported as omitted.
pub(crate) fn render_context(
    candidates: &[ContextLesson],
    eligible_count: usize,
    budget: usize,
) -> Option<ContextBlock> {
    if eligible_count == 0 {
        return None;
    }
    let mut eligible: Vec<&ContextLesson> = candidates.iter().collect();
    eligible.sort_by_key(|lesson| std::cmp::Reverse(lesson.sequence));

    let fixed_chars = CONTEXT_HEADER.len() + CONTEXT_FOOTER.len();
    let mut selected = Vec::with_capacity(eligible.len());
    let mut selected_chars = 0;
    for lesson in eligible {
        let line_chars = lesson.rendered_line_chars();
        let omitted = eligible_count.saturating_sub(selected.len() + 1);
        let omitted_chars = if omitted == 0 {
            0
        } else {
            "[ additional lesson(s) omitted]\n".len() + decimal_digits(omitted)
        };
        if fixed_chars + selected_chars + line_chars + omitted_chars > budget {
            break;
        }
        selected.push(lesson);
        selected_chars += line_chars;
    }
    let omitted_count = eligible_count.saturating_sub(selected.len());
    let text = render_selected(&selected, omitted_count);
    (text.chars().count() <= budget).then_some(ContextBlock {
        text,
        omitted_count,
    })
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn render_selected(selected: &[&ContextLesson], omitted_count: usize) -> String {
    let mut text = String::from(CONTEXT_HEADER);
    for lesson in selected {
        text.push_str("- ");
        text.push_str(lesson.text.redacted());
        text.push('\n');
    }
    if omitted_count > 0 {
        use std::fmt::Write as _;
        writeln!(text, "[{omitted_count} additional lesson(s) omitted]")
            .unwrap_or_else(|error| panic!("writing to String failed: {error}"));
    }
    text.push_str(CONTEXT_FOOTER);
    text
}

/// Whether an assignment value is credential-shaped rather than prose: it
/// carries a digit, credential punctuation, or is long enough that no
/// ordinary word would appear there.
fn looks_like_credential(value: &str) -> bool {
    value.chars().count() >= CREDENTIAL_LENGTH_ALONE
        || value.chars().any(|character| character.is_ascii_digit())
        || value.contains(CREDENTIAL_PUNCTUATION)
}

fn redact(input: &str) -> String {
    let redacted = PEM_PRIVATE_KEY.replace_all(input, REDACTED);
    let redacted = ASSIGNMENT_SECRET.replace_all(&redacted, |captures: &Captures<'_>| {
        let group = |index: usize| captures.get(index).map_or("", |found| found.as_str());
        if looks_like_credential(group(3)) {
            format!("{}{}{REDACTED}", group(1), group(2))
        } else {
            group(0).to_owned()
        }
    });
    let redacted = GITHUB_TOKEN.replace_all(&redacted, REDACTED);
    let redacted = AWS_ACCESS_KEY.replace_all(&redacted, REDACTED);
    OPENAI_TOKEN.replace_all(&redacted, REDACTED).into_owned()
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    fn lesson(sequence: u64, text: &str) -> ContextLesson {
        ContextLesson::new(sequence, LessonText::new(text).expect("lesson text"))
    }

    #[test]
    fn lesson_text_normalizes_and_validates_boundaries() {
        let text = LessonText::new("  line one\r\nline two\r  ").expect("valid");
        assert_eq!(text.redacted(), "line one\nline two");
        assert!(matches!(LessonText::new(" \n\t "), Err(LessonError::Empty)));
        assert!(matches!(
            LessonText::new("bad\0text"),
            Err(LessonError::ControlCharacter)
        ));
        assert!(LessonText::new(&"x".repeat(2_000)).is_ok());
        assert!(matches!(
            LessonText::new(&"x".repeat(2_001)),
            Err(LessonError::TooLong { .. })
        ));
    }

    #[test]
    fn lesson_cap_applies_to_the_redacted_text() {
        // 222 × "token=s3 " = 1998 chars before redaction and 3774 after
        // (`token=[REDACTED] `): the client-side constructor must reject
        // exactly what the runtime would.
        let input = "token=s3 ".repeat(222);
        assert_eq!(input.trim().chars().count(), 1_997);
        let expected = "token=[REDACTED] ".repeat(222).trim().chars().count();
        assert!(expected > MAX_LESSON_CHARS);
        let error = LessonText::new(&input).expect_err("over the cap after redaction");
        assert_eq!(
            error,
            LessonError::TooLongRedacted {
                actual: expected,
                max: MAX_LESSON_CHARS
            }
        );
        assert_eq!(
            error.to_string(),
            format!(
                "lesson text expands to {expected} characters after secret redaction; maximum is 2000"
            )
        );
        // A text that only shrinks under redaction is still bounded by the
        // pre-redaction cap.
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
            "A".repeat(2_100)
        );
        assert!(matches!(
            LessonText::new(&pem),
            Err(LessonError::TooLong { .. })
        ));
    }

    #[test]
    fn supported_secrets_are_redacted_before_every_boundary() {
        let cases = [
            ("password=hunter2", "password=[REDACTED]"),
            (
                "token: ghp_abcdefghijklmnopqrstuvwxyz123456",
                "token: [REDACTED]",
            ),
            (
                "api_key = sk-abcdefghijklmnopqrstuvwxyz123456",
                "api_key = [REDACTED]",
            ),
            ("AWS key AKIAABCDEFGHIJKLMNOP", "AWS key [REDACTED]"),
            (
                "-----BEGIN PRIVATE KEY-----\nabc123\n-----END PRIVATE KEY-----",
                "[REDACTED]",
            ),
            ("secret=my-secret", "secret=[REDACTED]"),
            (
                "passphrase password: correcthorsebatterystaple",
                "passphrase password: [REDACTED]",
            ),
        ];
        for (input, expected) in cases {
            let text = LessonText::new(input).expect("valid secret-shaped lesson");
            assert_eq!(text.redacted(), expected, "input: {input}");
            assert!(!text.redacted().contains("hunter2"));
            assert!(!format!("{text:?}").contains(input));
            assert_ne!(text.content_hash(), sha256(input.as_bytes()));
        }
        let lookalike =
            LessonText::new("use sk-short and ghp_tiny as examples").expect("lookalike");
        assert_eq!(
            lookalike.redacted(),
            "use sk-short and ghp_tiny as examples"
        );
    }

    #[test]
    fn prose_assignments_are_not_rewritten() {
        let cases = [
            "Never commit the password: use the vault",
            "never log the token: use structured fields",
            "the secret = don't ship on Fridays.",
            "api_key: rotate quarterly",
        ];
        for input in cases {
            let text = LessonText::new(input).expect("prose lesson");
            assert_eq!(text.redacted(), input, "input: {input}");
        }
    }

    #[test]
    fn glued_and_truncated_credentials_are_still_redacted() {
        let cases = [
            (
                "the old key was ghp_abcdefghijklmnopqrstuvwxyz123456_old, rotate it",
                "the old key was [REDACTED], rotate it",
            ),
            (
                "github_pat_abcdefghijklmnopqrstuvwxyz-suffix ok",
                "[REDACTED] ok",
            ),
            ("AKIAABCDEFGHIJKLMNOP_x rotated", "[REDACTED] rotated"),
            ("sk-abcdefghijklmnopqrstuvwxyz123456_legacy!", "[REDACTED]!"),
            (
                "key:\n-----BEGIN RSA PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC",
                "key:\n[REDACTED]",
            ),
        ];
        for (input, expected) in cases {
            let text = LessonText::new(input).expect("valid lesson");
            assert_eq!(text.redacted(), expected, "input: {input}");
        }
    }

    #[test]
    fn redaction_is_idempotent() {
        for input in [
            "password=hunter2",
            "token: ghp_abcdefghijklmnopqrstuvwxyz123456",
            "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
            "the old key was ghp_abcdefghijklmnopqrstuvwxyz123456_old",
        ] {
            let once = LessonText::new(input)
                .expect("first pass")
                .redacted()
                .to_owned();
            let twice = LessonText::new(&once).expect("second pass");
            assert_eq!(twice.redacted(), once, "input: {input}");
        }
    }

    #[test]
    fn stored_rows_verify_integrity_and_self_heal_redaction() {
        // A row an older, looser redactor would have written raw.
        let raw = "use ghp_abcdefghijklmnopqrstuvwxyz1234 in CI".to_owned();
        let hash = sha256(raw.as_bytes());
        let healed = LessonText::from_stored(raw, hash).expect("integrity holds");
        assert_eq!(healed.redacted(), "use [REDACTED] in CI");
        assert_eq!(
            healed.content_hash(),
            hash,
            "stored hash stays the row identity"
        );

        let clean = LessonText::new("prefer boring Rust").expect("clean");
        let round_trip = LessonText::from_stored(clean.redacted().to_owned(), clean.content_hash())
            .expect("clean row");
        assert_eq!(round_trip, clean);

        assert_eq!(
            LessonText::from_stored("tampered".to_owned(), hash),
            Err(LessonError::CorruptStored)
        );
        assert_eq!(
            LessonText::from_stored(String::new(), sha256(b"")),
            Err(LessonError::CorruptStored)
        );
    }

    #[test]
    fn context_block_orders_newest_first_within_whole_lesson_budget() {
        let newest = lesson(3, "newest explicit instruction");
        let older = lesson(
            1,
            "older explicit instruction that is deliberately much longer than omitted marker",
        );
        let candidates = vec![older, newest];

        assert!(render_context(&candidates, 0, 4_000).is_none());
        assert!(render_context(&candidates, candidates.len(), 0).is_none());
        let full = render_context(&candidates, candidates.len(), 4_000).expect("full block");
        assert!(full.text().starts_with("<CYRIL_LESSONS"));
        assert!(full.text().ends_with("</CYRIL_LESSONS>"));
        assert!(
            full.text().find("newest").expect("newest") < full.text().find("older").expect("older")
        );
        assert_eq!(full.omitted_count(), 0);
        assert!(full.text().chars().count() <= 4_000);

        let one_lesson_budget = CONTEXT_HEADER.chars().count()
            + "- newest explicit instruction\n".chars().count()
            + "[1 additional lesson(s) omitted]\n".chars().count()
            + CONTEXT_FOOTER.chars().count();
        let bounded = render_context(&candidates, candidates.len(), one_lesson_budget)
            .expect("bounded block");
        assert!(bounded.text().contains("newest explicit instruction"));
        assert!(!bounded.text().contains("older explicit instruction"));
        assert_eq!(bounded.omitted_count(), 1);
        assert!(bounded.text().chars().count() <= one_lesson_budget);

        // The caller may know about more eligible lessons than it read.
        let partial = render_context(&candidates, 5, 4_000).expect("partial block");
        assert_eq!(partial.omitted_count(), 3);
        assert!(partial.text().contains("[3 additional lesson(s) omitted]"));
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }
}
