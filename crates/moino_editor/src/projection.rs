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
    pub prefix: Vec<Wrapper>,
    pub rule: bool,
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
            prefix: Vec::new(),
            rule: false,
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
    mark_background: Hsla,
    scripts: Vec<(Range<usize>, bool)>,
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

    fn decoded(&mut self, range: Range<ScalarOffset>, text: &str) {
        let mut at = range.start.value();
        for ch in text.chars() {
            let matched = self.source[at..range.end.value()]
                .iter()
                .position(|source| *source == ch);
            let source = at + matched.unwrap_or(0);
            self.boundaries
                .push((self.text.len(), ScalarOffset::new(source)));
            self.text.push(ch);
            at = (source + 1).min(range.end.value());
        }
        self.boundaries.push((self.text.len(), range.end));
    }

    fn code(&mut self, mut range: Range<ScalarOffset>) {
        let spaces = |ch: &char| matches!(ch, ' ' | '\n');
        let raw = &self.source[range.start.value()..range.end.value()];
        if raw.first().is_some_and(spaces)
            && raw.last().is_some_and(spaces)
            && !raw.iter().all(spaces)
        {
            range.start = ScalarOffset::new(range.start.value() + 1);
            range.end = ScalarOffset::new(range.end.value() - 1);
        }
        for at in range.start.value()..range.end.value() {
            self.boundaries
                .push((self.text.len(), ScalarOffset::new(at)));
            self.text.push(if self.source[at] == '\n' {
                ' '
            } else {
                self.source[at]
            });
        }
        self.boundaries.push((self.text.len(), range.end));
    }

    fn inline(&mut self, node: &Node) {
        let raw = node.raw();
        let content = node.content();
        if matches!(node.syntax(), Syntax::Task(_)) {
            return;
        }
        if matches!(
            node.syntax(),
            Syntax::Text | Syntax::SoftBreak | Syntax::HardBreak
        ) {
            if matches!(node.syntax(), Syntax::Text) && self.expanded(raw.clone()) {
                self.append(raw);
            } else {
                self.decoded(raw, node.text());
            }
            return;
        }
        let supported = matches!(
            node.syntax(),
            Syntax::Emphasis
                | Syntax::Strong
                | Syntax::Strike
                | Syntax::Code
                | Syntax::Link(_)
                | Syntax::Highlight
                | Syntax::Subscript
                | Syntax::Superscript
        );
        if !supported {
            self.append(raw);
            return;
        }
        if self.expanded(raw.clone()) {
            if node.children().is_empty() {
                self.append(raw);
            } else {
                self.append(node.opening_marker());
                self.children(node.children(), content);
                self.append(node.closing_marker());
            }
            self.marker(node.opening_marker());
            self.marker(node.closing_marker());
            return;
        }
        let start = self.text.len();
        if node.children().is_empty() {
            if matches!(node.syntax(), Syntax::Code) {
                self.code(content);
            } else {
                self.append(content);
            }
        } else {
            self.children(node.children(), content);
        }
        let mark = match node.syntax() {
            Syntax::Strong => TextMark::default().bold(),
            Syntax::Emphasis => TextMark::default().italic(),
            Syntax::Strike => TextMark::default().strikethrough(),
            Syntax::Code => TextMark::default().code(),
            Syntax::Highlight => TextMark::default().highlight(self.mark_background),
            // Links are styled without granting document content URL activation.
            Syntax::Link(url) => TextMark::default().link(LinkMark {
                url: url.clone().into(),
                ..Default::default()
            }),
            _ => TextMark::default(),
        };
        if matches!(node.syntax(), Syntax::Subscript | Syntax::Superscript) {
            self.scripts.push((
                start..self.text.len(),
                matches!(node.syntax(), Syntax::Superscript),
            ));
        }
        self.marks.push((start..self.text.len(), mark));
    }

    fn children(&mut self, nodes: &[Node], range: Range<ScalarOffset>) {
        for node in nodes {
            let raw = node.raw();
            if raw.start < range.start || raw.end > range.end {
                continue;
            }
            self.inline(node);
        }
    }
}

