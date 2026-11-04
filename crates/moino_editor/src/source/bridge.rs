//! Publication boundary between the native source widget and the document queue.
use crate::native_input::normalized_position;
use crate::source_change::SourceChange;
use gpui_kit::UTF16Selection;
use moino_document::{Affinity, Endpoint, Origin, Selection, Snapshot};
use moino_text::{TextBuffer, Utf16Offset};
use std::ops::Range;

pub(super) struct Draft {
    pub text: String,
    pub selection: Selection,
}

impl Draft {
    pub fn capture(raw: &str, native: UTF16Selection) -> Option<Self> {
        if native.range.start > native.range.end {
            return None;
        }
        let start = normalized_position(raw, Utf16Offset::new(native.range.start))?;
        let end = normalized_position(raw, Utf16Offset::new(native.range.end))?;
        let (anchor, head) = if native.reversed {
            (end, start)
        } else {
            (start, end)
        };
        Some(Self {
            text: TextBuffer::new(raw).to_string(),
            selection: Selection::new(
                Endpoint {
                    offset: anchor,
                    affinity: Affinity::After,
                },
                Endpoint {
                    offset: head,
                    affinity: Affinity::After,
                },
            ),
        })
    }

    pub fn from_document(snapshot: &Snapshot, selection: Selection) -> Self {
        Self {
            text: snapshot.text().to_string(),
            selection,
        }
    }

    pub fn byte_selection(&self) -> Range<usize> {
        let text = TextBuffer::new(&self.text);
        text.to_byte(self.selection.anchor().offset)
            .expect("draft anchor")
            .value()
            ..text
                .to_byte(self.selection.head().offset)
                .expect("draft head")
                .value()
    }

    pub fn native_selection(&self) -> UTF16Selection {
        let text = TextBuffer::new(&self.text);
        let range = self.selection.range();
        UTF16Selection {
            range: text.to_utf16(range.start).expect("draft start").value()
                ..text.to_utf16(range.end).expect("draft end").value(),
            reversed: !self.selection.is_forward() && !self.selection.is_collapsed(),
        }
    }
}

pub(super) enum Publication {
    Text {
        text: String,
        selection: Selection,
        change: Option<SourceChange>,
        origin: Origin,
    },
    Selection(Selection),
}

/// Tracks only state already submitted to the document, never uncommitted preedit.
pub(super) struct SourceBridge {
    submitted: Draft,
    next_origin: Origin,
}

impl SourceBridge {
    pub fn new(draft: Draft) -> Self {
        Self {
            submitted: draft,
            next_origin: Origin::Typing,
        }
    }

    pub fn submitted(&self) -> &Draft {
        &self.submitted
    }

    pub fn adopt(&mut self, draft: Draft) {
        self.submitted = draft;
    }

    pub fn command(&mut self, origin: Origin) {
        self.next_origin = origin;
    }

    pub fn publish(
        &mut self,
        draft: Draft,
        change: Option<SourceChange>,
        composition: bool,
    ) -> Option<Publication> {
        let origin = if composition {
            Origin::Composition
        } else {
            self.next_origin
        };
        let publication = if draft.text != self.submitted.text {
            self.next_origin = Origin::Typing;
            Some(Publication::Text {
                text: draft.text.clone(),
                selection: draft.selection,
                change,
                origin,
            })
        } else if draft.selection != self.submitted.selection {
            Some(Publication::Selection(draft.selection))
        } else {
            None
        };
        self.submitted = draft;
        publication
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(text: &str, range: Range<usize>, reversed: bool) -> Draft {
        Draft::capture(text, UTF16Selection { range, reversed }).expect("valid native draft")
    }

    #[test]
    // Spec: docs/contracts/history.md#撤销单元
    fn selection_notifications_do_not_consume_a_pending_command_origin() {
        let mut bridge = SourceBridge::new(draft("ab", 2..2, false));
        bridge.command(Origin::Paste);
        assert!(matches!(
            bridge.publish(draft("ab", 1..1, false), None, false),
            Some(Publication::Selection(_))
        ));
        assert!(
            bridge
                .publish(draft("ab", 1..1, false), None, false)
                .is_none()
        );
        assert!(matches!(
            bridge.publish(draft("a中b", 2..2, false), None, false),
            Some(Publication::Text {
                origin: Origin::Paste,
                ..
            })
        ));
        assert!(
            bridge
                .publish(draft("a中b", 2..2, false), None, false)
                .is_none()
        );
        assert!(matches!(
            bridge.publish(draft("a中文b", 3..3, false), None, false),
            Some(Publication::Text {
                origin: Origin::Typing,
                ..
            })
        ));
    }

    #[test]
    // Spec: docs/modules/selection.md#逻辑选择
    fn normalized_source_draft_retains_backward_unicode_selection() {
        let draft = draft("a\r\n😀中", 1..6, true);
        assert_eq!(draft.text, "a\n😀中");
        assert_eq!(draft.selection.anchor().offset.value(), 4);
        assert_eq!(draft.selection.head().offset.value(), 1);
        assert_eq!(draft.byte_selection(), Range { start: 9, end: 1 });
        assert_eq!(draft.native_selection().range, 1..5);
        assert!(draft.native_selection().reversed);
    }
}
