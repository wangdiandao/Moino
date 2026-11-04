//! Native events are owned by the mounted probe, never its hidden widget.
use super::*;
use crate::native_input::NormalizedInput;

impl EntityInputHandler for DedicatedProbe {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.replacement(Some(range.clone()), window, cx)?;
        self.editor.update(cx, |editor, cx| {
            editor.text_for_range(range, adjusted, window, cx)
        })
    }
    fn selected_text_range(
        &mut self,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if !self.is_active() {
            return None;
        }
        self.editor.update(cx, |editor, cx| {
            editor.selected_text_range(disabled, window, cx)
        })
    }
    fn marked_text_range(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        if !self.is_active() {
            return None;
        }
        self.editor
            .update(cx, |editor, cx| editor.marked_text_range(window, cx))
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_preedit(window, cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(range) = self.replacement(range, window, cx) else {
            return;
        };
        let input = NormalizedInput::new(text, None).expect("native text without selection");
        self.editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(Some(range), input.text(), window, cx)
        });
        self.finish_preedit(window, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = NormalizedInput::new(text, selected) else {
            return;
        };
        let Some(range) = self.replacement(range, window, cx) else {
            return;
        };
        let Mount::Editing(preedit) = &mut self.mount else {
            return;
        };
        if text.is_empty() {
            let Some(original) = preedit.cancel() else {
                return;
            };
            let buffer = TextBuffer::new(&original.text);
            let selected = native_range(&buffer, original.selection.range)
                .expect("retained preedit selection");
            let start = buffer
                .to_byte(selected.start)
                .expect("preedit selection start")
                .value();
            let end = buffer
                .to_byte(selected.end)
                .expect("preedit selection end")
                .value();
            self.editor.update(cx, |editor, cx| {
                editor.unmark_text(window, cx);
                editor.set_value(original.text, window, cx);
                editor.set_selected_range(
                    if original.selection.reversed {
                        end..start
                    } else {
                        start..end
                    },
                    cx,
                );
            });
            return;
        }
        let Some(selected) = self.editor.update(cx, |editor, cx| {
            editor.selected_text_range(false, window, cx)
        }) else {
            return;
        };
        if !preedit.begin(self.editor.read(cx).value().as_ref(), selected) {
            return;
        }
        let marked = self.editor.update(cx, |editor, cx| {
            editor.replace_and_mark_text_in_range(
                Some(range),
                input.text(),
                Some(input.selection()),
                window,
                cx,
            );
            editor.marked_text_range(window, cx)
        });
        if let Some(marked) = marked {
            preedit.update_marked(marked);
        }
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.replacement(Some(range.clone()), window, cx)?;
        self.editor.update(cx, |editor, cx| {
            editor.bounds_for_range(range, editor.text_bounds().unwrap_or(bounds), window, cx)
        })
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        if !self.is_active() {
            return None;
        }
        self.editor.update(cx, |editor, cx| {
            editor.character_index_for_point(point, window, cx)
        })
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.replacement(Some(range.clone()), window, cx).is_none() {
            return;
        }
        let text = TextBuffer::new(self.editor.read(cx).value().as_ref());
        let selected = native_range(&text, range).expect("validated native selection");
        let start = text
            .to_byte(selected.start)
            .expect("native selection start")
            .value();
        let end = text
            .to_byte(selected.end)
            .expect("native selection end")
            .value();
        self.finish_preedit(window, cx);
        self.editor
            .update(cx, |editor, cx| editor.set_selected_range(start..end, cx));
    }
    fn accepts_text_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.is_active()
            && self
                .editor
                .update(cx, |editor, cx| editor.accepts_text_input(window, cx))
    }
    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        self.editor.update(cx, |editor, cx| {
            EntityInputHandler::text_input_configuration(editor, window, cx)
        })
    }
    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_active() {
            return;
        }
        self.finish_preedit(window, cx);
        self.editor
            .update(cx, |editor, cx| editor.paste(item, window, cx));
    }
}
