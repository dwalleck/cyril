use std::sync::LazyLock;

use regex::{Captures, Regex};

const REDACTED: &str = "[REDACTED]";

/// `key[:=]value` assignments. The value is redacted only when it looks like
/// a credential so prose such as `password: use the vault` survives intact.
static ASSIGNMENT_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"(?i-u:\b(password|passwd|token|secret|api_key|apikey|access_key|private_key)([ \t\r\n]*[:=][ \t\r\n]*))([^ \t\r\n]+)",
    )
});
static GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(r"(?-u:\b(?:ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})[A-Za-z0-9_-]*)")
});
static AWS_ACCESS_KEY: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?-u:\bAKIA[A-Z0-9]{16}[A-Za-z0-9_-]*)"));
static OPENAI_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"(?-u:\bsk-[A-Za-z0-9_-]{20,})"));
// A PEM block missing its END line is redacted to the end of the text.
static PEM_PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----(?:.*?-----END [A-Z0-9 ]*PRIVATE KEY-----|.*)",
    )
});
const CREDENTIAL_PUNCTUATION: &[char] = &[
    '_', '-', '/', '+', '=', '@', '#', '$', '%', '^', '&', '*', '~',
];
const CREDENTIAL_LENGTH_ALONE: usize = 16;

fn compile_regex(pattern: &str) -> Regex {
    Regex::new(pattern)
        .unwrap_or_else(|error| panic!("hard-coded secret regex is invalid: {error}"))
}

fn looks_like_credential(value: &str) -> bool {
    value.chars().count() >= CREDENTIAL_LENGTH_ALONE
        || value.chars().any(|character| character.is_ascii_digit())
        || value.contains(CREDENTIAL_PUNCTUATION)
}

pub(crate) fn redact(input: &str) -> String {
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
mod tests {
    use super::redact;

    #[test]
    fn shared_redactor_is_idempotent_and_preserves_prose() {
        for secret in [
            "password=hunter2",
            "token: ghp_abcdefghijklmnopqrstuvwxyz123456",
            "api_key = sk-abcdefghijklmnopqrstuvwxyz123456",
            "AWS key AKIAABCDEFGHIJKLMNOP",
            "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
        ] {
            let redacted = redact(secret);
            assert!(!redacted.contains("hunter2"));
            assert_eq!(redact(&redacted), redacted);
        }
        for prose in [
            "Never commit the password: use the vault",
            "never log the token: use structured fields",
            "the secret = don't ship on Fridays.",
        ] {
            assert_eq!(redact(prose), prose);
        }
    }
}
