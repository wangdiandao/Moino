use super::*;
use moino_document::{Document, DocumentId, Epoch};

#[test]
// Spec: docs/modules/editing.md#交互语法转换
fn empty_containers_keep_their_semantics_and_an_editable_content_position() {
    for (source, at) in [("> ", 2), (">\n", 1), ("- ", 2), ("- [ ] ", 6), ("> > ", 4)] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(ScalarOffset::new(at)),
            moino_theme::BuiltinTheme::Paper,
        );
        assert!(
            matches!(
                leaves[0].block,
                BlockNode::Blockquote { .. } | BlockNode::List { .. }
            ),
            "{source:?}"
        );
        assert!(
            leaves[0].text.is_empty(),
            "{source:?}: {:?}, CST={:#?}",
            leaves[0].text,
            document.snapshot().cst().nodes()
        );
        assert_eq!(leaves[0].source(0).value(), at, "{source:?}");
    }
}

#[test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn headings_hide_structural_markers_for_every_selection_and_keep_empty_content_positions() {
    for (source, text, start) in [
        ("# Title ###", "Title", 2),
        ("## Title", "Title", 3),
        ("Title\n=====", "Title", 0),
        ("Title\n-----", "Title", 0),
        ("## ", " ", 2),
        ("#\t", "\t", 1),
    ] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        for at in 0..=source.chars().count() {
            let leaves = project(
                &document.snapshot(),
                Selection::cursor(ScalarOffset::new(at)),
                moino_theme::BuiltinTheme::Paper,
            );
            let leaf = &leaves[0];
            assert_eq!(leaf.text, text, "{source:?} at {at}");
            assert!(matches!(leaf.block, BlockNode::Heading { .. }));
            assert_eq!(leaf.source(0).value(), start, "{source:?}");
            assert_eq!(
                leaf.byte(ScalarOffset::new(usize::MAX), Affinity::Before),
                text.len()
            );
        }
    }
}

#[test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn expanded_inline_content_retains_styles_without_styling_its_own_markers() {
    for source in [
        "**中😀**",
        "*中😀*",
        "~~中😀~~",
        "==中😀==",
        "`中😀`",
        "[中😀](https://example.com)",
        "~中😀~",
        "^中😀^",
    ] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(ScalarOffset::new(2)),
            moino_theme::BuiltinTheme::Paper,
        );
        assert_eq!(leaves[0].text, source);
        let BlockNode::Paragraph(paragraph) = &leaves[0].block else {
            panic!("paragraph");
        };
        let content = paragraph
            .children()
            .iter()
            .find(|run| run.text() == "中😀")
            .expect("content run");
        assert!(content.foreground().is_none());
        assert!(
            content.font_size_scale().is_some()
                || content.mark_runs().iter().any(|(_, mark)| mark.bold
                    || mark.italic
                    || mark.strikethrough
                    || mark.code
                    || mark.highlight.is_some()
                    || mark.link.is_some()),
            "{source}"
        );
        for marker in paragraph
            .children()
            .iter()
            .filter(|run| run.foreground().is_some())
        {
            assert!(
                marker.mark_runs().is_empty(),
                "{source}: {:?}",
                marker.mark_runs()
            );
            assert!(marker.font_size_scale().is_none());
        }
    }
}

#[test]
// Spec: docs/modules/editing.md#输入法组合
fn preedit_preserves_styled_text_in_the_same_leaf_and_shifts_following_heading_positions() {
    let source = "**bold** gap *ital*\n\n## tail";
    let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
    let range = ScalarOffset::new(9)..ScalarOffset::new(10);
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(range.start),
        moino_theme::BuiltinTheme::Paper,
    );
    let heading_start = leaves.last().expect("heading").source(0);
    let leaves = compose(leaves, range, "中😀");
    assert_eq!(leaves[0].text, "bold 中😀ap ital");
    let BlockNode::Paragraph(paragraph) = &leaves[0].block else {
        panic!("paragraph");
    };
    assert!(paragraph.children().iter().any(|run| {
        run.text().starts_with("bold")
            && run
                .mark_runs()
                .iter()
                .any(|(range, mark)| mark.bold && &run.text()[range.clone()] == "bold")
    }));
    assert!(
        paragraph
            .children()
            .iter()
            .any(|run| run.text().contains("ital")
                && run.mark_runs().iter().any(|(_, mark)| mark.italic))
    );
    let heading = leaves.last().expect("heading");
    assert_eq!(heading.source(0).value(), heading_start.value() + 1);
    assert_eq!(
        heading.content_position(heading.raw.start),
        heading.source(0)
    );
    assert_eq!(document.snapshot().revision().value(), 0);
}

