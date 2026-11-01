//! Syntax ranges are projected from the authoritative CST, never reparsed by Kit.
use gpui_kit::component::input::{
    EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter, Rope,
};
use gpui_kit::{Context, HighlightStyle, SharedString, Window};
use moino_document::Snapshot;
use moino_markdown::{Node, Syntax};
use std::ops::Range;

pub(super) struct SourceHighlighter {
    runs: Vec<(Range<usize>, &'static str)>,
    len: usize,
    valid: bool,
}
impl SourceHighlighter {
    pub fn new(snapshot: &Snapshot) -> Self {
        fn collect(
            nodes: &[Node],
            snapshot: &Snapshot,
            runs: &mut Vec<(Range<usize>, &'static str)>,
        ) {
            for node in nodes {
                let role = match node.syntax() {
                    Syntax::Heading(_) => "title",
                    Syntax::Strong => "emphasis.strong",
                    Syntax::Emphasis => "emphasis",
                    Syntax::Code | Syntax::CodeBlock(_) => "text.literal",
                    Syntax::Link(_) => "link_text",
                    _ => "",
                };
                if !role.is_empty() {
                    let raw = node.raw();
                    let start = snapshot
                        .text()
                        .to_byte(raw.start)
                        .expect("CST boundary")
                        .value();
                    let end = snapshot
                        .text()
                        .to_byte(raw.end)
                        .expect("CST boundary")
                        .value();
                    runs.push((start..end, role));
                }
                collect(node.children(), snapshot, runs);
            }
        }
        let mut runs = Vec::new();
        collect(snapshot.cst().nodes(), snapshot, &mut runs);
        Self {
            runs,
            len: snapshot.text().to_string().len(),
            valid: true,
        }
    }
}
impl InputHighlighter for SourceHighlighter {
    fn language(&self) -> SharedString {
        "moino-markdown".into()
    }
    fn update(
        &mut self,
        edit: Option<InputEdit>,
        text: &Rope,
        _: bool,
        _: &mut Window,
        _: &mut Context<EditorState>,
    ) {
        self.valid = edit.is_none() && text.len() == self.len;
    }
    fn styles(
        &self,
        range: &Range<usize>,
        resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        if !self.valid {
            return vec![(range.clone(), HighlightStyle::default())];
        }
        let mut edges = vec![range.start, range.end];
        for (run, _) in &self.runs {
            if range.start < run.end && run.start < range.end {
                edges.push(run.start.max(range.start));
                edges.push(run.end.min(range.end));
            }
        }
        edges.sort_unstable();
        edges.dedup();
        edges
            .windows(2)
            .map(|edge| {
                let style = self
                    .runs
                    .iter()
                    .rev()
                    .find(|(run, _)| run.start <= edge[0] && edge[1] <= run.end)
                    .and_then(|(_, role)| resolver.style(role))
                    .unwrap_or_default();
                (edge[0]..edge[1], style)
            })
            .collect()
    }
    fn fold_ranges(&self, _: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}
