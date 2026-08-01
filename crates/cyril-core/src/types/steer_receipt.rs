//! The `[STEERING <id>: <note>]` receipt the model emits for an injected
//! queue-steer (cyril-3qwa).
//!
//! When `_session/steer` drains at a tool boundary, Kiro's backend does not just
//! hand the model the steer text — it wraps it in a `[LIVE STEERING …]` turn that
//! ends with a mandatory reply contract:
//!
//! ```text
//! IMPORTANT: After completing your work, include a brief note about how you
//! handled this steering message. Use this exact format:
//!
//! [STEERING steer-<uuid>: <describe what you did or why it wasn't applicable>]
//! ```
//!
//! The model complies, emitting the marker as a trailer in its response text.
//! That text reaches cyril as ordinary `AgentMessageChunk`s, so without this
//! module the raw marker renders as prose in the transcript.
//!
//! Evidence: AWS `GenerateAssistantResponse` prompt logs (2026-07-23) — the
//! captured `assistantResponse` ends with a well-formed receipt whose note runs
//! to roughly 300 characters and carries an em dash, backticks and parentheses.
//! The receipt id shares the wire queue-id space (`messageId` on the new-family
//! v2 echoes and KAS is the same `steer-<uuid>` shape), which is what makes
//! id-correlation back to a `SteerEcho` possible.
//!
//! The test fixtures reproduce that shape — every character class, comparable
//! length — with neutral content: the capture came from a private workspace and
//! this repository is public.
//!
//! # Conservatism
//!
//! The two failure modes are not symmetric. Failing to strip a marker leaves the
//! pre-cyril-3qwa behaviour (visible noise, nothing lost). Stripping too much
//! silently eats agent output. Every ambiguous case therefore resolves toward
//! leaving the text alone:
//!
//! - The id must literally start with [`ID_PREFIX`]. A `[STEERING` that opens a
//!   sentence rather than a receipt is left in place.
//! - The body stops at the **first** `]`. A note containing `]` yields a
//!   truncated receipt plus visible remainder — never a greedy match that could
//!   swallow real prose that happens to follow.
//! - A marker that never terminates is never removed; it commits as ordinary
//!   text when the stream flushes.

/// Literal that opens a receipt.
const OPEN: &str = "[STEERING";

/// Every receipt id observed on the wire and in the prompt logs carries this
/// prefix. Requiring it is what keeps prose like `[STEERING GROUP]` from being
/// mistaken for a receipt.
const ID_PREFIX: &str = "steer-";

/// One receipt the model emitted for one injected queue-steer.
///
/// Fields are private with accessors per the workspace error/type conventions —
/// the internals are free to change without breaking callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteerReceipt {
    id: String,
    note: String,
}

impl SteerReceipt {
    /// The queue id this receipt answers — correlates with `SteeringQueued`'s
    /// `message_id`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The model's own account of how it handled the steer. May be empty when
    /// the model emitted `[STEERING <id>: ]`; the receipt still proves pickup.
    pub fn note(&self) -> &str {
        &self.note
    }
}

/// Is `id` a well-formed receipt id?
fn is_valid_id(id: &str) -> bool {
    id.len() > ID_PREFIX.len()
        && id.starts_with(ID_PREFIX)
        && !id.contains(char::is_whitespace)
        && !id.contains(']')
}

/// Parse one complete `[STEERING <id>: <note>]` slice.
///
/// Returns `None` when `s` opens with [`OPEN`] but is not a receipt (no
/// separating whitespace, missing colon, or an id that fails [`is_valid_id`]) —
/// the caller leaves such text untouched.
fn parse_marker(s: &str) -> Option<SteerReceipt> {
    let inner = s.strip_prefix(OPEN)?.strip_suffix(']')?;
    // `[STEERINGsteer-x: …]` is not a receipt — the format has a space.
    if !inner.starts_with(char::is_whitespace) {
        return None;
    }
    // The id cannot contain `:`, so the first colon is always the separator;
    // colons inside the note are preserved.
    let (id, note) = inner.split_once(':')?;
    let id = id.trim();
    if !is_valid_id(id) {
        return None;
    }
    Some(SteerReceipt {
        id: id.to_string(),
        note: note.trim().to_string(),
    })
}

