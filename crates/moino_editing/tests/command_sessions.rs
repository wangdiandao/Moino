//! Command sessions verify source, directional selection and a single shared undo unit.
use moino_document::{Affinity, Document, DocumentId, Endpoint, Epoch, Selection};
use moino_editing::{BlockFormat, InlineFormat, Intent, Options, resolve};
use moino_text::ScalarOffset;
use std::time::Duration;

fn selected(anchor: usize, head: usize) -> Selection {
    Selection::new(
        Endpoint {
            offset: ScalarOffset::new(anchor),
            affinity: Affinity::After,
        },
        Endpoint {
            offset: ScalarOffset::new(head),
            affinity: Affinity::After,
        },
    )
}
fn execute(source: &str, selection: Selection, intent: Intent, options: Options) -> Document {
    let mut document = Document::new(DocumentId::new(7), Epoch::new(0), source);
    document.set_selection(selection).expect("selection");
    let outcome = resolve(&document.snapshot(), selection, intent, &options).expect("command");
    if let Some(transaction) = outcome.into_transaction() {
        document
            .commit(transaction, Duration::ZERO)
            .expect("atomic command");
    }
    document
}
fn options() -> Options {
    Options::new("    ", true).expect("options")
}

#[test]
// Spec: docs/modules/editing.md#格式命令
fn formatting_at_a_word_selects_the_word_and_toggles_existing_wrappers() {
    let mut document = execute(
        "alpha beta",
        selected(2, 2),
        Intent::Inline(InlineFormat::Bold),
        options(),
    );
    assert_eq!(document.snapshot().text().to_string(), "**alpha** beta");
    assert_eq!(
        document.selection().range(),
        ScalarOffset::new(2)..ScalarOffset::new(7)
    );
    document.undo().expect("undo");
    assert_eq!(document.snapshot().text().to_string(), "alpha beta");
    assert_eq!(document.selection(), selected(2, 2));
    let document = execute(
        "**alpha** beta",
        selected(4, 4),
        Intent::Inline(InlineFormat::Bold),
        options(),
    );
    assert_eq!(document.snapshot().text().to_string(), "alpha beta");
    assert_eq!(document.selection(), selected(2, 2));
}

#[test]
// Spec: docs/modules/editing.md#格式命令
fn inline_commands_normalize_mixed_styles_and_keep_block_boundaries() {
    for (source, expected) in [
        ("**a** __b__", "a b"),
        ("a **b**", "**a b**"),
        ("a\n\nb", "**a**\n\n**b**"),
    ] {
        let selection = selected(source.chars().count(), 0);
        let mut document = execute(
            source,
            selection,
            Intent::Inline(InlineFormat::Bold),
            options(),
        );
        assert_eq!(document.snapshot().text().to_string(), expected);
        assert!(document.selection().anchor().offset > document.selection().head().offset);
        document.undo().expect("undo");
        assert_eq!(document.snapshot().text().to_string(), source);
        assert_eq!(document.selection(), selection);
        assert!(!document.has_undo());
    }
}

#[test]
// Spec: docs/modules/editing.md#格式命令
fn clear_formatting_uses_the_selected_blocks_instead_of_selected_bytes() {
    let source = "**one** and *two*\n\n**three**";
    let document = execute(source, selected(9, 10), Intent::ClearFormatting, options());
    assert_eq!(
        document.snapshot().text().to_string(),
        "one and two\n\n**three**"
    );
}

#[test]
// Spec: docs/modules/editing.md#交互语法转换
fn draft_heading_deletion_does_not_apply_committed_block_rules() {
    let source = "# text";
    let draft = options()
        .with_pending_block(ScalarOffset::new(0)..ScalarOffset::new(source.chars().count()));
    let document = execute(source, selected(2, 2), Intent::Backspace, draft);
    assert_eq!(document.snapshot().text().to_string(), "#text");
    let document = execute(source, selected(2, 2), Intent::Backspace, options());
    assert_eq!(document.snapshot().text().to_string(), "text");
}

#[test]
// Spec: docs/modules/editing.md#enter
fn heading_middle_enter_clones_heading_but_end_enter_creates_paragraph() {
    for pending in [false, true] {
        let options = if pending {
            options().with_pending_block(ScalarOffset::new(0)..ScalarOffset::new(7))
        } else {
            options()
        };
        let document = execute("# title", selected(4, 4), Intent::Enter, options.clone());
        assert_eq!(document.snapshot().text().to_string(), "# ti\n\n# tle");
        let document = execute("# title", selected(7, 7), Intent::Enter, options);
        assert_eq!(document.snapshot().text().to_string(), "# title\n\n");
    }
}

