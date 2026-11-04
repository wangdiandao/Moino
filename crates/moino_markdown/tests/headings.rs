//! Product heading grammar and its lossless source projection.
use moino_markdown::{Cst, Extension, Extensions, Node, Syntax};
use moino_text::TextBuffer;
use proptest::prelude::*;

fn headings(nodes: &[Node]) -> Vec<&Node> {
    nodes
        .iter()
        .flat_map(|node| {
            let mut found = Vec::new();
            if matches!(node.syntax(), Syntax::Heading(_)) {
                found.push(node);
            }
            found.extend(headings(node.children()));
            found
        })
        .collect()
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn atx_grammar_preserves_exact_body_and_delimiter_ranges() {
    for (source, level, body) in [
        ("#hello", 1, "hello"),
        ("##", 1, "#"),
        ("###", 2, "#"),
        ("# ", 1, " "),
        ("##  ", 2, " "),
        ("####### hi", 6, "# hi"),
        ("# a  ", 1, "a  "),
        ("# a ##", 1, "a"),
        ("# a ## ## ", 1, "a"),
        ("# a#", 1, "a#"),
        ("# a ## x", 1, "a ## x"),
        ("#中😀", 1, "中😀"),
    ] {
        let text = TextBuffer::new(source);
        let cst = Cst::parse(&text);
        assert!(cst.is_valid(), "{source}: {:#?}", cst.nodes());
        assert_eq!(cst.serialize(), source);
        let nodes = headings(cst.nodes());
        assert_eq!(nodes.len(), 1, "{source}: {:#?}", cst.nodes());
        assert_eq!(nodes[0].syntax(), &Syntax::Heading(level), "{source}");
        assert_eq!(nodes[0].text(), body, "{source}");
        assert_eq!(
            text.slice(nodes[0].content()).expect("heading body range"),
            body,
            "{source}"
        );
    }
    let cst = Cst::parse(&TextBuffer::new("#"));
    assert!(headings(cst.nodes()).is_empty());
    assert_eq!(cst.nodes()[0].text(), "#");
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn inline_syntax_and_global_references_are_retained() {
    let source = "#_中_ **粗** `# code` [link][id]  \n\n[id]: /target";
    let cst = Cst::parse(&TextBuffer::new(source));
    assert!(cst.is_valid(), "{:#?}", cst.nodes());
    let heading = headings(cst.nodes())[0];
    assert_eq!(heading.text(), "中 粗 # code link  ");
    for syntax in [
        Syntax::Emphasis,
        Syntax::Strong,
        Syntax::Code,
        Syntax::Link("/target".into()),
    ] {
        assert!(
            heading
                .children()
                .iter()
                .any(|node| node.syntax() == &syntax),
            "{syntax:?}: {heading:?}"
        );
    }
    for source in ["#&nbsp;", "#a&nbsp;&nbsp;", "#a\u{a0}\u{a0}"] {
        let cst = Cst::parse(&TextBuffer::new(source));
        let heading = headings(cst.nodes())[0];
        let expected = source.trim_start_matches('#').replace("&nbsp;", "\u{a0}");
        assert_eq!(heading.text(), expected, "{source}");
        assert!(cst.is_valid());
    }
    let cst = Cst::parse(&TextBuffer::new("#heading\n[id]: /target\n\n[id]"));
    assert!(cst.reference("id").is_some(), "{:#?}", cst.nodes());
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn setext_only_promotes_the_last_content_line_and_requires_two_markers() {
    for source in [
        "before\n中😀\n==\n",
        "> before\n> 中😀\n> ==\n",
        "- before\n  中😀\n  ==\n",
    ] {
        let cst = Cst::parse(&TextBuffer::new(source));
        assert!(cst.is_valid(), "{source}: {:#?}", cst.nodes());
        assert_eq!(cst.serialize(), source);
        assert_eq!(headings(cst.nodes()).len(), 1, "{source}");
        assert_eq!(headings(cst.nodes())[0].text(), "中😀", "{source}");
    }
    for source in ["title\n=", "title\n-", "> title\n> ="] {
        let cst = Cst::parse(&TextBuffer::new(source));
        assert!(cst.is_valid(), "{source}: {:#?}", cst.nodes());
        assert!(headings(cst.nodes()).is_empty(), "{source}");
        assert_eq!(cst.serialize(), source);
    }
    for (source, level) in [("title\n=-", 2), ("title\n-=", 1), ("> title\n> =-", 2)] {
        let cst = Cst::parse(&TextBuffer::new(source));
        assert!(cst.is_valid(), "{source}: {:#?}", cst.nodes());
        let found = headings(cst.nodes());
        assert_eq!(found.len(), 1, "{source}: {:#?}", cst.nodes());
        assert_eq!(found[0].syntax(), &Syntax::Heading(level));
        assert_eq!(found[0].text(), "title");
    }
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn containers_and_literal_blocks_keep_their_ownership() {
    for source in [
        "> #中\n> tail",
        "- #中\n- #文",
        "> - #中\n>   tail",
        "plain\n#中\n#文",
        "a\n=-\n==",
    ] {
        let cst = Cst::parse(&TextBuffer::new(source));
        assert!(cst.is_valid(), "{source}: {:#?}", cst.nodes());
        assert_eq!(cst.serialize(), source);
        assert!(
            !headings(cst.nodes()).is_empty(),
            "{source}: {:#?}",
            cst.nodes()
        );
    }
    for source in [
        "```\n#中\n```",
        "    #中",
        "<div>\n#中\n</div>",
        "`a\n#中\nb`",
        "\\#中",
    ] {
        let cst = Cst::parse(&TextBuffer::new(source));
        assert!(headings(cst.nodes()).is_empty(), "{source}");
    }
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#基础方言与扩展
fn commonmark_configuration_keeps_standard_headings() {
    for extensions in [
        Extensions::commonmark(),
        Extensions::default().with(Extension::LooseHeadings, false),
    ] {
        let text = TextBuffer::new("#\n\n#no-space\n\ntext\n=");
        let cst = Cst::parse_with_extensions(&text, extensions);
        let nodes = headings(cst.nodes());
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].text(), "");
        assert_eq!(nodes[1].text(), "text");
    }
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#cst-与标记区间
fn task_setext_keeps_checkbox_outside_heading_content() {
    let text = TextBuffer::new("- [x] Title\n  ===\n");
    let cst = Cst::parse(&text);
    assert!(cst.is_valid(), "{:#?}", cst.nodes());
    let found = headings(cst.nodes());
    assert_eq!(found.len(), 1, "{:#?}", cst.nodes());
    assert_eq!(found[0].raw().start.value(), 6);
    assert_eq!(found[0].content().start.value(), 6);
    assert_eq!(found[0].text(), "Title");
}

fn semantic(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| {
            format!(
                "{:?}:{:?}:{:?}:{:?}({})",
                node.syntax(),
                node.raw(),
                node.content(),
                node.text(),
                semantic(node.children())
            )
        })
        .collect()
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn heading_edits_match_full_parsing_at_every_scalar_boundary() {
    for source in [
        "lead\n\n#中😀\n\ntail",
        "- [x] Title\n  ===\n- tail",
        "> before\n> text\n> ==\n\ntail",
        "a\n#b\nc\n==\n\nend",
    ] {
        let old = Cst::parse(&TextBuffer::new(source));
        for byte in source
            .char_indices()
            .map(|(byte, _)| byte)
            .chain([source.len()])
        {
            for inserted in ["#", " ", "\n", "=", "中😀"] {
                let mut changed = source.to_owned();
                changed.insert_str(byte, inserted);
                let text = TextBuffer::new(&changed);
                let full = Cst::parse(&text);
                let incremental = old.reparse(&text);
                assert!(full.is_valid(), "{changed:?}: {:#?}", full.nodes());
                assert_eq!(
                    semantic(incremental.nodes()),
                    semantic(full.nodes()),
                    "{changed:?}"
                );
            }
        }
    }
}

proptest! {
    #[test]
    // Spec: docs/contracts/markdown-dialect.md#cst-与标记区间
    fn heading_grammar_never_loses_source_or_range_ownership(
        fragments in prop::collection::vec(prop::sample::select(vec!["#", "##", "#######", "中😀", " ", "\t", "\n", "==", "-", "> ", "- [x] ", "**", "_", "`", "[a]", "\u{a0}"]), 0..35)
    ) {
        let source = fragments.concat();
        let text = TextBuffer::new(&source);
        let cst = Cst::parse(&text);
        prop_assert!(cst.is_valid(), "{source:?}: {:#?}", cst.nodes());
        prop_assert_eq!(cst.serialize(), text.to_string());
    }
}
