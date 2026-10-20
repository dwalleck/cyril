//! Ownership behavior for the presentation view over authoritative tool output.

use crate::traits::{ToolText, TrackedToolCall};
use cyril_core::types::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};
use serde_json::{Value, json};

fn tracked_output(output: Value) -> TrackedToolCall {
    TrackedToolCall::new(
        ToolCall::new(
            ToolCallId::new("borrowed-output"),
            "Output fixture".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(output)),
    )
}

#[test]
fn output_view_borrows_the_selected_authoritative_json_string() {
    let cases = [
        (json!("root\r\npayload"), ""),
        (
            json!({"stdout": "out\r\npayload", "stderr": "not selected"}),
            "/stdout",
        ),
        (
            json!({"stdout": "  \n", "stderr": "err\npayload"}),
            "/stderr",
        ),
        (
            json!({"items": [{"Text": "item\npayload", "Json": {"ignored": 1}}]}),
            "/items/0/Text",
        ),
        (
            json!({"text": "first", "content": "second", "result": "third"}),
            "/text",
        ),
        (json!({"content": "middle", "result": "last"}), "/content"),
        (json!({"result": "last"}), "/result"),
    ];

    for (output, path) in cases {
        let tracked = tracked_output(output);
        let Some(expected) = tracked
            .raw_output()
            .and_then(|value| value.pointer(path))
            .and_then(Value::as_str)
        else {
            panic!("fixture does not contain a string at {path}");
        };
        let Some(ToolText::Borrowed { source, text }) = tracked.output_text_view() else {
            panic!("selected authoritative string at {path} must be borrowed");
        };
        assert_eq!(text, expected, "selected branch at {path}");
        assert!(std::ptr::eq(text.as_ptr(), expected.as_ptr()), "{path}");
        let Some(resolved) = tracked.resolve_text_source(source) else {
            panic!("selected source must resolve while the tool is unchanged");
        };
        assert_eq!(resolved, expected);
        assert!(std::ptr::eq(resolved.as_ptr(), expected.as_ptr()), "{path}");
    }
}

#[test]
fn output_source_resolution_keeps_the_recorded_branch_without_reselection() {
    let mut tracked = tracked_output(json!({
        "stdout": "  \r\n",
        "stderr": "selected stderr",
        "text": "unselected direct field"
    }));
    let Some(ToolText::Borrowed { source, text }) = tracked.output_text_view() else {
        panic!("stderr must supply the borrowed output");
    };
    assert_eq!(text, "selected stderr");
    for _ in 0..100 {
        let Some(resolved) = tracked.resolve_text_source(source) else {
            panic!("the recorded branch must stay resolvable");
        };
        assert_eq!(resolved, text);
        assert!(std::ptr::eq(resolved.as_ptr(), text.as_ptr()));
    }

    tracked.update(
        &ToolCall::new(
            ToolCallId::new("borrowed-output"),
            "Updated output fixture".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            None,
        )
        .with_raw_output(Some(json!({
            "stdout": "new preferred stdout",
            "stderr": "selected stderr"
        }))),
    );
    assert_eq!(
        tracked.resolve_text_source(source),
        Some("selected stderr"),
        "resolution follows recorded source metadata, not current precedence"
    );
    let Some(ToolText::Borrowed { text, .. }) = tracked.output_text_view() else {
        panic!("a newly prepared view must select the new preferred source");
    };
    assert_eq!(text, "new preferred stdout");
}

#[test]
fn only_serialized_json_owns_derived_output_text() {
    let cases = [
        (
            json!([1, {"kind": "data"}]),
            "[\n  1,\n  {\n    \"kind\": \"data\"\n  }\n]",
        ),
        (
            json!({"items": [{"Json": {"answer": 42}}]}),
            "{\n  \"answer\": 42\n}",
        ),
        (Value::Null, "null"),
        (json!(true), "true"),
    ];
    for (output, expected) in cases {
        let tracked = tracked_output(output);
        let Some(ToolText::Derived(text)) = tracked.output_text_view() else {
            panic!("serialization must produce owned derived backing");
        };
        assert_eq!(text, expected);
    }
}

#[test]
fn unsupported_object_shapes_have_no_invented_output() {
    for output in [
        json!({}),
        json!({"unknown": "not output"}),
        json!({"items": []}),
    ] {
        assert!(tracked_output(output).output_text_view().is_none());
    }
}

#[test]
fn failed_text_content_is_borrowed_without_trimming_its_display() {
    use cyril_core::types::ToolCallContent;

    for raw_output in [None, Some(json!({"unrecognized": "value"}))] {
        let tracked = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("content-error"),
                "Failed read".into(),
                ToolKind::Read,
                ToolCallStatus::Failed,
                None,
            )
            .with_content(vec![
                ToolCallContent::Text(" \t\n".into()),
                ToolCallContent::Text("  validation error\r\nat file  ".into()),
                ToolCallContent::Text("later error".into()),
            ])
            .with_raw_output(raw_output),
        );
        let Some(ToolCallContent::Text(expected)) = tracked.content().get(1) else {
            panic!("fixture must contain the authoritative first nonblank text");
        };
        let Some(ToolText::Borrowed { source, text }) = tracked.error_text_view() else {
            panic!("fiv1 content error must remain borrowed");
        };
        assert_eq!(text, "  validation error\r\nat file  ");
        assert!(std::ptr::eq(text.as_ptr(), expected.as_ptr()));
        assert_eq!(tracked.resolve_text_source(source), Some(text));
        let Some(std::borrow::Cow::Borrowed(public)) = tracked.error_message() else {
            panic!("public error consumer must not copy the selected text");
        };
        assert!(std::ptr::eq(public.as_ptr(), expected.as_ptr()));
    }
}

#[test]
fn failed_raw_error_precedence_is_borrowed_and_completion_has_no_error() {
    use cyril_core::types::ToolCallContent;

    for (output, path) in [
        (json!("raw root"), ""),
        (json!({"error": "first", "message": "second"}), "/error"),
        (json!({"message": "second", "stderr": "third"}), "/message"),
        (json!({"stderr": "third"}), "/stderr"),
        (json!(""), ""),
    ] {
        let mut tracked = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("raw-error"),
                "Failed read".into(),
                ToolKind::Read,
                ToolCallStatus::Failed,
                None,
            )
            .with_content(vec![ToolCallContent::Text("content fallback".into())])
            .with_raw_output(Some(output)),
        );
        let Some(expected) = tracked
            .raw_output()
            .and_then(|value| value.pointer(path))
            .and_then(Value::as_str)
        else {
            panic!("fixture must supply the selected raw error");
        };
        let Some(ToolText::Borrowed { source, text }) = tracked.error_text_view() else {
            panic!("raw error must take precedence over content");
        };
        assert_eq!(text, expected);
        assert!(std::ptr::eq(text.as_ptr(), expected.as_ptr()));
        assert_eq!(tracked.resolve_text_source(source), Some(expected));
        tracked.update(&ToolCall::new(
            ToolCallId::new("raw-error"),
            String::new(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        ));
        assert!(tracked.error_text_view().is_none());
        assert!(tracked.error_message().is_none());
    }
}
