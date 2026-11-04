//! Fixed regressions for contextual block-window convergence.
use moino_markdown::{Cst, Node, TextChange};
use moino_text::{ScalarOffset, TextBuffer};

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
fn ci_contextual_boundary_regressions() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/incremental-boundaries.json"))
            .expect("CI fixtures");
    for case in cases.as_array().expect("cases") {
        let text = TextBuffer::new(case["source"].as_str().expect("source"));
        let old = Cst::parse(&text);
        let start = ScalarOffset::new(
            usize::try_from(case["start"].as_u64().expect("start")).expect("offset"),
        );
        let end =
            ScalarOffset::new(usize::try_from(case["end"].as_u64().expect("end")).expect("offset"));
        let replacement = case["replacement"].as_str().expect("replacement");
        let mut changed = text.clone();
        changed
            .replace(start..end, replacement)
            .expect("fixture edit");
        let full = Cst::parse(&changed);
        let inferred = old.reparse(&changed);
        let explicit = old
            .reparse_with_changes(
                &changed,
                &[TextChange::new(
                    start..end,
                    ScalarOffset::new(moino_text::normalize_newlines(replacement).chars().count()),
                )],
            )
            .expect("actual changes");
        assert_eq!(semantic(inferred.nodes()), semantic(full.nodes()));
        assert_eq!(semantic(explicit.nodes()), semantic(full.nodes()));
        assert!(inferred.is_valid() && explicit.is_valid());
    }
}

#[test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn convergence_requires_following_text_and_complete_container_shape() {
    for (before, after) in [
        ("#=\n\t=\n (", "=\t=\n ("),
        ("~\n\n|\n-\ta\n\n a", "\n|\n-\ta\n\n a"),
        ("`#a`\n#b\n\tc\nx`", "#a`\n#b\n\tc\nx`"),
    ] {
        let old = Cst::parse(&TextBuffer::new(before));
        let text = TextBuffer::new(after);
        let incremental = old.reparse(&text);
        let full = Cst::parse(&text);
        assert_eq!(
            semantic(incremental.nodes()),
            semantic(full.nodes()),
            "{before:?} -> {after:?}"
        );
        assert!(incremental.is_valid());
    }
}
