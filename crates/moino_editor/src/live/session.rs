//! Editing state is retained across reparses; a parsed heading is not necessarily a committed block.
use super::*;
use moino_document::{Edit, HistoryState, Snapshot};
use moino_markdown::{Node, Syntax};
use std::{collections::HashSet, ops::Range};

#[derive(Clone, Default)]
pub(super) struct BlockSession {
    draft: Option<Range<ScalarOffset>>,
    states: HashMap<HistoryState, Option<Range<ScalarOffset>>>,
}

impl BlockSession {
    pub fn pending(&self) -> Option<Range<ScalarOffset>> {
        self.draft.clone()
    }

    pub fn begin(&mut self, document: &Document, intent: &Intent) {
        self.remember(document);
        if !matches!(intent, Intent::Insert(_) | Intent::Composition(_)) {
            return;
        }
        if self.draft.is_some() {
            return;
        }
        let snapshot = document.snapshot();
        let at = document.selection().head().offset;
        match block_at(snapshot.cst().nodes(), at, &snapshot) {
            Some(node)
                if matches!(
                    node.syntax(),
                    Syntax::Paragraph | Syntax::Trivia | Syntax::Item
                ) =>
            {
                self.draft = Some(trim_end(&snapshot, node.raw()));
            }
            None => self.draft = Some(at..at),
            _ => {}
        }
    }

    pub fn applied(&mut self, document: &Document, edits: &[Edit], keep_draft: bool) {
        if keep_draft {
            if let Some(range) = &self.draft {
                let mapped = Selection::new(
                    Endpoint {
                        offset: range.start,
                        affinity: Affinity::Before,
                    },
                    Endpoint {
                        offset: range.end,
                        affinity: Affinity::After,
                    },
                )
                .mapped(edits)
                .range();
                let snapshot = document.snapshot();
                self.draft = Some(trim_end(&snapshot, mapped));
                // List/quote conversion creates a container immediately. Its text leaf
                // may start a new candidate later; the preceding paragraph is gone.
                if block_at(
                    snapshot.cst().nodes(),
                    self.draft.as_ref().expect("draft").start,
                    &snapshot,
                )
                .is_some_and(|node| {
                    matches!(
                        node.syntax(),
                        Syntax::List(_) | Syntax::Quote | Syntax::Callout(_) | Syntax::Item
                    )
                }) {
                    self.draft = None;
                }
            }
        } else {
            self.draft = None;
        }
        self.remember(document);
    }

    pub fn leaves(&self, selected: Selection) -> bool {
        self.draft.as_ref().is_some_and(|range| {
            selected.head().offset < range.start || selected.head().offset > range.end
        })
    }

    pub fn has_conversion(&self, snapshot: &Snapshot) -> bool {
        self.draft.as_ref().is_some_and(|range| {
            block_at(snapshot.cst().nodes(), range.start, snapshot).is_some_and(|node| {
                matches!(
                    node.syntax(),
                    Syntax::Heading(_)
                        | Syntax::Rule
                        | Syntax::CodeBlock(_)
                        | Syntax::MathBlock
                        | Syntax::HtmlBlock
                        | Syntax::Toc
                )
            })
        })
    }

    pub fn finish(&mut self, document: &Document) {
        self.draft = None;
        self.remember(document);
    }

    pub fn restore(&mut self, document: &Document) {
        self.draft = self
            .states
            .get(&document.snapshot().history_state())
            .cloned()
            .flatten();
    }

    pub fn remember(&mut self, document: &Document) {
        self.states
            .insert(document.snapshot().history_state(), self.draft.clone());
        if self.states.len() > 2048 {
            let retained: HashSet<_> = document.history_states().collect();
            self.states.retain(|state, _| retained.contains(state));
        }
    }
}

fn trim_end(snapshot: &Snapshot, mut range: Range<ScalarOffset>) -> Range<ScalarOffset> {
    range.end = range.end.min(snapshot.text().len());
    range.start = range.start.min(range.end);
    let source = snapshot.text().slice(range.clone()).unwrap_or_default();
    let newlines = source.chars().rev().take_while(|ch| *ch == '\n').count();
    range.end = ScalarOffset::new(range.end.value() - newlines);
    range
}

fn block_at<'a>(nodes: &'a [Node], at: ScalarOffset, snapshot: &Snapshot) -> Option<&'a Node> {
    let node = nodes.iter().find(|node| {
        let range = trim_end(snapshot, node.raw());
        range.start <= at && at <= range.end
    })?;
    if matches!(
        node.syntax(),
        Syntax::List(_) | Syntax::Quote | Syntax::Callout(_) | Syntax::Item
    ) {
        block_at(node.children(), at, snapshot).or(Some(node))
    } else {
        Some(node)
    }
}

#[cfg(test)]
mod tests;
