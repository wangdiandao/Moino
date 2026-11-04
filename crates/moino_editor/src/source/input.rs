//! Preserves Kit input geometry while publishing every native composition boundary.
use super::SourceEditor;
use crate::native_input::{NormalizedInput, native_range};
use gpui_kit::component::input::EditorState;
use gpui_kit::{prelude::*, *};
use moino_text::TextBuffer;
use std::ops::Range;

impl SourceEditor {
    fn replacement_range(
        &self,
        requested: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.state.update(cx, |state, cx| {
            let range = requested
                .or_else(|| state.marked_text_range(window, cx))
                .or_else(|| {
                    state
                        .selected_text_range(false, window, cx)
                        .map(|value| value.range)
                })?;
            native_range(&TextBuffer::new(state.value().as_ref()), range.clone())?;
            Some(range)
        })
    }

    fn begin_preedit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.composition.is_active() {
            return true;
        }
        // Flush ordinary edits before retaining the cancellation baseline. They
        // belong to document history even when native events arrive in one turn.
        self.publish(window, cx);
        let original = self.state.update(cx, |state, cx| {
            Some((
                state.value().to_string(),
                state.selected_text_range(false, window, cx)?,
            ))
        });
        original.is_some_and(|(text, selection)| self.composition.begin(&text, selection))
    }

    fn cancel_preedit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(original) = self.composition.cancel() else {
            return;
        };
        let text = TextBuffer::new(&original.text);
        let range = native_range(&text, original.selection.range).expect("composition baseline");
        let start = text
            .to_byte(range.start)
            .expect("composition anchor")
            .value();
        let end = text.to_byte(range.end).expect("composition focus").value();
        self.state.update(cx, |state, cx| {
            state.unmark_text(window, cx);
            state.set_value(original.text, window, cx);
            state.set_selected_range(
                if original.selection.reversed {
                    end..start
                } else {
                    start..end
                },
                cx,
            );
        });
        self.changes.set(None);
        self.bridge.command(moino_document::Origin::Typing);
        self.normalized_selection = None;
        self.publish_selection(window, cx);
    }
}

pub(super) fn native_input(
    source: Entity<SourceEditor>,
    state: Entity<EditorState>,
) -> impl IntoElement {
    crate::native_input::editor_input(source, state)
}

impl EntityInputHandler for SourceEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        native_range(
            &TextBuffer::new(self.state.read(cx).value().as_ref()),
            range.clone(),
        )?;
        self.state.update(cx, |state, cx| {
            state.text_for_range(range, adjusted, window, cx)
        })
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.state.update(cx, |state, cx| {
            state.selected_text_range(ignore_disabled_input, window, cx)
        })
    }

    fn marked_text_range(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.state
            .update(cx, |state, cx| state.marked_text_range(window, cx))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(marked) = self.marked_text_range(window, cx) {
            self.observe_composition(marked);
            self.leave(window, cx);
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.document.read(cx).is_read_only() {
            return;
        }
        let Some(range) = self.replacement_range(range, window, cx) else {
            return;
        };
        let input = NormalizedInput::new(text, None).expect("text without a supplied selection");
        if let Some(marked) = self.marked_text_range(window, cx) {
            self.observe_composition(marked);
        }
        // Native character delivery starts its own ADD event. A previous
        // no-op widget command must not reclassify this input as that command.
        self.bridge.command(moino_document::Origin::Typing);
        self.state.update(cx, |state, cx| {
            state.replace_text_in_range(Some(range), input.text(), window, cx)
        });
        self.publish(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.document.read(cx).is_read_only() {
            return;
        }
        if text.is_empty() {
            self.cancel_preedit(window, cx);
            return;
        }
        let Some(input) = NormalizedInput::new(text, selection) else {
            return;
        };
        let Some(range) = self.replacement_range(range, window, cx) else {
            return;
        };
        if !self.begin_preedit(window, cx) {
            return;
        }
        let marked = self.state.update(cx, |state, cx| {
            state.replace_and_mark_text_in_range(
                Some(range),
                input.text(),
                Some(input.selection()),
                window,
                cx,
            );
            state.marked_text_range(window, cx)
        });
        if let Some(marked) = marked {
            self.composition.update_marked(marked);
        }
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        native_range(
            &TextBuffer::new(self.state.read(cx).value().as_ref()),
            range.clone(),
        )?;
        self.state.update(cx, |state, cx| {
            state.bounds_for_range(range, state.text_bounds().unwrap_or(bounds), window, cx)
        })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.state.update(cx, |state, cx| {
            state.character_index_for_point(point, window, cx)
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = TextBuffer::new(self.state.read(cx).value().as_ref());
        let Some(selected) = native_range(&text, range) else {
            return;
        };
        self.leave(window, cx);
        self.state.update(cx, |state, cx| {
            state.set_selected_range(
                text.to_byte(selected.start)
                    .expect("validated native start")
                    .value()
                    ..text
                        .to_byte(selected.end)
                        .expect("validated native end")
                        .value(),
                cx,
            );
        });
        self.publish_selection(window, cx);
    }

    fn accepts_text_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.state
            .update(cx, |state, cx| state.accepts_text_input(window, cx))
    }

    fn text_input_configuration(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TextInputConfiguration {
        self.state.update(cx, |state, cx| {
            EntityInputHandler::text_input_configuration(state, window, cx)
        })
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        self.before_command(moino_document::Origin::Paste, window, cx);
        self.state
            .update(cx, |state, cx| state.paste(item, window, cx));
    }
}
