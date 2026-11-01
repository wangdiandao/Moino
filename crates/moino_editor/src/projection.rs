//! CST projection and explicit source-to-leaf coordinate maps.
use gpui_kit::Hsla;
use gpui_kit::base::text::editor_host::{BlockNode, InlineNode, LinkMark, Paragraph, TextMark};
use moino_document::{Affinity, Selection, Snapshot};
use moino_markdown::{Node, Syntax};
use moino_text::ScalarOffset;
use std::ops::Range;

pub(super) struct Leaf {
    pub key: u64,
    pub block: BlockNode,
    pub text: String,
    pub raw: Range<ScalarOffset>,
    pub heading_badge: Option<u8>,
    boundaries: Vec<(usize, ScalarOffset)>,
}

impl Leaf {
    pub fn plain(text: &str, marked: Option<Range<usize>>) -> Self {
        let mut paragraph = Paragraph::default().with_leaf_key(0);
        paragraph.push(
            InlineNode::new(if text.is_empty() { " " } else { text }).marks(
                marked
                    .map(|range| (range, TextMark::default().underline()))
                    .into_iter()
                    .collect(),
            ),
        );
        let mut boundaries: Vec<_> = text
            .char_indices()
            .enumerate()
            .map(|(scalar, (byte, _))| (byte, ScalarOffset::new(scalar)))
            .collect();
        let end = ScalarOffset::new(text.chars().count());
        boundaries.push((text.len(), end));
        Self {
            key: 0,
            block: BlockNode::Paragraph(paragraph),
            text: text.into(),
            raw: ScalarOffset::new(0)..end,
            heading_badge: None,
            boundaries,
        }
    }
    pub fn source(&self, byte: usize) -> ScalarOffset {
        let byte = byte.min(self.text.len());
        self.boundaries
            .iter()
            .rev()
            .find(|(at, _)| *at <= byte)
            .map_or(self.raw.start, |(_, offset)| *offset)
    }

    pub fn byte(&self, offset: ScalarOffset, affinity: Affinity) -> usize {
        match affinity {
            Affinity::Before => self.boundaries.iter().find(|(_, at)| *at >= offset),
            Affinity::After => self.boundaries.iter().rev().find(|(_, at)| *at <= offset),
        }
        .map_or(0, |(byte, _)| *byte)
    }
}

struct Builder<'a> {
    source: &'a [char],
    text: String,
    boundaries: Vec<(usize, ScalarOffset)>,
    marks: Vec<(Range<usize>, TextMark)>,
    selection: Selection,
    markers: Vec<Range<usize>>,
}

impl Builder<'_> {
    fn marker(&mut self, range: Range<ScalarOffset>) {
        if range.is_empty() {
            return;
        }
        let start = self
            .boundaries
            .iter()
            .find(|(_, at)| *at == range.start)
            .map(|(byte, _)| *byte);
        let end = self
            .boundaries
            .iter()
            .find(|(_, at)| *at == range.end)
            .map(|(byte, _)| *byte);
        if let (Some(start), Some(end)) = (start, end)
            && start < end
        {
            self.markers.push(start..end);
        }
    }
    fn node_markers(&mut self, node: &Node) {
        self.marker(node.opening_marker());
        self.marker(node.closing_marker());
        for child in node.children() {
            self.node_markers(child);
        }
    }
    fn append(&mut self, range: Range<ScalarOffset>) {
        for at in range.start.value()..range.end.value() {
            self.boundaries
                .push((self.text.len(), ScalarOffset::new(at)));
            self.text.push(self.source[at]);
        }
        self.boundaries.push((self.text.len(), range.end));
    }

    fn expanded(&self, range: Range<ScalarOffset>) -> bool {
        let selection = self.selection.range();
        if self.selection.is_collapsed() {
            range.start <= selection.start && selection.start <= range.end
        } else {
            range.start < selection.end && selection.start < range.end
        }
    }

    fn inline(&mut self, node: &Node) {
        let raw = node.raw();
        let content = node.content();
        let supported = matches!(
            node.syntax(),
            Syntax::Emphasis | Syntax::Strong | Syntax::Strike | Syntax::Code | Syntax::Link(_)
        );
        if !supported || self.expanded(raw.clone()) {
            if node.children().is_empty() || supported {
                self.append(raw);
                if supported {
                    self.node_markers(node);
                }
            } else {
                self.children(node.children(), raw);
            }
            return;
        }
        let start = self.text.len();
        if node.children().is_empty() {
            self.append(content);
        } else {
            self.children(node.children(), content);
        }
        let mark = match node.syntax() {
            Syntax::Strong => TextMark::default().bold(),
            Syntax::Emphasis => TextMark::default().italic(),
            Syntax::Strike => TextMark::default().strikethrough(),
            Syntax::Code => TextMark::default().code(),
            // Links are styled without granting document content URL activation.
            Syntax::Link(url) => TextMark::default().link(LinkMark {
                url: url.clone().into(),
                ..Default::default()
            }),
            _ => TextMark::default(),
        };
        self.marks.push((start..self.text.len(), mark));
    }

    fn children(&mut self, nodes: &[Node], range: Range<ScalarOffset>) {
        let mut at = range.start;
        for node in nodes {
            let raw = node.raw();
            if raw.start < range.start || raw.end > range.end {
                continue;
            }
            if at < raw.start {
                self.append(at..raw.start);
            }
            self.inline(node);
            at = raw.end;
        }
        if at < range.end {
            self.append(at..range.end);
        }
    }
}