#[test]
// Spec: docs/modules/editing.md#backspace-与-delete
fn container_backspace_changes_one_semantic_layer_and_undo_restores_it() {
    for (source, at, expected) in [
        ("> alpha\n> beta", 2, "alpha\nbeta"),
        ("> > alpha\n> > beta", 4, "> alpha\n> beta"),
        ("- > alpha\n  > beta", 4, "- alpha\n  beta"),
        ("- [x] alpha", 6, "- [ ] alpha"),
        ("- [ ] alpha", 6, "alpha"),
    ] {
        let selection = selected(at, at);
        let mut document = execute(source, selection, Intent::Backspace, options());
        assert_eq!(document.snapshot().text().to_string(), expected, "{source}");
        document.undo().expect("undo");
        assert_eq!(document.snapshot().text().to_string(), source);
        assert_eq!(document.selection(), selection);
    }
}

#[test]
// Spec: docs/modules/editing.md#格式命令
fn clear_formatting_removes_quote_layers_on_every_selected_block_line() {
    for (source, at, expected) in [
        ("> **one**\n> *two*", 5, "one\ntwo"),
        ("> > **one**\n> > *two*", 7, "one\ntwo"),
    ] {
        let document = execute(source, selected(at, at), Intent::ClearFormatting, options());
        assert_eq!(document.snapshot().text().to_string(), expected);
    }
}

#[test]
// Spec: docs/modules/editing.md#格式命令
fn code_formatting_selects_content_inside_safe_delimiters_and_padding() {
    let source = "`word";
    let mut document = execute(
        source,
        selected(5, 0),
        Intent::Inline(InlineFormat::Code),
        options(),
    );
    assert_eq!(document.snapshot().text().to_string(), "`` `word ``");
    assert_eq!(
        document.selection().range(),
        ScalarOffset::new(3)..ScalarOffset::new(8)
    );
    assert!(!document.selection().is_forward());
    document.undo().expect("undo");
    assert_eq!(document.snapshot().text().to_string(), source);
    assert_eq!(document.selection(), selected(5, 0));
}

#[test]
// Spec: docs/modules/editing.md#enter
fn task_split_preserves_checked_content_and_unchecks_the_empty_item() {
    for (at, expected) in [
        (6, "- [ ] \n- [x] word"),
        (8, "- [x] wo\n- [x] rd"),
        (10, "- [x] word\n- [ ] "),
    ] {
        let mut document = execute("- [x] word", selected(at, at), Intent::Enter, options());
        assert_eq!(document.snapshot().text().to_string(), expected);
        document.undo().expect("undo");
        assert_eq!(document.snapshot().text().to_string(), "- [x] word");
        assert_eq!(document.selection(), selected(at, at));
    }
}

#[test]
// Spec: docs/modules/editing.md#格式命令
fn block_format_converts_every_selected_leaf_in_one_history_unit() {
    let source = "one\n\ntwo";
    let mut document = execute(
        source,
        selected(8, 0),
        Intent::Block(BlockFormat::Heading(2)),
        options(),
    );
    assert_eq!(document.snapshot().text().to_string(), "## one\n\n## two");
    assert_eq!(
        document.selection().range(),
        ScalarOffset::new(3)..ScalarOffset::new(14)
    );
    assert!(!document.selection().is_forward());
    document.undo().expect("undo");
    assert_eq!(document.snapshot().text().to_string(), source);
    assert_eq!(document.selection(), selected(8, 0));
    assert!(!document.has_undo());
}

#[test]
// Spec: docs/modules/editing.md#backspace-与-delete
fn backspace_at_fence_content_start_demotes_before_deleting_content() {
    let mut document = execute(
        "```rust\ncode\n```\n\nafter",
        selected(8, 8),
        Intent::Backspace,
        options(),
    );
    assert_eq!(document.snapshot().text().to_string(), "code\n\n\nafter");
    document.undo().expect("undo");
    assert_eq!(
        document.snapshot().text().to_string(),
        "```rust\ncode\n```\n\nafter"
    );
}

#[test]
// Spec: docs/modules/editing.md#backspace-与-delete
fn cross_block_delete_keeps_receiver_style_and_removes_donor_markers() {
    for (source, from, to, expected) in [
        ("one\n\n# title ###", 2, 9, "ontle"),
        ("one\n\nTitle\n=====", 2, 7, "ontle"),
        ("one\n\n> alpha\n> beta", 2, 9, "onpha\nbeta"),
        ("one\n\n- alpha\n  beta", 2, 9, "onpha\nbeta"),
        ("# one ###\n\nplain", 4, 13, "# onain ###"),
        ("> one\n>\n> alpha\n> beta", 4, 12, "> onpha\n> beta"),
    ] {
        for intent in [Intent::Backspace, Intent::Delete] {
            let selection = selected(to, from);
            let mut document = execute(source, selection, intent, options());
            assert_eq!(document.snapshot().text().to_string(), expected, "{source}");
            document.undo().expect("undo");
            assert_eq!(document.snapshot().text().to_string(), source);
            assert_eq!(document.selection(), selection);
        }
    }
}
