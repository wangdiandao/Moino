//! One validated source snapshot shared by every rule in a command dispatch.
use super::{
    EditingError, Node, Options, Origin, Outcome, Range, ScalarOffset, Selection, Snapshot,
    normalize_newlines, replacement,
};

pub(super) struct CommandContext<'a> {
    pub snapshot: &'a Snapshot,
    pub selection: Selection,
    pub options: &'a Options,
    pub chars: Vec<char>,
    pub line: Range<usize>,
}
impl<'a> CommandContext<'a> {
    pub fn new(
        snapshot: &'a Snapshot,
        selection: Selection,
        options: &'a Options,
    ) -> Result<Self, EditingError> {
        if options.read_only || selection.range().end > snapshot.text().len() {
            return Err(EditingError);
        }
        if options
            .pending_block()
            .is_some_and(|range| range.start > range.end || range.end > snapshot.text().len())
        {
            return Err(EditingError);
        }
        let chars: Vec<_> = snapshot.text().to_string().chars().collect();
        let start = selection.range().start.value();
        let line_start = chars[..start]
            .iter()
            .rposition(|ch| *ch == '\n')
            .map_or(0, |at| at + 1);
        let line_end = chars[start..]
            .iter()
            .position(|ch| *ch == '\n')
            .map_or(chars.len(), |at| start + at);
        Ok(Self {
            snapshot,
            selection,
            options,
            chars,
            line: line_start..line_end,
        })
    }
    pub fn insert_plain(&self, value: &str, origin: Origin) -> Outcome {
        let value = normalize_newlines(value);
        replacement(
            self.snapshot,
            self.selection.range(),
            &value,
            self.selection.range().start.value() + value.chars().count(),
            origin,
        )
    }
    pub fn is_pending(&self) -> bool {
        self.options.pending_block().is_some_and(|range| {
            range.start <= self.selection.range().start && self.selection.range().end <= range.end
        })
    }
    pub fn has_unfinished_heading_prefix(&self) -> bool {
        self.is_pending() && self.chars[self.line.clone()].iter().all(|ch| *ch == '#')
    }
    pub fn path(&self, position: ScalarOffset) -> Vec<&Node> {
        fn descend<'a>(
            nodes: &'a [Node],
            position: ScalarOffset,
            end: ScalarOffset,
            include_end: bool,
            output: &mut Vec<&'a Node>,
        ) {
            if let Some(node) = nodes.iter().find(|node| {
                node.raw().start <= position
                    && (position < node.raw().end
                        || include_end && position == end && node.raw().end == end)
            }) {
                output.push(node);
                descend(node.children(), position, end, include_end, output);
            }
        }
        let mut path = Vec::new();
        descend(
            self.snapshot.cst().nodes(),
            position,
            self.snapshot.text().len(),
            self.chars.last() != Some(&'\n'),
            &mut path,
        );
        path
    }
}