pub(super) fn project(
    snapshot: &Snapshot,
    selection: Selection,
    theme: moino_theme::BuiltinTheme,
) -> Vec<Leaf> {
    let source: Vec<char> = snapshot.text().to_string().chars().collect();
    let mut leaves = Vec::new();
    fn visit<'a>(
        nodes: &'a [Node],
        wrappers: &[Wrapper],
        output: &mut Vec<(&'a Node, Vec<Wrapper>, Range<ScalarOffset>)>,
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
                        let checked = item
                            .children()
                            .iter()
                            .chain(item.children().iter().flat_map(Node::children))
                            .find_map(|node| {
                                if let Syntax::Task(checked) = node.syntax() {
                                    Some(*checked)
                                } else {
                                    None
                                }
                            });
                        wrappers.push(Wrapper::List(
                            start.and_then(|start| start.checked_add(u64::try_from(ix).ok()?)),
                            checked,
                        ));
                        visit(std::slice::from_ref(item), &wrappers, output);
                    }
                }
                Syntax::Item => {
                    let children = node.children();
                    let inline = children
                        .iter()
                        .position(|child| {
                            matches!(
                                child.syntax(),
                                Syntax::Paragraph
                                    | Syntax::Heading(_)
                                    | Syntax::CodeBlock(_)
                                    | Syntax::Quote
                                    | Syntax::Callout(_)
                                    | Syntax::List(_)
                                    | Syntax::Rule
                                    | Syntax::MathBlock
                                    | Syntax::Table(_)
                                    | Syntax::HtmlBlock
                                    | Syntax::FrontMatter
                                    | Syntax::FootnoteDefinition(_)
                                    | Syntax::ReferenceDefinition
                                    | Syntax::Toc
                            )
                        })
                        .unwrap_or(children.len());
                    let mut continuation = wrappers.to_vec();
                    if let Some(wrapper) = continuation
                        .iter_mut()
                        .rev()
                        .find(|wrapper| matches!(wrapper, Wrapper::List(_, _)))
                    {
                        *wrapper = Wrapper::Indent;
                    }
                    let mut has_row = children[..inline]
                        .iter()
                        .any(|child| !matches!(child.syntax(), Syntax::Task(_)))
                        || children.is_empty()
                        || children
                            .first()
                            .is_some_and(|child| matches!(child.syntax(), Syntax::List(_)));
                    if has_row {
                        let end = children
                            .get(inline)
                            .map_or(node.raw().end, |child| child.raw().start);
                        output.push((node, wrappers.to_vec(), node.raw().start..end));
                    }
                    for child in &children[inline..] {
                        visit(
                            std::slice::from_ref(child),
                            if has_row { &continuation } else { wrappers },
                            output,
                        );
                        has_row = true;
                    }
                }
                _ => output.push((node, wrappers.to_vec(), node.raw())),
            }
        }
    }
    let mut rows = Vec::new();
    visit(snapshot.cst().nodes(), &[], &mut rows);
    for (node, wrappers, raw) in rows {
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
            mark_background: theme.mark_background(),
            scripts: Vec::new(),
        };
        let heading_ready = if matches!(node.syntax(), Syntax::Heading(_)) {
            let line = &source[raw.start.value()..raw.end.value()];
            let mut prefix = line.iter().skip_while(|ch| matches!(ch, ' ' | '\t'));
            if prefix.clone().next() == Some(&'#') {
                prefix.find(|ch| **ch != '#').is_some()
            } else {
                true
            }
        } else {
            false
        };
        let heading = match node.syntax() {
            Syntax::Heading(level) if heading_ready && !builder.expanded(raw.clone()) => {
                Some(*level)
            }
            _ => None,
        };
        let range = if matches!(node.syntax(), Syntax::Item) {
            node.children()
                .first()
                .map_or(raw.start, |child| child.raw().start)..raw.end
        } else if heading.is_some() {
            node.content()
        } else {
            raw.clone()
        };
        // Only ordinary leaves are interpreted here; rich blocks remain editable source until M4.
        let rule = matches!(node.syntax(), Syntax::Rule) && !builder.expanded(raw.clone());
        if matches!(node.syntax(), Syntax::Heading(_)) && !heading_ready {
            builder.append(raw.clone());
        } else if rule {
            builder.boundaries.push((0, raw.start));
        } else if matches!(
            node.syntax(),
            Syntax::Paragraph | Syntax::Heading(_) | Syntax::Item
        ) {
            if matches!(node.syntax(), Syntax::Heading(_)) && heading.is_none() {
                builder.append(node.opening_marker());
            }
            builder.children(node.children(), range);
            if matches!(node.syntax(), Syntax::Heading(_)) && heading.is_none() {
                builder.append(node.closing_marker());
            }
        } else {
            builder.append(range);
        }
        if heading_ready && heading.is_none() {
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
        for (range, _) in &builder.scripts {
            edges.extend([
                range.start.min(builder.text.len()),
                range.end.min(builder.text.len()),
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
                    (start < end).then(|| (start - edge[0]..end - edge[0], mark.clone()))
                })
                .collect();
            let mut scale = 1.;
            let mut shift = 0.;
            for (_, superscript) in builder
                .scripts
                .iter()
                .rev()
                .filter(|(range, _)| range.start <= edge[0] && edge[1] <= range.end)
            {
                shift += scale * moino_theme::inline_script_shift(*superscript);
                scale *= moino_theme::inline_script_scale();
            }
            let inline = InlineNode::new(builder.text[edge[0]..edge[1]].to_owned()).marks(marks);
            let inline = if scale != 1. {
                inline
                    .with_font_size_scale(scale)
                    .with_baseline_shift(shift)
            } else {
                inline
            };
            paragraph.push(
                if builder
                    .markers
                    .iter()
                    .any(|range| range.start <= edge[0] && edge[1] <= range.end)
                {
                    inline.with_foreground(theme.marker())
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
        let split = wrappers
            .iter()
            .rposition(|wrapper| matches!(wrapper, Wrapper::Indent))
            .map_or(0, |ix| ix + 1);
        for wrapper in wrappers[split..].iter().cloned().rev() {
            block = match wrapper {
                Wrapper::Quote => BlockNode::Blockquote {
                    children: vec![block],
                    span: None,
                },
                Wrapper::List(start, checked) => BlockNode::List {
                    ordered: start.is_some(),
                    start: start.and_then(|value| u32::try_from(value).ok()),
                    span: None,
                    children: vec![BlockNode::ListItem {
                        children: vec![block],
                        spread: false,
                        checked,
                        span: None,
                    }],
                },
                Wrapper::Indent => unreachable!("indent prefixes are rendered outside the block"),
            };
        }
        leaves.push(Leaf {
            key: node.id().value(),
            block,
            text: builder.text,
            raw,
            heading_badge: match node.syntax() {
                Syntax::Heading(level) if heading_ready && heading.is_none() => Some(*level),
                _ => None,
            },
            boundaries: builder.boundaries,
            prefix: wrappers[..split].to_vec(),
            rule,
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
            prefix: Vec::new(),
            rule: false,
            boundaries: vec![(0, at)],
        });
    }
    leaves
}

#[derive(Clone)]
pub(super) enum Wrapper {
    Quote,
    List(Option<u64>, Option<bool>),
    Indent,
}

/// Overlays native preedit on its affected leaves without reparsing temporary text.
pub(super) fn compose(mut leaves: Vec<Leaf>, range: Range<ScalarOffset>, text: &str) -> Vec<Leaf> {
    let first = leaves
        .iter()
        .rposition(|leaf| leaf.raw.start <= range.start)
        .unwrap_or(0);
    let last = leaves
        .iter()
        .rposition(|leaf| leaf.raw.start < range.end)
        .unwrap_or(first)
        .max(first);
    let before = &leaves[first];
    let after = &leaves[last];
    let start = before.byte(range.start, Affinity::After);
    let end = after.byte(range.end, Affinity::Before);
    let prefix = &before.text[..start];
    let suffix = &after.text[end..];
    let visible = format!("{prefix}{text}{suffix}");
    let inserted = text.chars().count();
    let shift = |at: ScalarOffset| {
        if at >= range.end {
            ScalarOffset::new(at.value() - range.end.value() + range.start.value() + inserted)
        } else if at >= range.start {
            ScalarOffset::new(range.start.value() + inserted)
        } else {
            at
        }
    };
    let mut boundaries = Vec::new();
    for (byte, _) in prefix.char_indices() {
        boundaries.push((byte, before.source(byte)));
    }
    for (scalar, (byte, _)) in text.char_indices().enumerate() {
        boundaries.push((
            start + byte,
            ScalarOffset::new(range.start.value() + scalar),
        ));
    }
    for (byte, _) in suffix.char_indices() {
        boundaries.push((start + text.len() + byte, shift(after.source(end + byte))));
    }
    boundaries.push((visible.len(), shift(after.source(after.text.len()))));
    let mut replacement = Leaf::plain(&visible, Some(start..start + text.len()));
    replacement.key = before.key;
    replacement.raw = before.raw.start..shift(after.raw.end);
    replacement.boundaries = boundaries;
    replacement.heading_badge = before.heading_badge;
    replacement.prefix = before.prefix.clone();
    let BlockNode::Paragraph(paragraph) = replacement.block else {
        unreachable!("plain leaf")
    };
    fn replace(block: &mut BlockNode, paragraph: &Paragraph) {
        match block {
            BlockNode::Paragraph(value)
            | BlockNode::Heading {
                children: value, ..
            } => *value = paragraph.clone(),
            BlockNode::Blockquote { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => {
                for child in children {
                    replace(child, paragraph);
                }
            }
            _ => {}
        }
    }
    let mut block = before.block.clone();
    replace(&mut block, &paragraph.with_leaf_key(before.key));
    replacement.block = block;
    for leaf in &mut leaves[last + 1..] {
        leaf.raw = shift(leaf.raw.start)..shift(leaf.raw.end);
        for (_, offset) in &mut leaf.boundaries {
            *offset = shift(*offset);
        }
    }
    leaves.splice(first..=last, [replacement]);
    leaves
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
    fn expanding_an_outer_mark_keeps_unselected_nested_marks_collapsed() {
        let document = Document::new(DocumentId::new(1), Epoch::new(0), "**a *中* z** end");
        let leaves = project(
            &document.snapshot(),
            Selection::cursor(ScalarOffset::new(3)),
            moino_theme::BuiltinTheme::Paper,
        );
        assert_eq!(leaves[0].text, "**a 中 z** end");
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
}
