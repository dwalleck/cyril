use std::sync::LazyLock;

use regex::{Captures, Regex};

const REDACTED: &str = "[REDACTED]";

/// `key[:=]value` assignments in prose, shell/env (`DB_PASSWORD=…`), and
/// JSON/YAML (`"password": "…"`) form.
///
/// Group 1 is the key including any `[a-z0-9_]` prefix (`DB_PASSWORD`,
/// `access_token`), group 2 the separator including an optional closing
/// quote on the key and optional whitespace, group 3 the optional opening
/// quote on the value, group 4 the candidate value. A `"`-quoted value is a
/// literal by construction and is always redacted; an unquoted value is
/// redacted only when it looks like a credential, so prose such as
/// `password: use the vault` survives intact.
static ASSIGNMENT_SECRET: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r#"(?i-u:\b([a-z0-9_]*(?:password|passwd|token|secret|api_key|apikey|access_key|private_key))("?[ \t\r\n]*[:=][ \t\r\n]*("?)))([^ \t\r\n"]+)"#,
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

/// Whether an assignment value is credential-shaped rather than prose: it
/// carries a digit, credential punctuation, or is long enough that no
/// ordinary word would appear there.
fn looks_like_credential(value: &str) -> bool {
    value.chars().count() >= CREDENTIAL_LENGTH_ALONE
        || value.chars().any(|character| character.is_ascii_digit())
        || value.contains(CREDENTIAL_PUNCTUATION)
}

/// Redact credentials from `input`. Idempotent: redacting already redacted
/// text yields the same text, which lets readers re-apply it freely.
pub(crate) fn redact(input: &str) -> String {
    let redacted = PEM_PRIVATE_KEY.replace_all(input, REDACTED);
    let redacted = ASSIGNMENT_SECRET.replace_all(&redacted, |captures: &Captures<'_>| {
        let group = |index: usize| captures.get(index).map_or("", |found| found.as_str());
        let quoted = !group(3).is_empty();
        if quoted || looks_like_credential(group(4)) {
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
        for (secret, leaked) in [
            ("password=hunter2", "hunter2"),
            ("token: ghp_abcdefghijklmnopqrstuvwxyz123456", "ghp_"),
            ("api_key = sk-abcdefghijklmnopqrstuvwxyz123456", "sk-"),
            ("AWS key AKIAABCDEFGHIJKLMNOP", "AKIA"),
            (
                "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
                "abc",
            ),
        ] {
            let redacted = redact(secret);
            assert!(!redacted.contains(leaked), "{secret} -> {redacted}");
            assert_eq!(redact(&redacted), redacted, "{secret}");
        }
        for prose in [
            "Never commit the password: use the vault",
            "never log the token: use structured fields",
            "the secret = don't ship on Fridays.",
            "the tokenizer = whitespace",
        ] {
            assert_eq!(redact(prose), prose);
        }
    }

    #[test]
    fn json_keyed_and_env_style_secrets_are_redacted() {
        let cases = [
            (
                r#"{"password":"hunter2","token":"abcdefghijklmnop1234"}"#,
                r#"{"password":"[REDACTED]","token":"[REDACTED]"}"#,
            ),
            // `.env` contents as a tool's raw input: real newlines separate
            // the assignments; a JSON-escaped `\n` is part of the value run,
            // so the whole run to the closing quote is redacted (over-,
            // never under-redaction).
            (
                "{\"path\":\".env\",\"content\":\"DB_PASSWORD=s3cret!\nAPI_TOKEN=abcdef0123456789xyz\"}",
                "{\"path\":\".env\",\"content\":\"DB_PASSWORD=[REDACTED]\nAPI_TOKEN=[REDACTED]\"}",
            ),
            (
                r#"{"path":".env","content":"DB_PASSWORD=s3cret!\nAPI_TOKEN=abcdef0123456789xyz"}"#,
                r#"{"path":".env","content":"DB_PASSWORD=[REDACTED]"}"#,
            ),
            (
                "DB_PASSWORD=s3cret!\nAPI_TOKEN=abcdef0123456789xyz\nAWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG",
                "DB_PASSWORD=[REDACTED]\nAPI_TOKEN=[REDACTED]\nAWS_SECRET_ACCESS_KEY=[REDACTED]",
            ),
            // A quoted value is a literal even when it does not look like a
            // credential; an unquoted prose value still survives.
            (r#""password": "hunter""#, r#""password": "[REDACTED]""#),
            ("password: hunter", "password: hunter"),
            (
                "mytoken=abc123 and access_token: 'x-9'",
                "mytoken=[REDACTED] and access_token: [REDACTED]",
            ),
        ];
        for (input, expected) in cases {
            let redacted = redact(input);
            assert_eq!(redacted, expected, "{input}");
            assert_eq!(redact(&redacted), redacted, "{input} must be idempotent");
        }
    }
}
