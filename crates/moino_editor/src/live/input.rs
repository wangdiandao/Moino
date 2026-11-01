use super::*;

impl EntityInputHandler for LiveEditor {
    fn text_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        adjusted: &mut Option<std::ops::Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.display_text();
        let start = text.utf16_to_scalar(Utf16Offset::new(range.start)).ok()?;
        let end = text.utf16_to_scalar(Utf16Offset::new(range.end)).ok()?;
        *adjusted = Some(range);
        text.slice(start..end).ok()
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.native_selection()
    }
    fn marked_text_range(
        &self,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
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
        range: Option<std::ops::Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = if let Some(range) = range {
            let Some(target) = self.native_range(range) else {
                return;
            };
            Some(target)
        } else {
            None
        };
        let composing = self.composition.take();
        if let Some(target) = target {
            self.enqueue(
                self.selection_operation(selection(target.start, target.end)),
                window,
                cx,
            );
        } else if let Some(composition) = &composing
            && !composition.follows_pending
        {
            self.enqueue(
                self.selection_operation(selection(composition.range.start, composition.range.end)),
                window,
                cx,
            );
        }
        self.enqueue(
            Operation::Intent(if composing.is_some() {
                Intent::Composition(text.into())
            } else {
                Intent::Insert(text.into())
            }),
            window,
            cx,
        );
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        selected: Option<std::ops::Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.is_empty() {
            self.composition = None;
            self.refresh(window, cx);
            return;
        }
        let follows_pending = self
            .composition
            .as_ref()
            .map_or(range.is_none() && self.is_pending(), |composition| {
                composition.follows_pending
            });
        let range = if let Some(range) = range {
            let Some(target) = self.native_range(range) else {
                return;
            };
            target
        } else {
            self.composition.as_ref().map_or_else(
                || self.document.selection().range(),
                |value| value.range.clone(),
            )
        };
        let normalized = moino_text::normalize_newlines(text);
        let len = normalized.encode_utf16().count();
        let preedit = TextBuffer::new(&normalized);
        let selected = selected
            .filter(|value| {
                value.start <= value.end
                    && value.end <= len
                    && preedit
                        .utf16_to_scalar(Utf16Offset::new(value.start))
                        .is_ok()
                    && preedit.utf16_to_scalar(Utf16Offset::new(value.end)).is_ok()
            })
            .unwrap_or(len..len);
        self.composition = Some(Composition {
            range,
            text: normalized,
            selection: selected,
            follows_pending,
            revision: self.document.snapshot().revision(),
        });
        self.reveal_caret = true;
        self.refresh(window, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let at = self
            .display_text()
            .utf16_to_scalar(Utf16Offset::new(range.start))
            .ok()?;
        self.caret(at)
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
            .map(|at| at.value())
    }
    fn set_selected_text_range(
        &mut self,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.display_text();
        if let (Ok(start), Ok(end)) = (
            text.utf16_to_scalar(Utf16Offset::new(range.start)),
            text.utf16_to_scalar(Utf16Offset::new(range.end)),
        ) && start <= end
        {
            self.select(selection(start, end), window, cx);
        }
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let text = self.display_text();
        Some(text.to_utf16(text.len()).ok()?.value())
    }
}
