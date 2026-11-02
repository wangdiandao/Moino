//! Full-source Kit Editor adapter; the document remains the sole history owner.
use crate::LiveEditor;
use crate::source_change::SourceChange;
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::{prelude::*, *};
use moino_document::{Edit, Origin, Selection, Snapshot, Transaction};
use moino_text::ByteOffset;
use moino_text::{ScalarOffset, TextBuffer, Utf16Offset};
use std::{cell::Cell, rc::Rc};

fn normalized_position(source: &str, offset: Utf16Offset) -> Option<ScalarOffset> {
    let mut units = 0;
    for (byte, character) in source.char_indices() {
        if units == offset.value() {
            return Some(TextBuffer::new(&source[..byte]).len());
        }
        units += character.len_utf16();
    }
    (units == offset.value()).then(|| TextBuffer::new(source).len())
}

pub(super) fn transaction(
    snapshot: &Snapshot,
    text: &str,
    selection: Selection,
    change: Option<SourceChange>,
    origin: Origin,
) -> Option<Transaction> {
    let old = snapshot.text().to_string();
    if old == text {
        return None;
    }
    let change = change
        .filter(|change| {
            old.get(..change.start.value()) == text.get(..change.start.value())
                && old.get(change.old_end.value()..) == text.get(change.new_end.value()..)
                && old.is_char_boundary(change.start.value())
                && old.is_char_boundary(change.old_end.value())
                && text.is_char_boundary(change.start.value())
                && text.is_char_boundary(change.new_end.value())
                && change.start <= change.old_end
                && change.start <= change.new_end
        })
        .unwrap_or(SourceChange {
            start: ByteOffset::new(0),
            old_end: ByteOffset::new(old.len()),
            new_end: ByteOffset::new(text.len()),
        });
    let start = snapshot
        .text()
        .byte_to_scalar(change.start)
        .expect("validated source edit");
    let end = snapshot
        .text()
        .byte_to_scalar(change.old_end)
        .expect("validated source edit");
    let inserted = &text[change.start.value()..change.new_end.value()];
    Some(
        Transaction::new(snapshot, origin)
            .with_edit(Edit::new(start..end, inserted))
            .with_selection(selection),
    )
}