#[test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn heading_style_is_stable_and_its_badge_excludes_terminating_newlines() {
    for level in 1_u8..=6 {
        for suffix in ["\n", "\n\n", "\n\nbody"] {
            let heading = format!("{} 中😀", "#".repeat(usize::from(level)));
            let source = format!("{heading}{suffix}");
            let document = Document::new(DocumentId::new(1), Epoch::new(0), &source);
            let end = heading.chars().count();
            for caret in [end, end + 1] {
                let leaves = project(
                    &document.snapshot(),
                    Selection::cursor(ScalarOffset::new(caret)),
                    moino_theme::BuiltinTheme::Paper,
                );
                assert_eq!(
                    leaves[0].heading_badge,
                    (caret == end).then_some(level),
                    "{source:?} at {caret}"
                );
                assert!(
                    matches!(leaves[0].block, BlockNode::Heading { .. }),
                    "{source:?} at {caret}"
                );
            }
        }
    }
    for source in [
        "Title\n===\n",
        "Title\n---\n",
        "> ## title\n",
        "- ## title\n",
    ] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(document.snapshot().text().len()),
            moino_theme::BuiltinTheme::Paper,
        );
        assert!(leaves[0].heading_badge.is_none(), "{source:?}");
    }
    for source in ["---\n", "***\n\n"] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(ScalarOffset::new(4)),
            moino_theme::BuiltinTheme::Paper,
        );
        assert!(leaves[0].rule, "{source:?}");
    }
}

#[test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn collapsed_markers_map_unicode_back_to_source() {
    let document = Document::new(DocumentId::new(1), Epoch::new(0), "**中😀** end");
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(ScalarOffset::new(10)),
        moino_theme::BuiltinTheme::Paper,
    );
    assert_eq!(leaves[0].text, "中😀 end");
    assert_eq!(leaves[0].source(3), ScalarOffset::new(3));
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(ScalarOffset::new(3)),
        moino_theme::BuiltinTheme::Paper,
    );
    assert_eq!(leaves[0].text, "**中😀** end");
    let BlockNode::Paragraph(paragraph) = &leaves[0].block else {
        panic!("paragraph projection")
    };
    assert_eq!(
        paragraph.children()[0].foreground(),
        Some(moino_theme::BuiltinTheme::Paper.marker())
    );
    assert_eq!(paragraph.children()[1].foreground(), None);
    assert_eq!(document.snapshot().revision().value(), 0);
}

#[test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn expanding_an_outer_mark_also_reveals_all_nested_markers() {
    let document = Document::new(DocumentId::new(1), Epoch::new(0), "**a *中* z** end");
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(ScalarOffset::new(3)),
        moino_theme::BuiltinTheme::Paper,
    );
    assert_eq!(leaves[0].text, "**a *中* z** end");
    let BlockNode::Paragraph(paragraph) = &leaves[0].block else {
        panic!("paragraph")
    };
    assert!(
        paragraph
            .children()
            .iter()
            .flat_map(InlineNode::mark_runs)
            .any(|(_, mark)| mark.italic)
    );
    assert_eq!(document.snapshot().revision().value(), 0);
}

#[test]
// Spec: docs/design/ink-theme.md#正文颜色
fn inline_highlight_uses_the_effective_theme_and_preserves_source_coordinates() {
    let document = Document::new(DocumentId::new(1), Epoch::new(0), "==中😀== end");
    for theme in [
        moino_theme::BuiltinTheme::Paper,
        moino_theme::BuiltinTheme::Inkstone,
    ] {
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(document.snapshot().text().len()),
            theme,
        );
        assert_eq!(leaves[0].text, "中😀 end");
        assert_eq!(leaves[0].source(3), ScalarOffset::new(3));
        let BlockNode::Paragraph(paragraph) = &leaves[0].block else {
            panic!("paragraph")
        };
        assert!(
            paragraph
                .children()
                .iter()
                .flat_map(InlineNode::mark_runs)
                .any(|(_, mark)| mark.highlight == Some(theme.mark_background()))
        );
    }
}

