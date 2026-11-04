//! Shared native input validation and reversible preedit sessions.
use gpui_kit::UTF16Selection;
use moino_text::{ScalarOffset, TextBuffer, Utf16Offset};
use std::ops::Range;

/// Registers one owner after a retained Kit widget paints its native text bounds.
pub(crate) fn editor_input<T: gpui_kit::EntityInputHandler>(
    owner: gpui_kit::Entity<T>,
    state: gpui_kit::Entity<gpui_kit::component::input::EditorState>,
) -> impl gpui_kit::IntoElement {
    use gpui_kit::{prelude::*, *};
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let state = state.read(cx);
            window.handle_input(
                &state.focus_handle(cx),
                ElementInputHandler::new(state.text_bounds().unwrap_or(bounds), owner),
                cx,
            );
        },
    )
    .absolute()
    .size_full()
}

/// Converts a native position while accounting for CRLF normalization.
pub(crate) fn normalized_position(source: &str, offset: Utf16Offset) -> Option<ScalarOffset> {
    let mut units = 0;
    for (byte, character) in source.char_indices() {
        if units == offset.value() {
            return Some(TextBuffer::new(&source[..byte]).len());
        }
        units += character.len_utf16();
    }
    (units == offset.value()).then(|| TextBuffer::new(source).len())
}

/// Rejects reversed, out-of-bounds and split-surrogate native replacement ranges.
pub(crate) fn native_range(text: &TextBuffer, range: Range<usize>) -> Option<Range<ScalarOffset>> {
    if range.start > range.end {
        return None;
    }
    Some(
        text.utf16_to_scalar(Utf16Offset::new(range.start)).ok()?
            ..text.utf16_to_scalar(Utf16Offset::new(range.end)).ok()?,
    )
}

/// Validated LF text and its document-independent native selection.
pub(crate) struct NormalizedInput {
    text: String,
    selection: Range<Utf16Offset>,
}

impl NormalizedInput {
    pub(crate) fn new(source: &str, selection: Option<Range<usize>>) -> Option<Self> {
        let text = TextBuffer::new(source);
        let selection = if let Some(range) = selection {
            if range.start > range.end {
                return None;
            }
            let start = normalized_position(source, Utf16Offset::new(range.start))?;
            let end = normalized_position(source, Utf16Offset::new(range.end))?;
            text.to_utf16(start).ok()?..text.to_utf16(end).ok()?
        } else {
            let end = text.to_utf16(text.len()).ok()?;
            end..end
        };
        Some(Self {
            text: text.to_string(),
            selection,
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Selection in normalized UTF-16 code units, as required by native handlers.
    pub(crate) fn selection(&self) -> Range<usize> {
        self.selection.start.value()..self.selection.end.value()
    }
}

/// A direction-preserving text snapshot used only to cancel uncommitted preedit.
pub(crate) struct InputSnapshot {
    pub text: String,
    pub selection: UTF16Selection,
}

/// One active native composition; finishing makes late cancellation a no-op.
#[derive(Default)]
pub(crate) enum PreeditSession {
    #[default]
    Idle,
    Active {
        original: InputSnapshot,
        marked: Range<Utf16Offset>,
    },
}

impl PreeditSession {
    pub(crate) fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }

    pub(crate) fn begin(&mut self, text: &str, selection: UTF16Selection) -> bool {
        if self.is_active() {
            return true;
        }
        if native_range(&TextBuffer::new(text), selection.range.clone()).is_none() {
            return false;
        }
        let marked = Utf16Offset::new(selection.range.start)..Utf16Offset::new(selection.range.end);
        *self = Self::Active {
            original: InputSnapshot {
                text: text.to_owned(),
                selection,
            },
            marked,
        };
        true
    }

    pub(crate) fn update_marked(&mut self, range: Range<usize>) {
        if let Self::Active { marked, .. } = self {
            *marked = Utf16Offset::new(range.start)..Utf16Offset::new(range.end);
        }
    }

    pub(crate) fn marked_range(&self) -> Option<Range<usize>> {
        match self {
            Self::Idle => None,
            Self::Active { marked, .. } => Some(marked.start.value()..marked.end.value()),
        }
    }

    pub(crate) fn finish(&mut self) -> bool {
        self.cancel().is_some()
    }

    pub(crate) fn cancel(&mut self) -> Option<InputSnapshot> {
        match std::mem::take(self) {
            Self::Idle => None,
            Self::Active { original, .. } => Some(original),
        }
    }
}
