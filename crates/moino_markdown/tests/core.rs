//! Public syntax invariants, dialect examples and formal acceptance data.
use moino_markdown::{Cst, Node, Syntax, heading_anchor, normalize_label};
use moino_text::{ScalarOffset, TextBuffer};
use proptest::prelude::*;
use std::{collections::HashSet, path::Path};

fn meaningful(cst: &Cst) -> Vec<&Node> {
    cst.nodes()
        .iter()
        .filter(|node| node.syntax() != &Syntax::Trivia)
        .collect()
}
fn validate(nodes: &[Node], parent: std::ops::Range<ScalarOffset>, ids: &mut HashSet<u64>) {
    for node in nodes {
        let raw = node.raw();
        let content = node.content();
        assert!(
            raw.start >= parent.start && raw.end <= parent.end && raw.start <= raw.end,
            "{node:?}"
        );
        assert!(
            content.start >= raw.start && content.end <= raw.end && content.start <= content.end,
            "{node:?}"
        );
        assert!(ids.insert(node.id().value()));
        validate(node.children(), raw, ids);
    }
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn executes_markdown_vectors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/acceptance/vectors");
    let vectors = moino_testkit::load_vectors(&root).expect("formal vectors");
    let mut count = 0;
    for vector in vectors.iter().filter(|vector| vector.action().is_none()) {
        let text = TextBuffer::new(vector.input());
        let cst = Cst::parse(&text);
        assert_eq!(cst.serialize(), vector.input(), "{}", vector.name());
        validate(
            cst.nodes(),
            ScalarOffset::new(0)..text.len(),
            &mut HashSet::new(),
        );
        let nodes = meaningful(&cst);
        let actual = match vector.name() {
            "paragraph" => {
                assert_eq!(nodes[0].syntax(), &Syntax::Paragraph);
                format!("Paragraph[{:?}]\n", nodes[0].text())
            }
            "atx-heading" => {
                assert_eq!(nodes[0].syntax(), &Syntax::Heading(1));
                format!(
                    "Heading(level=1, anchor={:?})[{:?}]\n",
                    nodes[0].anchor().expect("heading anchor"),
                    nodes[0].text()
                )
            }
            "setext-heading" => {
                assert_eq!(nodes[0].syntax(), &Syntax::Heading(1));
                format!("Heading(level=1)[{:?}]\n", nodes[0].text())
            }
            "fenced-code" => {
                let Syntax::CodeBlock(info) = nodes[0].syntax() else {
                    panic!("code block")
                };
                format!("Code(info={info:?})[{:?}]\n", nodes[0].text())
            }
            "thematic-break" => {
                assert_eq!(
                    nodes.iter().map(|node| node.syntax()).collect::<Vec<_>>(),
                    vec![&Syntax::Paragraph, &Syntax::Rule, &Syntax::Paragraph]
                );
                "Paragraph\nThematicBreak\nParagraph\n".to_owned()
            }
            "table" => {
                let Syntax::Table(columns) = nodes[0].syntax() else {
                    panic!("table")
                };
                format!(
                    "Table(cols={columns}, rows={})\n",
                    nodes[0]
                        .children()
                        .iter()
                        .filter(|node| node.syntax() == &Syntax::TableRow)
                        .count()
                )
            }
            "inline-strong" => {
                let children = nodes[0].children();
                assert_eq!(children.len(), 3);
                assert_eq!(children[1].syntax(), &Syntax::Strong);
                format!(
                    "Paragraph[Text({:?}), Strong, Text({:?})]\n",
                    children[0].text(),
                    children[2].text()
                )
            }
            "math-block" => {
                assert_eq!(nodes[0].syntax(), &Syntax::MathBlock);
                format!("MathBlock[{:?}]\n", nodes[0].text())
            }
            "front-matter" => {
                assert_eq!(nodes.len(), 2);
                assert_eq!(nodes[0].syntax(), &Syntax::FrontMatter);
                assert_eq!(nodes[1].syntax(), &Syntax::Paragraph);
                "FrontMatter\nParagraph\n".to_owned()
            }
            "unclosed-code-as-text" => {
                assert!(
                    nodes[0]
                        .children()
                        .iter()
                        .all(|node| node.syntax() == &Syntax::Text)
                );
                "Paragraph[Text]\n".to_owned()
            }
            name => panic!("unexecuted parsing vector {name}"),
        };
        assert_eq!(actual, vector.expected(), "{}", vector.name());
        count += 1;
    }
    assert_eq!(count, 10);
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#cst-与标记区间
fn exposes_unicode_markers_and_extensions_without_rewriting_text() {
    let text = TextBuffer::new(
        "# 墨\n\na **中** ==亮== ~低~ ^高^ $x$ [链](url)\n\n[toc]\n\n> [!CUSTOM]\n> body\n",
    );
    let cst = Cst::parse(&text);
    let nodes = meaningful(&cst);
    assert_eq!(nodes[0].anchor(), Some("墨"));
    for syntax in [
        Syntax::Strong,
        Syntax::Highlight,
        Syntax::Subscript,
        Syntax::Superscript,
        Syntax::Math,
        Syntax::Link("url".to_owned()),
    ] {
        let node = nodes[1]
            .children()
            .iter()
            .find(|node| node.syntax() == &syntax)
            .expect("extension node");
        assert!(
            !text
                .slice(node.opening_marker())
                .expect("marker")
                .is_empty()
        );
        assert!(
            !text
                .slice(node.closing_marker())
                .expect("marker")
                .is_empty()
        );
    }
    assert_eq!(nodes[2].syntax(), &Syntax::Toc);
    assert_eq!(nodes[3].syntax(), &Syntax::Callout("CUSTOM".to_owned()));
    assert_eq!(normalize_label("  A\t B \nC "), "a b c");
    assert_eq!(heading_anchor("Hello,  World!"), "hello-world");
    assert_eq!(cst.serialize(), text.to_string());
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn reparse_keeps_unchanged_block_ids_and_snapshot_isolation() {
    let old = Cst::parse(&TextBuffer::new("first\n\nsecond\n\nthird\n"));
    let new = old.reparse(&TextBuffer::new("first changed\n\nsecond\n\nthird\n"));
    assert_eq!(meaningful(&old)[1].id(), meaningful(&new)[1].id());
    assert_eq!(meaningful(&old)[2].id(), meaningful(&new)[2].id());
    assert_ne!(meaningful(&old)[0].id(), meaningful(&new)[0].id());
    assert_eq!(old.serialize(), "first\n\nsecond\n\nthird\n");
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn extensions_can_be_disabled_and_product_differences_are_explicit() {
    use moino_markdown::{Extension, Extensions};
    let source = "==**亮**== ~低~ ^高^ $x$ user@example.org https://example.org/a(b)). www.example.org\n\n[toc]\n";
    let text = TextBuffer::new(source);
    let cst = Cst::parse(&text);
    assert!(cst.is_valid());
    let nodes = meaningful(&cst);
    let highlight = nodes[0]
        .children()
        .iter()
        .find(|node| node.syntax() == &Syntax::Highlight)
        .expect("nested highlight");
    assert!(
        highlight
            .children()
            .iter()
            .any(|node| node.syntax() == &Syntax::Strong)
    );
    assert_eq!(highlight.text(), "亮");
    assert!(
        nodes[0]
            .children()
            .iter()
            .any(|node| node.syntax() == &Syntax::Link("mailto:user@example.org".to_owned()))
    );
    assert!(
        nodes[0]
            .children()
            .iter()
            .any(|node| node.syntax() == &Syntax::Link("https://example.org/a(b)".to_owned()))
    );
    assert!(
        !nodes[0]
            .children()
            .iter()
            .any(|node| matches!(node.syntax(), Syntax::Link(url) if url.contains("www.example")))
    );
    let plain = Cst::parse_with_extensions(&text, Extensions::commonmark());
    assert_eq!(meaningful(&plain)[1].syntax(), &Syntax::Paragraph);
    assert!(
        !meaningful(&plain)[0].children().iter().any(|node| matches!(
            node.syntax(),
            Syntax::Highlight
                | Syntax::Math
                | Syntax::Subscript
                | Syntax::Superscript
                | Syntax::Link(_)
        ))
    );
    assert!(
        Extensions::commonmark()
            .with(Extension::Highlight, true)
            .is_enabled(Extension::Highlight)
    );
    let table = Cst::parse(&TextBuffer::new("| a | b |\n|---|---|\n"));
    assert_eq!(meaningful(&table)[0].syntax(), &Syntax::Paragraph);
    assert!(table.is_valid());
    let math = Cst::parse(&TextBuffer::new("$a\nb$\n\n$$x$$\n"));
    assert_eq!(meaningful(&math)[0].text(), "$a\nb$");
    assert_eq!(meaningful(&math)[1].text(), "$$x$$");
    assert_eq!(normalize_label("Straße"), normalize_label("STRASSE"));
    let headings = Cst::parse(&TextBuffer::new("# a\n# a 1\n# a\n# e\u{301}😀!\n"));
    let anchors: HashSet<_> = headings.nodes().iter().filter_map(Node::anchor).collect();
    assert_eq!(anchors.len(), 4);
    assert!(anchors.contains("e\u{301}😀"));
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#锚点与引用
fn reference_definitions_are_source_ordered_and_first_definition_wins() {
    let source = "[link][ A B ]\n\n[A   B]: /one \"title\"\n[z]: /two\n[a b]: /ignored\n";
    let text = TextBuffer::new(source);
    let cst = Cst::parse(&text);
    assert!(cst.is_valid());
    let definition = cst.reference("a\tb").expect("normalized label");
    assert_eq!(definition.label(), "a b");
    assert_eq!(definition.destination(), "/one");
    assert_eq!(definition.title(), Some("title"));
    assert!(
        text.slice(definition.raw())
            .expect("raw definition")
            .contains("/one")
    );
    assert_eq!(cst.references().len(), 2);
    assert!(
        cst.references()
            .windows(2)
            .all(|pair| pair[0].raw().start < pair[1].raw().start)
    );
    assert!(cst.reference("missing").is_none());
    assert_eq!(
        meaningful(&cst)[0].children()[0].syntax(),
        &Syntax::Link("/one".to_owned())
    );
    let next = TextBuffer::new(&source.replace("/one", "/changed"));
    let new = cst.reparse(&next);
    assert_eq!(
        new.reference("a b")
            .expect("updated definition")
            .destination(),
        "/changed"
    );
    assert_eq!(
        projection(new.nodes()),
        projection(Cst::parse(&next).nodes())
    );
    assert_eq!(cst.serialize(), source);
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#cst-与标记区间
fn empty_constructs_expose_empty_content_instead_of_their_delimiters() {
    for source in [
        "# ", "# ##", "```\n```", "> ", "- ", "[](url)", "![](url)", "---", "[toc]",
    ] {
        let text = TextBuffer::new(source);
        let cst = Cst::parse(&text);
        assert!(cst.is_valid(), "{source}");
        let mut pending: Vec<_> = cst.nodes().iter().collect();
        while let Some(node) = pending.pop() {
            if matches!(
                node.syntax(),
                Syntax::Heading(_)
                    | Syntax::CodeBlock(_)
                    | Syntax::Quote
                    | Syntax::Item
                    | Syntax::Link(_)
                    | Syntax::Image(_)
                    | Syntax::Rule
                    | Syntax::Toc
            ) && node.text().is_empty()
            {
                assert_eq!(
                    text.slice(node.content()).expect("empty content"),
                    "",
                    "{source}"
                );
            }
            pending.extend(node.children());
        }
    }
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn middle_edits_fences_references_and_duplicate_blocks_match_full_parse() {
    let source = (0..80)
        .map(|i| format!("paragraph {i}\n\n"))
        .collect::<String>();
    let text = TextBuffer::new(&source);
    let old = Cst::parse(&text);
    let next = TextBuffer::new(&source.replace("paragraph 40", "changed 40"));
    let incremental = old.reparse(&next);
    assert_eq!(
        projection(incremental.nodes()),
        projection(Cst::parse(&next).nodes())
    );
    for index in [0, 10, 70, 79] {
        assert_eq!(
            meaningful(&old)[index].id(),
            meaningful(&incremental)[index].id()
        );
    }
    for (before, after) in [
        ("a\n\nb\n\nc\n\nd\n\ne\n", "a\n\n```\n\nc\n\nd\n\ne\n"),
        ("[a][x]\n\n[x]: one\n", "[a][x]\n\n[x]: two\n"),
        ("same\n\nsame\n", "same\n\nsame\n\nsame\n"),
        ("\t(\n\n~", "\t(\n\n~a"),
    ] {
        let old = Cst::parse(&TextBuffer::new(before));
        let text = TextBuffer::new(after);
        let new = old.reparse(&text);
        assert!(new.is_valid());
        assert_eq!(
            projection(new.nodes()),
            projection(Cst::parse(&text).nodes())
        );
    }
}
fn projection(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| {
            format!(
                "{:?}:{:?}:{:?}:{:?}:{:?}({})",
                node.syntax(),
                node.raw(),
                node.content(),
                node.text(),
                node.anchor(),
                projection(node.children())
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}
proptest! {
    #[test]
    fn arbitrary_unicode_is_lossless(source in "[a-z中😀#*_~^$=\\[\\]()`>!|: \n\t-]{0,200}", replacement in ".{0,40}", first in any::<usize>(), last in any::<usize>()) {
        let text = TextBuffer::new(&source); let old = Cst::parse(&text);
        prop_assert!(old.is_valid());
        prop_assert_eq!(old.serialize(), text.to_string());
        validate(old.nodes(), ScalarOffset::new(0)..text.len(), &mut HashSet::new());
        let mut next = text.clone();
        let first = first % (text.len().value() + 1); let last = last % (text.len().value() + 1);
        next.replace(ScalarOffset::new(first.min(last))..ScalarOffset::new(first.max(last)), &replacement).expect("valid edit");
        let incremental = old.reparse(&next); let full = Cst::parse(&next);
        prop_assert_eq!(incremental.serialize(), full.serialize());
        prop_assert_eq!(projection(incremental.nodes()), projection(full.nodes()));
        let hint = moino_markdown::TextChange::new(ScalarOffset::new(first.min(last))..ScalarOffset::new(first.max(last)), ScalarOffset::new(moino_text::normalize_newlines(&replacement).chars().count()));
        let explicit = old.reparse_with_changes(&next, &[hint]).expect("actual edit window");
        prop_assert!(explicit.is_valid());
        prop_assert_eq!(projection(explicit.nodes()), projection(full.nodes()));
        validate(incremental.nodes(), ScalarOffset::new(0)..next.len(), &mut HashSet::new());
    }
    #[test]
    fn local_edits_in_many_blocks_match_full_parse(parts in prop::collection::vec(0u8..7, 10..35), at in any::<usize>(), delete in 0usize..6, inserted in "[a中#*`$> \n]{0,12}") {
        let source = parts.into_iter().enumerate().map(|(index, syntax)| match syntax {
            0 => format!("paragraph {index}"),
            1 => format!("# title {index}"),
            2 => format!("- item {index}\n- sibling"),
            3 => format!("> quote {index}"),
            4 => format!("```\ncode {index}\n```"),
            5 => "[a]: /url\n\n[ref][a]".to_owned(),
            _ => "$$\nx^2\n$$".to_owned(),
        }).collect::<Vec<_>>().join("\n\n");
        let text = TextBuffer::new(&source); let old = Cst::parse(&text);
        let start = at % (text.len().value()+1); let end = (start+delete).min(text.len().value());
        let mut next = text.clone(); next.replace(ScalarOffset::new(start)..ScalarOffset::new(end), &inserted).expect("valid local edit");
        let change = moino_markdown::TextChange::new(ScalarOffset::new(start)..ScalarOffset::new(end), ScalarOffset::new(inserted.chars().count()));
        let incremental = old.reparse_with_changes(&next, &[change]).expect("exact change");
        prop_assert!(incremental.is_valid());
        prop_assert_eq!(projection(incremental.nodes()), projection(Cst::parse(&next).nodes()));
    }
}
#[test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn rejects_inconsistent_change_hints_without_touching_the_old_snapshot() {
    use moino_markdown::TextChange;
    let old = Cst::parse(&TextBuffer::new("abc"));
    let next = TextBuffer::new("axc");
    for hints in [
        vec![],
        vec![TextChange::new(
            ScalarOffset::new(2)..ScalarOffset::new(1),
            ScalarOffset::new(1),
        )],
        vec![TextChange::new(
            ScalarOffset::new(9)..ScalarOffset::new(9),
            ScalarOffset::new(1),
        )],
        vec![TextChange::new(
            ScalarOffset::new(0)..ScalarOffset::new(0),
            ScalarOffset::new(usize::MAX),
        )],
    ] {
        assert!(old.reparse_with_changes(&next, &hints).is_none());
    }
    assert_eq!(old.serialize(), "abc");
}
