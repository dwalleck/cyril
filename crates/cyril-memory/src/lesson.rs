use std::fmt;
use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_LESSON_CHARS: usize = 2_000;
const CONTEXT_HEADER: &str = "<CYRIL_LESSONS trust=\"user_explicit_instruction\">\nThese are explicit project instructions taught by the user. Follow them unless the current user request supersedes them.\n";
const CONTEXT_FOOTER: &str = "</CYRIL_LESSONS>";
const REDACTED: &str = "[REDACTED]";

static ASSIGNMENT_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"(?i-u:\b(password|passwd|token|secret|api_key|apikey|access_key|private_key)([ \t\r\n]*[:=][ \t\r\n]*))([^ \t\r\n]+)",
    )
});
static GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r"(?-u:\b(?:ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b)")
});
static AWS_ACCESS_KEY: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?-u:\bAKIA[A-Z0-9]{16}\b)"));
static OPENAI_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?-u:\bsk-[A-Za-z0-9_-]{20,}\b)"));
static PEM_PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
    )
});

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonProvenance {
    UserExplicit,
    Document,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonTrust {
    Instruction,
    Reference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonStatus {
    Active,
    Invalidated,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextLesson {
    id: LessonId,
    sequence: u64,
    text: LessonText,
    provenance: LessonProvenance,
    trust: LessonTrust,
    status: LessonStatus,
}

impl ContextLesson {
    pub const fn new(
        id: LessonId,
        sequence: u64,
        text: LessonText,
        provenance: LessonProvenance,
        trust: LessonTrust,
        status: LessonStatus,
    ) -> Self {
        Self {
            id,
            sequence,
            text,
            provenance,
            trust,
            status,
        }
    }

    pub const fn id(&self) -> LessonId {
        self.id
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn text(&self) -> &LessonText {
        &self.text
    }

    fn is_explicit_instruction(&self) -> bool {
        self.provenance == LessonProvenance::UserExplicit
            && self.trust == LessonTrust::Instruction
            && self.status == LessonStatus::Active
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
}

pub fn render_context(candidates: &[ContextLesson], budget: usize) -> Option<ContextBlock> {
    let mut eligible: Vec<&ContextLesson> = candidates
        .iter()
        .filter(|lesson| lesson.is_explicit_instruction())
        .collect();
    eligible.sort_by_key(|lesson| std::cmp::Reverse(lesson.sequence));
    if eligible.is_empty() {
        return None;
    }

    let mut selected = Vec::new();
    for lesson in &eligible {
        selected.push(*lesson);
        let omitted = eligible.len() - selected.len();
        if render_selected(&selected, omitted).chars().count() > budget {
            selected.pop();
            break;
        }
    }
    let omitted_count = eligible.len() - selected.len();
    let text = render_selected(&selected, omitted_count);
    (text.chars().count() <= budget).then_some(ContextBlock {
        text,
        omitted_count,
    })
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

fn redact(input: &str) -> String {
    let redacted = PEM_PRIVATE_KEY.replace_all(input, REDACTED);
    let redacted = ASSIGNMENT_SECRET.replace_all(&redacted, "${1}${2}[REDACTED]");
    let redacted = GITHUB_TOKEN.replace_all(&redacted, REDACTED);
    let redacted = AWS_ACCESS_KEY.replace_all(&redacted, REDACTED);
    OPENAI_TOKEN.replace_all(&redacted, REDACTED).into_owned()
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    fn lesson(sequence: u64, text: &str) -> ContextLesson {
        ContextLesson::new(
            LessonId::from_bytes([sequence as u8; 16]),
            sequence,
            LessonText::new(text).expect("lesson text"),
            LessonProvenance::UserExplicit,
            LessonTrust::Instruction,
            LessonStatus::Active,
        )
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
    fn context_block_respects_trust_order_and_whole_lesson_budget() {
        let newest = lesson(3, "newest explicit instruction");
        let older = lesson(
            1,
            "older explicit instruction that is deliberately much longer than omitted marker",
        );
        let invalidated = ContextLesson::new(
            LessonId::from_bytes([4; 16]),
            4,
            LessonText::new("invalidated").expect("text"),
            LessonProvenance::UserExplicit,
            LessonTrust::Instruction,
            LessonStatus::Invalidated,
        );
        let derived = ContextLesson::new(
            LessonId::from_bytes([5; 16]),
            5,
            LessonText::new("derived reference").expect("text"),
            LessonProvenance::Document,
            LessonTrust::Reference,
            LessonStatus::Active,
        );
        let candidates = vec![older, derived, newest, invalidated];

        assert!(render_context(&candidates, 0).is_none());
        let full = render_context(&candidates, 4_000).expect("full block");
        assert!(full.text().starts_with("<CYRIL_LESSONS"));
        assert!(full.text().ends_with("</CYRIL_LESSONS>"));
        assert!(
            full.text().find("newest").expect("newest") < full.text().find("older").expect("older")
        );
        assert!(!full.text().contains("derived"));
        assert!(!full.text().contains("invalidated"));
        assert_eq!(full.omitted_count(), 0);
        assert!(full.text().chars().count() <= 4_000);

        let one_lesson_budget = CONTEXT_HEADER.chars().count()
            + "- newest explicit instruction\n".chars().count()
            + "[1 additional lesson(s) omitted]\n".chars().count()
            + CONTEXT_FOOTER.chars().count();
        let bounded = render_context(&candidates, one_lesson_budget).expect("bounded block");
        assert!(bounded.text().contains("newest explicit instruction"));
        assert!(!bounded.text().contains("older explicit instruction"));
        assert_eq!(bounded.omitted_count(), 1);
        assert!(bounded.text().chars().count() <= one_lesson_budget);
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        Sha256::digest(bytes).into()
    }
}
