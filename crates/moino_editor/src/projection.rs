//! Presentation compiler: retained block state, syntax and endpoint selection produce mapped leaves.
use gpui_kit::base::text::editor_host::{BlockNode, InlineNode, Paragraph, TextMark};
use moino_document::{Affinity, Selection, Snapshot};
use moino_markdown::{Node, Syntax};
use moino_text::ScalarOffset;
use std::ops::Range;

mod blocks;
mod composition;
mod inline;
pub(super) use composition::compose;

pub(super) struct Leaf {
    pub key: u64,
    pub block: BlockNode,
    pub text: String,
    pub raw: Range<ScalarOffset>,
    pub heading_badge: Option<u8>,
    pub prefix: Vec<Wrapper>,
    pub rule: bool,
    content: Option<Range<ScalarOffset>>,
    hidden_prefixes: Vec<Range<ScalarOffset>>,
    boundaries: Vec<(usize, ScalarOffset)>,
}

impl Leaf {
    pub fn plain(text: &str, marked: Option<Range<usize>>) -> Self {
        let end = ScalarOffset::new(text.chars().count());
        let mut boundaries: Vec<_> = text
            .char_indices()
            .enumerate()
            .map(|(scalar, (byte, _))| (byte, ScalarOffset::new(scalar)))
            .collect();
        boundaries.push((text.len(), end));
        let mut paragraph = Paragraph::default().with_leaf_key(0);
        paragraph.push(
            InlineNode::new(if text.is_empty() { " " } else { text }).marks(
                marked
                    .into_iter()
                    .map(|range| (range, TextMark::default().underline()))
                    .collect(),
            ),
        );
        Self {
            key: 0,
            block: BlockNode::Paragraph(paragraph),
            text: text.into(),
            raw: ScalarOffset::new(0)..end,
            heading_badge: None,
            prefix: Vec::new(),
            rule: false,
            content: None,
            hidden_prefixes: Vec::new(),
            boundaries,
        }
    }

    pub fn source(&self, byte: usize) -> ScalarOffset {
        let byte = byte.min(self.text.len());
        self.boundaries
            .iter()
            .rev()
            .find(|(position, _)| *position <= byte)
            .map_or(self.raw.start, |(_, source)| *source)
    }

    pub fn byte(&self, source: ScalarOffset, affinity: Affinity) -> usize {
        if self.boundaries.first().is_some_and(|(_, at)| source <= *at) {
            return 0;
        }
        if self.boundaries.last().is_some_and(|(_, at)| source >= *at) {
            return self.text.len();
        }
        match affinity {
            Affinity::Before => self.boundaries.iter().find(|(_, at)| *at >= source),
            Affinity::After => self.boundaries.iter().rev().find(|(_, at)| *at <= source),
        }
        .map_or(0, |(byte, _)| *byte)
    }

    pub fn content_position(&self, source: ScalarOffset) -> ScalarOffset {
        let source = self
            .content
            .as_ref()
            .map_or(source, |range| source.clamp(range.start, range.end));
        self.hidden_prefixes
            .iter()
            .find(|range| range.contains(&source))
            .map_or(source, |range| range.end)
    }

    pub fn contains_content(&self, source: ScalarOffset) -> bool {
        self.content
            .as_ref()
            .is_none_or(|range| range.start <= source && source <= range.end)
            && !self
                .hidden_prefixes
                .iter()
                .any(|range| range.contains(&source))
    }
}

#[derive(Clone)]
pub(super) enum Wrapper {
    Quote,
    List(Option<u64>, Option<bool>),
    Indent,
}

pub(super) fn project(
    snapshot: &Snapshot,
    selected: Selection,
    theme: moino_theme::BuiltinTheme,
) -> Vec<Leaf> {
    project_draft(snapshot, selected, theme, None)
}

pub(super) fn project_draft(
    snapshot: &Snapshot,
    selected: Selection,
    theme: moino_theme::BuiltinTheme,
    draft: Option<Range<ScalarOffset>>,
) -> Vec<Leaf> {
    let source: Vec<char> = snapshot.text().to_string().chars().collect();
    let mut leaves = Vec::new();
    for row in blocks::rows(snapshot.cst().nodes()) {
        if row.raw.is_empty() {
            continue;
        }
        let pending = draft
            .as_ref()
            .is_some_and(|range| range.start < row.raw.end && row.raw.start <= range.end);
        leaves.push(blocks::compile(row, &source, selected, theme, pending));
    }
    if leaves.is_empty() || source.last() == Some(&'\n') {
        let mut empty = Leaf::plain("", None);
        empty.key = u64::MAX;
        empty.raw = snapshot.text().len()..snapshot.text().len();
        empty.boundaries = vec![(0, snapshot.text().len())];
        let mut paragraph = Paragraph::default().with_leaf_key(empty.key);
        paragraph.push(InlineNode::new(" "));
        empty.block = BlockNode::Paragraph(paragraph);
        leaves.push(empty);
    }
    leaves
}

fn editable_end(source: &[char], range: &Range<ScalarOffset>) -> ScalarOffset {
    let mut end = range.end.value();
    while end > range.start.value() && source[end - 1] == '\n' {
        end -= 1;
    }
    ScalarOffset::new(end)
}

#[cfg(test)]
mod tests;