/// Could `s` — a buffer tail known to contain no `]` — still grow into a receipt?
///
/// Drives [`withheld_tail`]: a tail that is still plausibly a receipt is hidden
/// from the live view until it resolves, so a partially streamed marker never
/// flashes on screen.
fn plausible_prefix(s: &str) -> bool {
    // Still streaming the opening literal: "[", "[S", … "[STEERING".
    if OPEN.starts_with(s) {
        return true;
    }
    let Some(rest) = s.strip_prefix(OPEN) else {
        return false;
    };
    let Some(first) = rest.chars().next() else {
        // Exactly "[STEERING" — covered above, but keep the arm total.
        return true;
    };
    if !first.is_whitespace() {
        return false;
    }
    let rest = rest.trim_start();
    if rest.is_empty() {
        // "[STEERING " — the id has not started yet.
        return true;
    }
    match rest.split_once(':') {
        // Id is complete and the note is streaming.
        Some((id, _note)) => is_valid_id(id.trim_end()),
        // Id itself is still streaming: either a prefix of "steer-" or a
        // longer id that has not reached its colon yet.
        None => {
            ID_PREFIX.starts_with(rest)
                || (rest.starts_with(ID_PREFIX) && !rest.contains(char::is_whitespace))
        }
    }
}

/// Remove every complete receipt from `buf`, returning them in emission order.
///
/// Call on each streaming append. Malformed `[STEERING` runs are stepped over
/// and left in the buffer; an unterminated trailing marker is left for a later
/// chunk to complete.
pub fn harvest(buf: &mut String) -> Vec<SteerReceipt> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = buf[from..].find(OPEN) {
        let start = from + rel;
        // `]` is ASCII, so the offset is always a char boundary.
        let Some(rel_end) = buf[start..].find(']') else {
            // Unterminated — a later chunk may finish it. Stop rather than
            // scanning past, so the marker stays harvestable.
            break;
        };
        let end = start + rel_end + 1;
        match parse_marker(&buf[start..end]) {
            Some(receipt) => {
                buf.replace_range(start..end, "");
                found.push(receipt);
                // Resume at the splice point — a second receipt may follow.
                from = start;
            }
            // Not a receipt. Step past this `[STEERING` and keep looking.
            None => from = start + OPEN.len(),
        }
    }
    found
}

