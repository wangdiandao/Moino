//! Independent specification and parser checks; HTML here is an inert test representation.
use super::{Cst, events};
use moino_text::TextBuffer;
use proptest::prelude::*;
use pulldown_cmark::{Options, html};
fn canonical(html: &str) -> String {
    html.replace("&quot;", "\"")
        .replace("<li>\n<", "<li><")
        .trim_end()
        .to_owned()
}
fn backend(source: &str) -> String {
    let mut output = String::new();
    html::push_html(
        &mut output,
        events(source, Options::empty()).map(|(event, _)| event),
    );
    output
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn commonmark_0312_official_examples() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/commonmark-0.31.2.json"))
            .expect("official examples");
    let cases = cases.as_array().expect("case list");
    assert_eq!(cases.len(), 652);
    for case in cases {
        let source = case["markdown"].as_str().expect("Markdown");
        let expected = case["html"].as_str().expect("HTML");
        assert_eq!(
            canonical(&backend(source)),
            canonical(expected),
            "CommonMark example {}",
            case["example"]
        );
        let text = TextBuffer::new(source);
        let parsed = Cst::parse(&text);
        assert!(
            parsed.is_valid(),
            "CST invariants in example {}",
            case["example"]
        );
        assert_eq!(
            parsed.serialize(),
            text.to_string(),
            "lossless example {}",
            case["example"]
        );
    }
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn preserves_code_whitespace_and_handles_tab_continuations() {
    for (source, expected) in [
        ("`\n ` ", "<p><code>  </code></p>\n"),
        ("中\t\t\na", "<p>中\na</p>\n"),
        ("\t]", "<pre><code>]\n</code></pre>\n"),
        (">[\n\t>", "<blockquote>\n<p>[\n&gt;</p>\n</blockquote>\n"),
        ("#\t]\t\t", "<h1>]</h1>\n"),
    ] {
        assert_eq!(backend(source), expected);
    }
}
proptest! {
    #[test]
    fn grammar_fuzz_matches_independent_comrak(parts in prop::collection::vec((0u8..9, "[a-z0-9中 ]{0,40}"), 0..15)) {
        let source = parts.into_iter().map(|(syntax, body)| match syntax {
            0 => body,
            1 => format!("# {body}"),
            2 => format!("**{body}**"),
            3 => format!("> {body}"),
            4 => format!("- {body}"),
            5 => format!("```rust\n{body}\n```"),
            6 => format!("[{body}](https://example.org)"),
            7 => "---".to_owned(),
            _ => format!("`{body}`"),
        }).collect::<Vec<_>>().join("\n\n");
        let mut options = comrak::Options::default();
        options.render.r#unsafe = true;
        let reference = comrak::markdown_to_html(&source, &options);
        let actual = canonical(&backend(&source));
        prop_assert_eq!(actual, canonical(&reference), "source={:?}", source);
    }
}
