use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::encoding::decode_fixed_hex;
use crate::redaction::redact;

pub const MAX_LESSON_CHARS: usize = 2_000;
/// Independent budget for explicit lessons in a prepared first-prompt block.
pub const MAX_LESSON_CONTEXT_CHARS: usize = 4_000;
/// Maximum characters of lesson content carried by one `list` row.
pub const LESSON_PREVIEW_CHARS: usize = 160;
const LESSON_HEADER: &str = "<CYRIL_LESSONS trust=\"user_explicit_instruction\">\nThese are explicit project instructions taught by the user. Follow them unless the current user request supersedes them.\n";
const LESSON_FOOTER: &str = "</CYRIL_LESSONS>";

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

/// One active lesson considered for first-prompt rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LessonCandidate {
    sequence: u64,
    text: LessonText,
}

impl LessonCandidate {
    pub(crate) const fn new(sequence: u64, text: LessonText) -> Self {
        Self { sequence, text }
    }

    pub(crate) fn rendered_line_chars(&self) -> usize {
        3 + self.text.redacted().chars().count()
    }
}

/// Render newest explicit lessons that fit whole inside `budget` characters.
pub(crate) fn render_lessons(
    candidates: &[LessonCandidate],
    eligible_count: usize,
    budget: usize,
) -> Option<String> {
    if eligible_count == 0 {
        return None;
    }
    let mut eligible: Vec<&LessonCandidate> = candidates.iter().collect();
    eligible.sort_by_key(|lesson| std::cmp::Reverse(lesson.sequence));
    let fixed_chars = LESSON_HEADER.chars().count() + LESSON_FOOTER.chars().count();
    let mut selected = Vec::with_capacity(eligible.len());
    let mut selected_chars = 0;
    for lesson in eligible {
        let line_chars = lesson.rendered_line_chars();
        let omitted = eligible_count.saturating_sub(selected.len() + 1);
        let omitted_chars = if omitted == 0 {
            0
        } else {
            "[ additional lesson(s) omitted]\n".chars().count() + decimal_digits(omitted)
        };
        if fixed_chars + selected_chars + line_chars + omitted_chars > budget {
            break;
        }
        selected.push(lesson);
        selected_chars += line_chars;
    }
    let omitted_count = eligible_count.saturating_sub(selected.len());
    let mut text = String::from(LESSON_HEADER);
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
    text.push_str(LESSON_FOOTER);
    (text.chars().count() <= budget).then_some(text)
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    fn lesson(sequence: u64, text: &str) -> LessonCandidate {
        LessonCandidate::new(sequence, LessonText::new(text).expect("lesson text"))
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
    fn lesson_render_orders_newest_first_within_budget() {
        let newest = lesson(3, "newest explicit instruction");
        let older = lesson(
            1,
            "older explicit instruction that is deliberately much longer than omitted marker",
        );
        let candidates = vec![older, newest];

        assert!(render_lessons(&candidates, 0, MAX_LESSON_CONTEXT_CHARS).is_none());
        assert!(render_lessons(&candidates, candidates.len(), 0).is_none());
        let full = render_lessons(&candidates, candidates.len(), MAX_LESSON_CONTEXT_CHARS)
            .expect("full block");
        assert!(full.starts_with("<CYRIL_LESSONS"));
        assert!(full.ends_with("</CYRIL_LESSONS>"));
        assert!(full.find("newest").expect("newest") < full.find("older").expect("older"));
        assert!(!full.contains("additional lesson(s) omitted"));
        assert!(full.chars().count() <= MAX_LESSON_CONTEXT_CHARS);

        let one_lesson_budget = LESSON_HEADER.chars().count()
            + "- newest explicit instruction\n".chars().count()
            + "[1 additional lesson(s) omitted]\n".chars().count()
            + LESSON_FOOTER.chars().count();
        let bounded = render_lessons(&candidates, candidates.len(), one_lesson_budget)
            .expect("bounded block");
        assert!(bounded.contains("newest explicit instruction"));
        assert!(!bounded.contains("older explicit instruction"));
        assert!(bounded.contains("[1 additional lesson(s) omitted]"));
        assert!(bounded.chars().count() <= one_lesson_budget);

        let partial = render_lessons(&candidates, 5, MAX_LESSON_CONTEXT_CHARS).expect("partial");
        assert!(partial.contains("[3 additional lesson(s) omitted]"));
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }
}
