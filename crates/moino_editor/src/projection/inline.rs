//! Inline style and coordinates are compiled together, never inferred from painted text.
use super::*;
use gpui_kit::Hsla;
use gpui_kit::base::text::editor_host::LinkMark;

#[derive(Clone, Default)]
pub(super) struct Style {
    marks: Vec<TextMark>,
    foreground: Option<Hsla>,
    scale: Option<f32>,
    shift: f32,
}
impl Style {
    pub fn bold(mut self) -> Self {
        self.marks.push(TextMark::default().bold());
        self
    }
}

pub(super) struct InlineCompiler<'a> {
    source: &'a [char],
    selected: Selection,
    theme: moino_theme::BuiltinTheme,
    text: String,
    boundaries: Vec<(usize, ScalarOffset)>,
    runs: Vec<InlineNode>,
}
impl<'a> InlineCompiler<'a> {
    pub fn new(source: &'a [char], selected: Selection, theme: moino_theme::BuiltinTheme) -> Self {
        Self {
            source,
            selected,
            theme,
            text: String::new(),
            boundaries: Vec::new(),
            runs: Vec::new(),
        }
    }
    pub fn active(&self, range: &Range<ScalarOffset>) -> bool {
        let at = self.selected.head().offset;
        range.start <= at && at <= editable_end(self.source, range)
    }
    fn at_endpoint(&self, range: &Range<ScalarOffset>) -> bool {
        let selected = self.selected.range();
        if self.selected.is_collapsed() {
            range.start <= selected.start && selected.start <= range.end
        } else {
            (range.start < selected.start && selected.start < range.end)
                || (range.start < selected.end && selected.end < range.end)
        }
    }
    pub fn source(&mut self, range: Range<ScalarOffset>, style: &Style) {
        let text: String = self.source[range.start.value()..range.end.value()]
            .iter()
            .collect();
        self.push(
            &text,
            (range.start.value()..range.end.value())
                .map(ScalarOffset::new)
                .collect(),
            range.end,
            style,
        );
    }
    pub fn marker(&mut self, range: Range<ScalarOffset>, inherited: &Style) {
        let mut style = inherited.clone();
        style.foreground = Some(self.theme.marker());
        self.source(range, &style);
    }
    fn push(&mut self, text: &str, positions: Vec<ScalarOffset>, end: ScalarOffset, style: &Style) {
        let base = self.text.len();
        for ((byte, _), position) in text.char_indices().zip(positions) {
            self.boundaries.push((base + byte, position));
        }
        self.text.push_str(text);
        self.boundaries.push((self.text.len(), end));
        if text.is_empty() {
            return;
        }
        let mut run = InlineNode::new(text.to_owned()).marks(
            style
                .marks
                .iter()
                .cloned()
                .map(|mark| (0..text.len(), mark))
                .collect(),
        );
        if let Some(color) = style.foreground {
            run = run.with_foreground(color);
        }
        if let Some(scale) = style.scale {
            run = run
                .with_font_size_scale(scale)
                .with_baseline_shift(style.shift);
        }
        self.runs.push(run);
    }
    fn decoded(&mut self, node: &Node, style: &Style) {
        let range = node.raw();
        let mut at = range.start.value();
        let mut positions = Vec::new();
        for ch in node.text().chars() {
            let matched = self.source[at..range.end.value()]
                .iter()
                .position(|source| *source == ch)
                .unwrap_or(0);
            let source = (at + matched).min(range.end.value());
            positions.push(ScalarOffset::new(source));
            at = (source + 1).min(range.end.value());
        }
        self.push(node.text(), positions, range.end, style);
    }
    fn code(&mut self, mut range: Range<ScalarOffset>, style: &Style) {
        let spaces = |ch: &char| matches!(ch, ' ' | '\n');
        let chars = &self.source[range.start.value()..range.end.value()];
        if chars.first().is_some_and(spaces)
            && chars.last().is_some_and(spaces)
            && !chars.iter().all(spaces)
        {
            range.start = ScalarOffset::new(range.start.value() + 1);
            range.end = ScalarOffset::new(range.end.value() - 1);
        }
        let text: String = self.source[range.start.value()..range.end.value()]
            .iter()
            .map(|ch| if *ch == '\n' { ' ' } else { *ch })
            .collect();
        self.push(
            &text,
            (range.start.value()..range.end.value())
                .map(ScalarOffset::new)
                .collect(),
            range.end,
            style,
        );
    }
    pub fn children(
        &mut self,
        nodes: &[Node],
        range: Range<ScalarOffset>,
        style: &Style,
        expanded: bool,
    ) {
        for node in nodes {
            if node.raw().start >= range.start && node.raw().end <= range.end {
                self.node(node, style, expanded);
            }
        }
    }
    fn node(&mut self, node: &Node, inherited: &Style, outer_expanded: bool) {
        match node.syntax() {
            Syntax::Task(_) => return,
            Syntax::Text | Syntax::SoftBreak | Syntax::HardBreak => {
                if outer_expanded || self.at_endpoint(&node.raw()) {
                    self.source(node.raw(), inherited);
                } else {
                    self.decoded(node, inherited);
                }
                return;
            }
            Syntax::Emphasis
            | Syntax::Strong
            | Syntax::Strike
            | Syntax::Code
            | Syntax::Link(_)
            | Syntax::Highlight
            | Syntax::Subscript
            | Syntax::Superscript => {}
            _ => {
                self.source(node.raw(), inherited);
                return;
            }
        }
        let expanded = outer_expanded || self.at_endpoint(&node.raw());
        if expanded {
            self.marker(node.opening_marker(), inherited);
        }
        let mut style = inherited.clone();
        let mark = match node.syntax() {
            Syntax::Emphasis => TextMark::default().italic(),
            Syntax::Strong => TextMark::default().bold(),
            Syntax::Strike => TextMark::default().strikethrough(),
            Syntax::Code => TextMark::default().code(),
            Syntax::Highlight => TextMark::default().highlight(self.theme.mark_background()),
            Syntax::Link(url) => TextMark::default().link(LinkMark {
                url: url.clone().into(),
                ..Default::default()
            }),
            Syntax::Subscript | Syntax::Superscript => {
                let parent_scale = style.scale.unwrap_or(1.);
                style.shift += parent_scale
                    * moino_theme::inline_script_shift(matches!(
                        node.syntax(),
                        Syntax::Superscript
                    ));
                style.scale = Some(parent_scale * moino_theme::inline_script_scale());
                TextMark::default()
            }
            _ => unreachable!("supported inline syntax"),
        };
        style.marks.push(mark);
        if node.children().is_empty() {
            if matches!(node.syntax(), Syntax::Code) && !expanded {
                self.code(node.content(), &style);
            } else {
                self.source(node.content(), &style);
            }
        } else {
            self.children(node.children(), node.content(), &style, expanded);
        }
        if expanded {
            self.marker(node.closing_marker(), inherited);
        }
    }
    pub fn finish(
        mut self,
        key: u64,
        fallback: ScalarOffset,
    ) -> (Paragraph, String, Vec<(usize, ScalarOffset)>) {
        // Emitted block ranges exclude terminators; soft breaks inside text remain visible.
        if self.boundaries.is_empty() {
            self.boundaries.push((0, fallback));
        }
        let mut paragraph = Paragraph::default().with_leaf_key(key);
        for run in self.runs {
            paragraph.push(run);
        }
        if self.text.is_empty() {
            paragraph.push(InlineNode::new(" "));
        }
        (paragraph, self.text, self.boundaries)
    }
}
