//! Native UTF-16 events become validated edits before entering the document queue.
use super::*;
use crate::native_input::{NormalizedInput, native_range};
use std::ops::Range;

struct Replacement {
    range: Range<ScalarOffset>,
    text: String,
    selection: Range<usize>,
}

impl LiveEditor {
    fn replacement(
        &self,
        range: Option<Range<usize>>,
        input: &NormalizedInput,
    ) -> Option<Replacement> {
        let target = match range.as_ref() {
            Some(range) => self.native_range(range.clone())?,
            None => self.composition.as_ref().map_or_else(
                || self.document.selection().range(),
                |composition| composition.range.clone(),
            ),
        };
        if let (Some(native), Some(composition)) = (range, &self.composition) {
            let display = self.display_text();
            let replaced = native_range(&display, native)?;
            let union =
                target.start.min(composition.range.start)..target.end.max(composition.range.end);
            let end = ScalarOffset::new(
                union.end.value() - composition.range.end.value()
                    + composition.range.start.value()
                    + composition.text.chars().count(),
            );
            let prefix = display.slice(union.start..replaced.start).ok()?;
            let suffix = display.slice(replaced.end..end).ok()?;
            let shift = prefix.encode_utf16().count();
            return Some(Replacement {
                range: union,
                text: format!("{prefix}{}{suffix}", input.text()),
                selection: shift + input.selection().start..shift + input.selection().end,
            });
        }
        let range = if target.is_empty() && self.composition.is_none() {
            let at = self.content_position(target.start);
            at..at
        } else {
            target
        };
        Some(Replacement {
            range,
            text: input.text().into(),
            selection: input.selection(),
        })
    }
}

impl EntityInputHandler for LiveEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.display_text();
        let value = text.slice(native_range(&text, range.clone())?).ok()?;
        *adjusted = Some(range);
        Some(value)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.native_selection()
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let composition = self.composition.as_ref()?;
        let start = self
            .document
            .snapshot()
            .text()
            .to_utf16(composition.range.start)
            .ok()?
            .value();
        Some(start..start + composition.text.encode_utf16().count())
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition(window, cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_read_only() {
            return;
        }
        let Some(input) = NormalizedInput::new(text, None) else {
            return;
        };
        let explicit = range.is_some();
        let Some(replacement) = self.replacement(range, &input) else {
            return;
        };
        self.keyboard_boundary();
        let composition = self.composition.take();
        // Implicit native typing is relative to the queue tail, not the last painted frame.
        if explicit
            || composition
                .as_ref()
                .is_some_and(|composition| !composition.follows_pending)
        {
            self.enqueue(
                self.selection_operation(selection(replacement.range.start, replacement.range.end)),
                window,
                cx,
            );
        }
        self.enqueue(
            Operation::Intent(if composition.is_some() {
                Intent::Composition(replacement.text)
            } else {
                Intent::Insert(replacement.text)
            }),
            window,
            cx,
        );
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_read_only() {
            return;
        }
        let Some(input) = NormalizedInput::new(text, selected) else {
            return;
        };
        let follows_pending = self
            .composition
            .as_ref()
            .map_or(range.is_none() && self.is_pending(), |composition| {
                composition.follows_pending
            });
        let Some(replacement) = self.replacement(range, &input) else {
            return;
        };
        self.keyboard_boundary();
        if text.is_empty() {
            self.composition = None;
        } else {
            self.composition = Some(Composition {
                range: replacement.range,
                text: replacement.text,
                selection: replacement.selection,
                follows_pending,
                revision: self.document.snapshot().revision(),
            });
        }
        self.reveal_caret = true;
        self.refresh(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.caret(native_range(&self.display_text(), range)?.start)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.display_text()
            .to_utf16(self.hit(point)?)
            .ok()
            .map(Utf16Offset::value)
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(range) = native_range(&self.display_text(), range) {
            self.keyboard_boundary();
            self.select(selection(range.start, range.end), window, cx);
        }
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let text = self.display_text();
        text.to_utf16(text.len()).ok().map(Utf16Offset::value)
    }
}

#[cfg(test)]
mod tests;