/// Byte length of the trailing run of `buf` that must be withheld from the live
/// view because it may still become a receipt.
///
/// Always a suffix, so callers can slice rather than copy. Returns 0 when
/// nothing is pending. Assumes [`harvest`] has already run, i.e. any `]` in the
/// buffer belongs to text rather than to a complete receipt.
pub fn withheld_tail(buf: &str) -> usize {
    // Only the last `[` can open a still-growing marker: anything before it is
    // followed by more text and so is already resolved.
    let Some(open) = buf.rfind('[') else {
        return 0;
    };
    let tail = &buf[open..];
    // A `]` in the tail means this run already resolved — harvest declined it,
    // so it is ordinary text.
    if tail.contains(']') {
        return 0;
    }
    if plausible_prefix(tail) {
        buf.len() - open
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact trailer captured in the 2026-07-23 prompt logs, em dash,
    /// backticks, parens and all. Any regression here means cyril stopped
    /// recognising the real-world shape.
    const CAPTURED: &str = "[STEERING steer-3f2a9c14-7b6d-4e05-9a81-2c5d8e0b41f7: Applied directly — used the external `region_map` sheet as the label source, extracting it once into a committed `src/lib/region-names.json` (rather than reading the per-user path at runtime) and joined it into the CSV export as a Region Name column.]";

    #[test]
    fn harvests_the_captured_trailer() {
        let mut buf = format!("Committed and pushed.\n\n{CAPTURED}");
        let found = harvest(&mut buf);
        assert_eq!(found.len(), 1, "one receipt in {buf:?}");
        assert_eq!(found[0].id(), "steer-3f2a9c14-7b6d-4e05-9a81-2c5d8e0b41f7");
        assert!(
            found[0]
                .note()
                .starts_with("Applied directly — used the external `region_map`"),
            "note was {:?}",
            found[0].note()
        );
        assert!(
            found[0].note().ends_with("as a Region Name column."),
            "note lost its tail: {:?}",
            found[0].note()
        );
        assert_eq!(buf, "Committed and pushed.\n\n", "marker not excised");
    }

    #[test]
    fn harvests_multiple_receipts() {
        let mut buf = "a [STEERING steer-1a: first] b [STEERING steer-2b: second] c".to_string();
        let found = harvest(&mut buf);
        assert_eq!(
            found.iter().map(SteerReceipt::id).collect::<Vec<_>>(),
            ["steer-1a", "steer-2b"]
        );
        assert_eq!(
            found.iter().map(SteerReceipt::note).collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(buf, "a  b  c");
    }

    #[test]
    fn empty_note_is_still_a_receipt() {
        // Proves pickup even when the model says nothing useful.
        let mut buf = "[STEERING steer-x9: ]".to_string();
        let found = harvest(&mut buf);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].note(), "");
        assert_eq!(buf, "");
    }

    #[test]
    fn leaves_non_receipts_alone() {
        // Every one of these opens with `[STEERING` but is not a receipt.
        for text in [
            "[STEERING GROUP]",            // no colon
            "[STEERING committee: notes]", // id lacks the steer- prefix
            "[STEERINGsteer-1: x]",        // no separating whitespace
            "[STEERING steer-: x]",        // prefix only, no id body
            "[STEERING steer- 1: x]",      // whitespace inside the id
        ] {
            let mut buf = text.to_string();
            let found = harvest(&mut buf);
            assert!(found.is_empty(), "{text:?} parsed as a receipt");
            assert_eq!(buf, text, "{text:?} was mutated");
        }
    }

    #[test]
    fn unterminated_marker_is_left_for_a_later_chunk() {
        let mut buf = "done. [STEERING steer-abc: partial no".to_string();
        let found = harvest(&mut buf);
        assert!(found.is_empty());
        assert_eq!(buf, "done. [STEERING steer-abc: partial no");
    }

    #[test]
    fn note_containing_a_bracket_truncates_rather_than_over_strips() {
        // Documented trade-off: stop at the first `]`, never swallow prose that
        // follows. The remainder stays visible.
        let mut buf = "[STEERING steer-z: see docs[1] for why] tail".to_string();
        let found = harvest(&mut buf);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].note(), "see docs[1");
        assert_eq!(buf, " for why] tail", "over-stripped past the first ]");
    }

    #[test]
    fn withholds_every_prefix_of_a_streaming_marker() {
        let full = "ok. [STEERING steer-9f: applied]";
        // Feed the marker one byte at a time; the live view must never expose
        // any part of it.
        let visible_prefix = "ok. ";
        for end in visible_prefix.len()..full.len() {
            let mut buf = full[..end].to_string();
            let receipts = harvest(&mut buf);
            assert!(receipts.is_empty(), "premature harvest at {end}");
            let cut = buf.len() - withheld_tail(&buf);
            assert_eq!(
                &buf[..cut],
                visible_prefix,
                "leaked marker bytes at prefix length {end}: {:?}",
                &buf[..cut]
            );
        }
        // The final byte completes it: harvested, and nothing withheld.
        let mut buf = full.to_string();
        assert_eq!(harvest(&mut buf).len(), 1);
        assert_eq!(buf, visible_prefix);
        assert_eq!(withheld_tail(&buf), 0);
    }

    #[test]
    fn withholds_nothing_for_ordinary_bracket_text() {
        for text in [
            "see [1] and [2]",
            "a markdown [link](url)",
            "no brackets at all",
            "",
            "[STEERING GROUP] met today", // resolved: has `]`, harvest declined
            "[Sting",                     // diverges from "[STEERING" at 'i'
        ] {
            assert_eq!(withheld_tail(text), 0, "withheld from {text:?}");
        }
    }

    #[test]
    fn withholds_an_unterminated_open_bracket_run() {
        // Bare "[" could still become a marker, so it is held back.
        assert_eq!(withheld_tail("done ["), 1);
        assert_eq!(withheld_tail("done [STEER"), 6);
        assert_eq!(withheld_tail("done [STEERING steer-a: par"), 22);
    }

    #[test]
    fn withheld_tail_is_a_char_boundary() {
        // The note carries multi-byte characters; slicing at the withheld
        // offset must not split one.
        let buf = "ok — [STEERING steer-a: café applied";
        let cut = buf.len() - withheld_tail(buf);
        assert!(buf.is_char_boundary(cut), "cut {cut} splits a char");
        assert_eq!(&buf[..cut], "ok — ");
    }
}