pub(super) fn project(snapshot: &Snapshot, selection: Selection, marker_color: Hsla) -> Vec<Leaf> {
    let source: Vec<char> = snapshot.text().to_string().chars().collect();
    let mut leaves = Vec::new();
    fn visit<'a>(
        nodes: &'a [Node],
        wrappers: &[Wrapper],
        output: &mut Vec<(&'a Node, Vec<Wrapper>)>,
    ) {
        for node in nodes {
            match node.syntax() {
                Syntax::Quote | Syntax::Callout(_) => {
                    let mut wrappers = wrappers.to_vec();
                    wrappers.push(Wrapper::Quote);
                    visit(node.children(), &wrappers, output);
                }
                Syntax::List(start) => {
                    for (ix, item) in node.children().iter().enumerate() {
                        let mut wrappers = wrappers.to_vec();
                        wrappers.push(Wrapper::List(
                            start.and_then(|start| start.checked_add(u64::try_from(ix).ok()?)),
                        ));
                        visit(item.children(), &wrappers, output);
                    }
                }
                Syntax::Item => visit(node.children(), wrappers, output),
                _ => output.push((node, wrappers.to_vec())),
            }
        }
    }
    let mut rows = Vec::new();
    visit(snapshot.cst().nodes(), &[], &mut rows);
    for (node, wrappers) in rows {
        let raw = node.raw();
        if raw.is_empty() {
            continue;
        }
        let mut builder = Builder {
            source: &source,
            text: String::new(),
            boundaries: Vec::new(),
            marks: Vec::new(),
            markers: Vec::new(),
            selection,
        };
        let heading = match node.syntax() {
            Syntax::Heading(level) if !builder.expanded(raw.clone()) => Some(*level),
            _ => None,
        };
        let range = if heading.is_some() {
            node.content()
        } else {
            raw.clone()
        };
        // Only ordinary leaves are interpreted here; rich blocks remain editable source until M4.
        if matches!(node.syntax(), Syntax::Paragraph | Syntax::Heading(_)) {
            builder.children(node.children(), range);
        } else {
            builder.append(range);
        }
        if matches!(node.syntax(), Syntax::Heading(_)) && heading.is_none() {
            builder.marker(node.opening_marker());
            builder.marker(node.closing_marker());
        }
        while builder.text.ends_with('\n') {
            builder.text.pop();
            builder
                .boundaries
                .retain(|(byte, _)| *byte <= builder.text.len());
        }
        let mut paragraph = Paragraph::default().with_leaf_key(node.id().value());
        let mut edges = vec![0, builder.text.len()];
        for marker in &builder.markers {
            edges.extend([
                marker.start.min(builder.text.len()),
                marker.end.min(builder.text.len()),
            ]);
        }
        edges.sort_unstable();
        edges.dedup();
        for edge in edges.windows(2) {
            let marks = builder
                .marks
                .iter()
                .filter_map(|(range, mark)| {
                    let start = range.start.max(edge[0]);
                    let end = range.end.min(edge[1]);
                    (start < end).then_some((start - edge[0]..end - edge[0], mark.clone()))
                })
                .collect();
            let inline = InlineNode::new(builder.text[edge[0]..edge[1]].to_owned()).marks(marks);
            paragraph.push(
                if builder
                    .markers
                    .iter()
                    .any(|range| range.start <= edge[0] && edge[1] <= range.end)
                {
                    inline.with_foreground(marker_color)
                } else {
                    inline
                },
            );
        }
        if builder.text.is_empty() {
            paragraph.push(InlineNode::new(" "));
        }
        let mut block = if let Some(level) = heading {
            BlockNode::Heading {
                level,
                children: paragraph,
                span: None,
            }
        } else {
            BlockNode::Paragraph(paragraph)
        };
        for wrapper in wrappers.into_iter().rev() {
            block = match wrapper {
                Wrapper::Quote => BlockNode::Blockquote {
                    children: vec![block],
                    span: None,
                },
                Wrapper::List(start) => BlockNode::List {
                    ordered: start.is_some(),
                    start: start.and_then(|value| u32::try_from(value).ok()),
                    span: None,
                    children: vec![BlockNode::ListItem {
                        children: vec![block],
                        spread: false,
                        checked: None,
                        span: None,
                    }],
                },
            };
        }
        leaves.push(Leaf {
            key: node.id().value(),
            block,
            text: builder.text,
            raw,
            heading_badge: match node.syntax() {
                Syntax::Heading(level) if heading.is_none() => Some(*level),
                _ => None,
            },
            boundaries: builder.boundaries,
        });
    }
    if leaves.is_empty() || source.last() == Some(&'\n') {
        let at = snapshot.text().len();
        let key = u64::MAX;
        let mut paragraph = Paragraph::default().with_leaf_key(key);
        paragraph.push(InlineNode::new(" "));
        leaves.push(Leaf {
            key,
            block: BlockNode::Paragraph(paragraph),
            text: String::new(),
            raw: at..at,
            heading_badge: None,
            boundaries: vec![(0, at)],
        });
    }
    leaves
}

#[derive(Clone)]
enum Wrapper {
    Quote,
    List(Option<u64>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use moino_document::{Document, DocumentId, Epoch};

    #[test]
    // Spec: docs/modules/selection.md#标记展开与折叠
    fn collapsed_markers_map_unicode_back_to_source() {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), "**中😀** end");
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(ScalarOffset::new(10)),
            moino_theme::BuiltinTheme::Paper.marker(),
        );
        assert_eq!(leaves[0].text, "中😀 end");
        assert_eq!(leaves[0].source(3), ScalarOffset::new(3));
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(ScalarOffset::new(3)),
            moino_theme::BuiltinTheme::Paper.marker(),
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
}