#[test]
// Spec: docs/design/typography.md#字体角色
fn scripts_are_ordinary_editable_runs_with_documented_typography() {
    let document = Document::new(DocumentId::new(1), Epoch::new(0), "a ~中😀~ b ^上标^ end");
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(document.snapshot().text().len()),
        moino_theme::BuiltinTheme::Paper,
    );
    assert_eq!(leaves[0].text, "a 中😀 b 上标 end");
    let BlockNode::Paragraph(paragraph) = &leaves[0].block else {
        panic!("paragraph")
    };
    let sub = paragraph
        .children()
        .iter()
        .find(|run| run.text() == "中😀")
        .expect("subscript run");
    let sup = paragraph
        .children()
        .iter()
        .find(|run| run.text() == "上标")
        .expect("superscript run");
    assert_eq!(sub.font_size_scale(), Some(0.75));
    assert_eq!(sub.baseline_shift(), Some(0.2));
    assert_eq!(sup.font_size_scale(), Some(0.75));
    assert_eq!(sup.baseline_shift(), Some(-0.35));
    assert!(sub.as_custom().is_none() && sup.as_custom().is_none());
    assert_eq!(leaves[0].source(5).value(), 4);
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn nested_and_loose_list_rows_keep_one_marker_per_item() {
    for (source, expected) in [
        ("- a\n  - b\n  - c\n- d", vec!["a", "b", "c", "d"]),
        ("- [x] 中😀\n\n  tail", vec!["中😀", "tail"]),
        (
            "- a\n\n  continued\n\n  - child",
            vec!["a", "continued", "child"],
        ),
    ] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(document.snapshot().text().len()),
            moino_theme::BuiltinTheme::Paper,
        );
        assert_eq!(
            leaves
                .iter()
                .map(|leaf| leaf.text.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(leaves[0].prefix.is_empty());
        assert!(
            leaves[1]
                .prefix
                .iter()
                .any(|wrapper| matches!(wrapper, Wrapper::Indent))
        );
        if source.contains("continued") {
            assert!(matches!(leaves[1].block, BlockNode::Paragraph(_)));
        }
    }
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn container_prefixes_and_decoded_inline_syntax_do_not_leak_into_body_text() {
    for (source, expected) in [
        ("> first\n> second", "first\nsecond"),
        ("a &amp; b \\* c", "a & b * c"),
        ("`` a\nb `` end", "a b end"),
    ] {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), source);
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(document.snapshot().text().len()),
            moino_theme::BuiltinTheme::Paper,
        );
        assert_eq!(leaves[0].text, expected, "{source}");
        assert_eq!(document.snapshot().text().to_string(), source);
    }
    let document = Document::new(DocumentId::new(1), Epoch::new(0), "> - a\n>   - b");
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(document.snapshot().text().len()),
        moino_theme::BuiltinTheme::Paper,
    );
    assert!(matches!(
        leaves[1].prefix.as_slice(),
        [Wrapper::Quote, Wrapper::Indent]
    ));
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn task_projection_keeps_the_checkbox_state_and_hides_its_source_marker() {
    let document = Document::new(DocumentId::new(1), Epoch::new(0), "- [x] done\n- [ ] next");
    let leaves = project(
        &document.snapshot(),
        Selection::cursor(document.snapshot().text().len()),
        moino_theme::BuiltinTheme::Paper,
    );
    for (leaf, expected) in leaves.iter().zip([true, false]) {
        let BlockNode::List { children, .. } = &leaf.block else {
            panic!("list")
        };
        let BlockNode::ListItem { checked, .. } = &children[0] else {
            panic!("item")
        };
        assert_eq!(*checked, Some(expected));
        assert!(
            !leaf.text.contains('['),
            "text={:?} CST={:#?}",
            leaf.text,
            document.snapshot().cst().nodes()
        );
    }
    assert_eq!(leaves.len(), 2);
}