/// Retains the full-source widget while routing committed edits to the same document.
pub struct SourceEditor {
    state: Entity<EditorState>,
    document: Entity<LiveEditor>,
    subscriptions: Vec<Subscription>,
    published: String,
    changes: Rc<Cell<Option<SourceChange>>>,
    origin: Origin,
    composing: bool,
    normalized_selection: Option<Selection>,
    published_selection: Selection,
}
impl SourceEditor {
    /// Creates a source surface for the supplied document session.
    pub fn new(document: Entity<LiveEditor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = document.read(cx).snapshot().text().to_string();
        let state = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(false)
                .folding(false)
                .default_value(source.clone())
        });
        let mut this = Self {
            state: state.clone(),
            document: document.clone(),
            subscriptions: Vec::new(),
            published: source,
            changes: Rc::default(),
            origin: Origin::Typing,
            composing: false,
            normalized_selection: None,
            published_selection: document.read(cx).selection(),
        };
        this.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            |this, state, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let marked = state.update(cx, |state, cx| state.marked_text_range(window, cx));
                    if marked.is_none() {
                        this.publish(window, cx);
                    } else {
                        this.composing = true;
                    }
                }
            },
        ));
        this.subscriptions
            .push(cx.observe_in(&state, window, |this, _, window, cx| {
                this.publish_selection(window, cx);
            }));
        this.subscriptions
            .push(cx.observe_in(&document, window, |this, _, window, cx| {
                if !this.document.read(cx).is_pending() {
                    this.sync(window, cx);
                }
            }));
        this.sync(window, cx);
        this
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.document.read(cx).error_code().is_some() {
            return;
        }
        if self.state.read(cx).value().as_ref() != self.published {
            return;
        }
        let snapshot = self.document.read(cx).snapshot();
        let selected = self.document.read(cx).selection();
        let source = snapshot.text().to_string();
        let selection_changed = self.published_selection != selected;
        let range = snapshot
            .text()
            .to_byte(selected.anchor().offset)
            .expect("validated selection")
            .value()
            ..snapshot
                .text()
                .to_byte(selected.head().offset)
                .expect("validated selection")
                .value();
        self.state.update(cx, |state, cx| {
            if state.value().as_ref() != source {
                state.set_value(source, window, cx);
                state.set_selected_range(range, cx);
            } else if selection_changed {
                state.set_selected_range(range, cx);
            }
            let changes = self.changes.clone();
            state.set_highlighter_factory(
                std::rc::Rc::new(move |_| {
                    Some(Box::new(super::source_highlight::SourceHighlighter::new(
                        &snapshot,
                        changes.clone(),
                    )))
                }),
                cx,
            );
        });
        self.published = self.state.read(cx).value().to_string();
        self.published_selection = selected;
        self.changes.set(None);
    }
    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.state.read(cx).value().to_string();
        let Some(selected) = self
            .state
            .update(cx, |state, cx| state.selected_text_range(false, window, cx))
        else {
            return;
        };
        let text = TextBuffer::new(&raw);
        let source = text.to_string();
        let change = self.changes.take().map(|mut change| {
            if raw != source
                && let Some(prefix) = raw.get(..change.new_end.value())
            {
                change.new_end = ByteOffset::new(moino_text::normalize_newlines(prefix).len());
            }
            change
        });
        let origin = if self.composing {
            Origin::Composition
        } else {
            self.origin
        };
        self.composing = false;
        self.origin = Origin::Typing;
        let position = |offset| {
            if raw == source {
                text.utf16_to_scalar(Utf16Offset::new(offset)).ok()
            } else {
                normalized_position(&raw, Utf16Offset::new(offset))
            }
        };
        if let (Some(start), Some(end)) =
            (position(selected.range.start), position(selected.range.end))
        {
            let (anchor, head) = if selected.reversed {
                (end, start)
            } else {
                (start, end)
            };
            self.published = source.clone();
            if raw != source {
                self.state.update(cx, |state, cx| {
                    state.set_value(source.clone(), window, cx);
                    state.set_selected_range(
                        text.to_byte(anchor).expect("normalized anchor").value()
                            ..text.to_byte(head).expect("normalized head").value(),
                        cx,
                    );
                });
            }
            self.published_selection = super::live::selection(anchor, head);
            self.document.update(cx, |document, cx| {
                document.replace_source_edit(
                    source,
                    super::live::selection(anchor, head),
                    change,
                    origin,
                    window,
                    cx,
                )
            });
        }
    }
    fn publish_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.read(cx).value().as_ref() != self.published || self.composing {
            return;
        }
        let selected = self.state.update(cx, |state, cx| {
            let selected = state.selected_text_range(false, window, cx)?;
            let text = TextBuffer::new(state.value().as_ref());
            let start = text
                .utf16_to_scalar(Utf16Offset::new(selected.range.start))
                .ok()?;
            let end = text
                .utf16_to_scalar(Utf16Offset::new(selected.range.end))
                .ok()?;
            let anchor = if selected.reversed { end } else { start };
            let head = if selected.reversed { start } else { end };
            let selection = super::live::selection(anchor, head);
            if self.normalized_selection != Some(selection) {
                let anchor_byte = text.to_byte(anchor).ok()?.value();
                let head_byte = text.to_byte(head).ok()?.value();
                state.set_selected_range(anchor_byte..head_byte, cx);
                self.normalized_selection = Some(selection);
            }
            Some(selection)
        });
        if let Some(selected) = selected
            && selected != self.published_selection
        {
            self.published_selection = selected;
            self.document.update(cx, |document, cx| {
                document.select_source_after_pending(selected, window, cx)
            });
        }
    }
    /// Focuses the source widget with the shared document selection.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync(window, cx);
        let snapshot = self.document.read(cx).snapshot();
        let selected = self.document.read(cx).selection();
        self.state.update(cx, |state, cx| {
            let start = snapshot
                .text()
                .to_byte(selected.anchor().offset)
                .expect("selection")
                .value();
            let end = snapshot
                .text()
                .to_byte(selected.head().offset)
                .expect("selection")
                .value();
            state.set_selected_range(start..end, cx);
            state.focus(window, cx);
        });
        self.document.update(cx, |document, cx| {
            document.select_source(selected, window, cx)
        });
    }
    /// Commits native preedit and transfers the source selection before leaving.
    pub fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, cx| state.unmark_text(window, cx));
        self.publish(window, cx);
    }
    fn before_command(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.leave(window, cx);
        self.origin = origin;
    }
}
impl Render for SourceEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme;
        div()
            .id("source-surface")
            .test_support()
            .key_context("MoinoSource")
            .size_full()
            .relative()
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Copy, window, cx| {
                    this.leave(window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Backspace, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Delete, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Indent, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Outdent, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                },
            ))
            .on_action(cx.listener(|this, _: &crate::Undo, window, cx| {
                this.leave(window, cx);
                this.document
                    .update(cx, |document, cx| document.undo(window, cx));
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &crate::Redo, window, cx| {
                this.leave(window, cx);
                this.document
                    .update(cx, |document, cx| document.redo(window, cx));
                cx.stop_propagation();
            }))
            .capture_key_down(cx.listener(|this, _, window, cx| {
                this.normalized_selection = None;
                this.publish_selection(window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.publish_selection(window, cx)),
            )
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Paste, window, cx| {
                    this.before_command(Origin::Paste, window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::Cut, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                }),
            )
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Enter, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Undo, window, cx| {
                    this.leave(window, cx);
                    this.document
                        .update(cx, |document, cx| document.undo(window, cx));
                    cx.stop_propagation();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Redo, window, cx| {
                    this.leave(window, cx);
                    this.document
                        .update(cx, |document, cx| document.redo(window, cx));
                    cx.stop_propagation();
                },
            ))
            .child(
                Editor::new(&self.state)
                    .readonly(self.document.read(cx).is_read_only())
                    .appearance(false)
                    .bordered(false)
                    .context_menu(|menu, _, _| menu)
                    .h_full()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(moino_theme::code_size())
                    .line_height(relative(1.6))
                    .aria_label("Markdown source"),
            )
            .when_some(self.document.read(cx).error_code(), |surface, code| {
                let message = moino_ui::editing_error(code);
                surface.child(
                    div()
                        .id("source-error")
                        .test_support()
                        .role(Role::Alert)
                        .aria_label(message.clone())
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().danger)
                        .px_3()
                        .py_2()
                        .child(message),
                )
            })
    }
}

#[cfg(test)]
mod tests;
