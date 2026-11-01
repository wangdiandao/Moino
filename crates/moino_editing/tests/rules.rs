//! Concrete editing outcomes beyond the introductory YAML vectors.
use moino_document::{Affinity, Document, DocumentId, Endpoint, Epoch, Selection};
use moino_editing::{BlockFormat, InlineFormat, Intent, Options, resolve};
use moino_text::ScalarOffset;
use std::time::Duration;
fn run(source: &str, start: usize, end: usize, intent: Intent, pair: bool) -> (String, usize) {
    let mut doc = Document::new(DocumentId::new(1), Epoch::new(0), source);
    let selection = Selection::new(
        Endpoint {
            offset: ScalarOffset::new(start),
            affinity: Affinity::After,
        },
        Endpoint {
            offset: ScalarOffset::new(end),
            affinity: Affinity::After,
        },
    );
    doc.set_selection(selection).expect("initial selection");
    let options = Options::new("  ", pair).expect("options");
    let outcome = resolve(&doc.snapshot(), selection, intent, &options).expect("resolve");
    let caret = outcome.selection().head().offset.value();
    if let Some(transaction) = outcome.into_transaction() {
        doc.commit(transaction, Duration::ZERO).expect("commit");
    }
    (doc.snapshot().text().to_string(), caret)
}
#[test]
// Spec: docs/modules/editing.md#enter
fn enter_continues_containers_and_preserves_literal_newlines() {
    for (source, caret, expected, next) in [
        ("- item", 6, "- item\n- ", 9),
        ("9. item", 7, "9. item\n10. ", 12),
        ("- [x] item", 10, "- [x] item\n- [ ] ", 17),
        ("- ", 2, "", 0),
        ("> quote", 7, "> quote\n> ", 10),
        ("> ", 2, "", 0),
        ("# title", 2, "\n\n# title", 4),
        ("```\nabc\n```", 5, "```\na\nbc\n```", 6),
        ("<pre>abc</pre>", 5, "<pre>\nabc</pre>", 6),
    ] {
        assert_eq!(
            run(source, caret, caret, Intent::Enter, false),
            (expected.to_owned(), next)
        );
    }
}
#[test]
// Spec: docs/modules/editing.md#backspace-与-delete
fn deletion_uses_graphemes_and_preserves_atomic_selection_replacement() {
    for (source, start, end, intent, expected, caret) in [
        ("e\u{301}墨", 2, 2, Intent::Backspace, "墨", 0),
        ("👨‍👩‍👧‍👦墨", 0, 0, Intent::Delete, "墨", 0),
        ("a\n\n# b", 1, 1, Intent::Delete, "ab", 1),
        ("a\n\nb", 3, 3, Intent::Backspace, "ab", 1),
        ("# title", 2, 2, Intent::Backspace, "title", 0),
        ("hello", 1, 4, Intent::Backspace, "ho", 1),
        ("hello", 1, 4, Intent::Delete, "ho", 1),
        ("", 0, 0, Intent::Backspace, "", 0),
        ("", 0, 0, Intent::Delete, "", 0),
    ] {
        assert_eq!(
            run(source, start, end, intent, false),
            (expected.to_owned(), caret)
        );
    }
    assert_eq!(
        run("para\n2. item", 8, 8, Intent::Backspace, false).0,
        "para\n2.item"
    );
    assert_eq!(
        run("```\n- item\n```", 6, 6, Intent::Backspace, false).0,
        "```\n-item\n```"
    );
    assert_eq!(
        run("```\na\n# literal\n```", 5, 5, Intent::Delete, false).0,
        "```\na# literal\n```"
    );
    assert_eq!(
        run("---\n- item\n---", 6, 6, Intent::Backspace, false).0,
        "---\n-item\n---"
    );
}
#[test]
// Spec: docs/modules/editing.md#tab-与缩进
fn indentation_and_block_formats_toggle() {
    assert_eq!(
        run("abc", 1, 1, Intent::Tab { reverse: false }, false),
        ("a  bc".to_owned(), 3)
    );
    assert_eq!(
        run("- a", 3, 3, Intent::Tab { reverse: false }, false).0,
        "- a"
    );
    assert_eq!(
        run("  - a", 5, 5, Intent::Tab { reverse: true }, false).0,
        "a"
    );
    assert_eq!(
        run("a\nb", 0, 3, Intent::Tab { reverse: false }, false).0,
        "  a\n  b"
    );
    for (format, prefix) in [
        (BlockFormat::Heading(6), "###### "),
        (BlockFormat::Quote, "> "),
        (BlockFormat::BulletList, "- "),
        (BlockFormat::OrderedList, "1. "),
        (BlockFormat::TaskList, "- [ ] "),
    ] {
        let source = format!("{prefix}text");
        assert_eq!(run("text", 4, 4, Intent::Block(format), false).0, source);
        assert_eq!(
            run(
                &source,
                source.chars().count(),
                source.chars().count(),
                Intent::Block(format),
                false
            )
            .0,
            "text"
        );
        assert_eq!(
            run(
                &source,
                source.chars().count(),
                source.chars().count(),
                Intent::Block(BlockFormat::Paragraph),
                false
            )
            .0,
            "text"
        );
    }
}
#[test]
// Spec: docs/modules/editing.md#格式命令
fn inline_formats_toggle_and_pairing_is_disabled_for_composition() {
    for (format, marker) in [
        (InlineFormat::Bold, "**"),
        (InlineFormat::Italic, "*"),
        (InlineFormat::Code, "`"),
        (InlineFormat::Strike, "~~"),
        (InlineFormat::Highlight, "=="),
        (InlineFormat::Subscript, "~"),
        (InlineFormat::Superscript, "^"),
    ] {
        let source = format!("{marker}墨{marker}");
        assert_eq!(run("墨", 0, 1, Intent::Inline(format), false).0, source);
        assert_eq!(
            run(
                &source,
                0,
                source.chars().count(),
                Intent::Inline(format),
                false
            )
            .0,
            "墨"
        );
        assert_eq!(run("", 0, 0, Intent::Inline(format), false).1, marker.len());
    }
    assert_eq!(
        run("", 0, 0, Intent::Insert("(".to_owned()), true),
        ("()".to_owned(), 1)
    );
    assert_eq!(
        run("x", 0, 1, Intent::Insert("[".to_owned()), true).0,
        "[x]"
    );
    assert_eq!(
        run("()", 1, 1, Intent::Insert(")".to_owned()), true),
        ("()".to_owned(), 2)
    );
    assert_eq!(
        run("", 0, 0, Intent::Composition("(".to_owned()), true).0,
        "("
    );
    assert_eq!(
        run("`code`", 2, 2, Intent::Insert("(".to_owned()), true).0,
        "`c(ode`"
    );
    assert!(Options::new("x", true).is_err());
    assert!(Options::new("", false).is_err());
    let options = Options::new("\t", true).expect("tab").read_only(true);
    assert!(options.is_read_only());
    let doc = Document::new(DocumentId::new(1), Epoch::new(0), "");
    let error = resolve(
        &doc.snapshot(),
        Selection::cursor(ScalarOffset::new(0)),
        Intent::Enter,
        &options,
    )
    .expect_err("read only");
    assert_eq!(error.code(), "editing.unavailable");
    assert_eq!(error.message_key(), "error.editing.unavailable");
    assert!(!error.is_retryable());
    assert!(
        resolve(
            &doc.snapshot(),
            Selection::cursor(ScalarOffset::new(0)),
            Intent::Block(BlockFormat::Heading(7)),
            &Options::new("  ", false).expect("options")
        )
        .is_err()
    );
}
#[test]
// Spec: docs/modules/editing.md#自动配对与智能标点
fn empty_pairs_upgrade_and_structural_markers_are_deleted_before_content() {
    for (source, caret, value, expected, next) in [
        ("**", 1, "*", "****", 2),
        ("~~", 1, "~", "~~~~", 2),
        ("$$", 1, "$", "$$\n\n$$", 3),
        ("=", 1, "=", "====", 2),
        ("[]", 1, "]", "[]()", 1),
        ("``", 2, "`", "```\n\n```", 4),
    ] {
        assert_eq!(
            run(source, caret, caret, Intent::Insert(value.to_owned()), true),
            (expected.to_owned(), next)
        );
    }
    assert_eq!(
        run("**墨**", 2, 2, Intent::Backspace, false),
        ("墨".to_owned(), 0)
    );
    assert_eq!(
        run("**墨**", 3, 3, Intent::Delete, false),
        ("墨".to_owned(), 1)
    );
    assert_eq!(run("[墨](url)", 5, 5, Intent::Backspace, false).0, "墨");
    assert_eq!(run("  - item", 4, 4, Intent::Backspace, false).0, "item");
    assert_eq!(
        run("# **墨**", 2, 7, Intent::ClearFormatting, false).0,
        "墨"
    );
    assert_eq!(
        run("plain", 0, 5, Intent::ClearFormatting, false).0,
        "plain"
    );
    assert_eq!(run("", 0, 0, Intent::Paste("(".to_owned()), true).0, "(");
}
#[test]
// Spec: docs/modules/editing.md#enter
fn ordered_enter_renumbers_siblings_and_continues_quoted_lists() {
    assert_eq!(
        run("1. one\n2. two\n3. three", 6, 6, Intent::Enter, false).0,
        "1. one\n2. \n3. two\n4. three"
    );
    assert_eq!(
        run("> - item", 8, 8, Intent::Enter, false).0,
        "> - item\n> - "
    );
    assert_eq!(
        run("> - ", 4, 4, Intent::Enter, false),
        ("> ".to_owned(), 2)
    );
    assert_eq!(
        run("> # title", 9, 9, Intent::Enter, false).0,
        "> # title\n> "
    );
    assert_eq!(
        run(
            "> - prev\n> - item",
            17,
            17,
            Intent::Tab { reverse: false },
            false
        )
        .0,
        "> - prev\n>   - item"
    );
    assert_eq!(
        run("> - prev\n>   - item", 15, 15, Intent::Backspace, false).0,
        "> - prev\n> - item"
    );
}
#[test]
// Spec: docs/modules/editing.md#格式命令
fn block_formats_transform_all_soft_lines_and_preserve_following_blocks() {
    assert_eq!(
        run("a\nb", 3, 3, Intent::Block(BlockFormat::Heading(1)), false),
        ("# a b".to_owned(), 5)
    );
    assert_eq!(
        run("a\nb", 3, 3, Intent::Block(BlockFormat::Quote), false).0,
        "> a\n> b"
    );
    assert_eq!(
        run("a\nb", 3, 3, Intent::Block(BlockFormat::BulletList), false).0,
        "- a\n  b"
    );
    assert_eq!(
        run("a\nb", 3, 3, Intent::Block(BlockFormat::OrderedList), false).0,
        "1. a\n   b"
    );
    assert_eq!(
        run("a\nb", 3, 3, Intent::Block(BlockFormat::TaskList), false).0,
        "- [ ] a\n  b"
    );
    assert_eq!(
        run(
            "# a\n# b",
            4,
            4,
            Intent::Block(BlockFormat::Paragraph),
            false
        )
        .0,
        "# a\nb"
    );
    assert_eq!(
        run("# a\n# b", 0, 4, Intent::ClearFormatting, false).0,
        "a\n# b"
    );
}
#[test]
// Spec: docs/modules/editing.md#tab-与缩进
fn table_tab_navigates_and_appends_only_after_last_cell() {
    let source = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let (unchanged, caret) = run(source, 2, 2, Intent::Tab { reverse: false }, false);
    assert_eq!(unchanged, source);
    assert_eq!(caret, 6);
    assert_eq!(run(source, 6, 6, Intent::Tab { reverse: true }, false).1, 2);
    let last = source.chars().position(|ch| ch == '2').expect("last cell");
    assert_eq!(
        run(source, last, last, Intent::Tab { reverse: false }, false).0,
        format!("{source}|  |  |\n")
    );
}
#[test]
// Spec: docs/modules/editing.md#tab-与缩进
fn list_indentation_preserves_cursor_and_uses_the_previous_container_width() {
    let source = "9. prev\n10. item";
    assert_eq!(
        run(source, 14, 14, Intent::Tab { reverse: false }, false),
        ("9. prev\n   1. item".to_owned(), 16)
    );
    assert_eq!(
        run(
            "9. prev\n   1. item",
            16,
            16,
            Intent::Tab { reverse: true },
            false
        ),
        ("9. prev\n1. item".to_owned(), 13)
    );
    assert_eq!(
        run(
            "- prev\n- item\n  tail",
            18,
            18,
            Intent::Tab { reverse: false },
            false
        ),
        ("- prev\n  - item\n    tail".to_owned(), 22)
    );
    assert_eq!(
        run("- item\n  tail", 3, 3, Intent::Tab { reverse: true }, false),
        ("item\ntail".to_owned(), 1)
    );
    let mut doc = Document::new(DocumentId::new(1), Epoch::new(0), "a\nb");
    let selection = Selection::new(
        Endpoint {
            offset: ScalarOffset::new(3),
            affinity: Affinity::After,
        },
        Endpoint {
            offset: ScalarOffset::new(0),
            affinity: Affinity::After,
        },
    );
    doc.set_selection(selection).expect("backward selection");
    let outcome = resolve(
        &doc.snapshot(),
        selection,
        Intent::Tab { reverse: false },
        &Options::new("  ", false).expect("options"),
    )
    .expect("indent");
    assert_eq!(outcome.selection().anchor().offset.value(), 7);
    assert_eq!(outcome.selection().head().offset.value(), 2);
    doc.commit(outcome.into_transaction().expect("edits"), Duration::ZERO)
        .expect("commit");
    doc.undo().expect("undo indentation");
    assert_eq!(doc.selection(), selection);
    assert_eq!(doc.snapshot().text().to_string(), "a\nb");
}
#[test]
// Spec: docs/modules/editing.md#backspace-与-delete
fn heading_backspace_preserves_outer_containers_and_handles_setext_at_document_start() {
    assert_eq!(
        run("- # title", 4, 4, Intent::Backspace, false),
        ("- title".to_owned(), 2)
    );
    assert_eq!(
        run("> # title", 4, 4, Intent::Backspace, false),
        ("> title".to_owned(), 2)
    );
    assert_eq!(
        run("Title\n===\n", 0, 0, Intent::Backspace, false),
        ("Title\n".to_owned(), 0)
    );
}
