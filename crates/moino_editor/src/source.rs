//! Full-source surface: a native widget submits drafts to one authoritative document queue.
mod bridge;
mod input;
mod render;
mod transaction;
use crate::LiveEditor;
use crate::native_input::PreeditSession;
use crate::source_change::SourceChange;
use bridge::{Draft, Publication, SourceBridge};
use gpui_kit::component::input::{EditorState, InputEvent};
use gpui_kit::{prelude::*, *};
use moino_document::{Origin, Selection};
use moino_text::ByteOffset;
use std::{cell::Cell, rc::Rc};
pub(super) use transaction::transaction;

/// Owns native source input while the shared document owns revisions and history.
pub struct SourceEditor {
    state: Entity<EditorState>,
    document: Entity<LiveEditor>,
    bridge: SourceBridge,
    composition: PreeditSession,
    changes: Rc<Cell<Option<SourceChange>>>,
    normalized_selection: Option<Selection>,
    subscriptions: Vec<Subscription>,
}

impl SourceEditor {
    /// Creates one retained native widget and subscribes to its document queue.
    pub fn new(document: Entity<LiveEditor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let draft =
            Draft::from_document(&document.read(cx).snapshot(), document.read(cx).selection());
        let state = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .line_number(false)
                .folding(false)
                .default_value(draft.text.clone());
            state.set_selected_range(draft.byte_selection(), cx);
            state
        });
        let mut source = Self {
            state: state.clone(),
            document: document.clone(),
            bridge: SourceBridge::new(draft),
            composition: PreeditSession::default(),
            changes: Rc::default(),
            normalized_selection: None,
            subscriptions: Vec::new(),
        };
        source.subscriptions.push(
            cx.subscribe_in(&state, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(marked) = this.widget_marked(window, cx) {
                        this.observe_composition(marked);
                    } else {
                        this.publish(window, cx);
                    }
                }
            }),
        );
        source
            .subscriptions
            .push(cx.observe_in(&state, window, |this, _, window, cx| {
                this.publish_selection(window, cx);
            }));
        source
            .subscriptions
            .push(cx.observe_in(&document, window, |this, _, window, cx| {
                this.sync(window, cx);
            }));
        source.sync(window, cx);
        source
    }

    fn widget_marked(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        self.state
            .update(cx, |state, cx| state.marked_text_range(window, cx))
    }

    fn capture(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<Draft> {
        self.state.update(cx, |state, cx| {
            Draft::capture(
                state.value().as_ref(),
                state.selected_text_range(false, window, cx)?,
            )
        })
    }

    fn observe_composition(&mut self, marked: std::ops::Range<usize>) {
        let submitted = self.bridge.submitted();
        self.composition
            .begin(&submitted.text, submitted.native_selection());
        self.composition.update_marked(marked);
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = self.document.read(cx);
        if owner.is_pending()
            || owner.error_code().is_some()
            || self.composition.is_active()
            || self.state.read(cx).value().as_ref() != self.bridge.submitted().text
        {
            return;
        }
        let snapshot = owner.snapshot();
        let draft = Draft::from_document(&snapshot, owner.selection());
        let changed_text = self.state.read(cx).value().as_ref() != draft.text;
        // Native navigation can precede its observer. An unchanged owner
        // notification must not restore the previous caret over that new input.
        let changed_selection = self.bridge.submitted().selection != draft.selection;
        let range = draft.byte_selection();
        self.state.update(cx, |state, cx| {
            if changed_text {
                state.set_value(draft.text.clone(), window, cx);
            }
            if changed_text || changed_selection {
                state.set_selected_range(range, cx);
            }
            let changes = self.changes.clone();
            state.set_highlighter_factory(
                Rc::new(move |_| {
                    Some(Box::new(crate::source_highlight::SourceHighlighter::new(
                        &snapshot,
                        changes.clone(),
                    )))
                }),
                cx,
            );
        });
        self.normalized_selection = Some(draft.selection);
        self.bridge.adopt(draft);
        self.changes.set(None);
    }

    fn submit(
        &mut self,
        publication: Option<Publication>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match publication {
            Some(Publication::Text {
                text,
                selection,
                change,
                origin,
            }) => {
                self.document.update(cx, |document, cx| {
                    document.replace_source_edit(text, selection, change, origin, window, cx)
                });
            }
            Some(Publication::Selection(selection)) => {
                self.document.update(cx, |document, cx| {
                    document.select_source_after_pending(selection, window, cx)
                });
            }
            None => {}
        }
    }

    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.widget_marked(window, cx).is_some() {
            return;
        }
        let Some(draft) = self.capture(window, cx) else {
            return;
        };
        let raw = self.state.read(cx).value().to_string();
        let mut change = self.changes.take();
        if raw != draft.text {
            if let Some(edit) = &mut change
                && let Some(prefix) = raw.get(..edit.new_end.value())
            {
                edit.new_end = ByteOffset::new(moino_text::normalize_newlines(prefix).len());
            }
            let range = draft.byte_selection();
            self.state.update(cx, |state, cx| {
                state.set_value(draft.text.clone(), window, cx);
                state.set_selected_range(range, cx);
            });
            self.changes.set(None);
        }
        let composition = self.composition.finish();
        let publication = self.bridge.publish(draft, change, composition);
        self.submit(publication, window, cx);
    }

    fn publish_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composition.is_active()
            || self.widget_marked(window, cx).is_some()
            || self.state.read(cx).value().as_ref() != self.bridge.submitted().text
        {
            return;
        }
        let Some(draft) = self.capture(window, cx) else {
            return;
        };
        if self.normalized_selection != Some(draft.selection) {
            let range = draft.byte_selection();
            self.state
                .update(cx, |state, cx| state.set_selected_range(range, cx));
            self.normalized_selection = Some(draft.selection);
        }
        let publication = self.bridge.publish(draft, None, false);
        self.submit(publication, window, cx);
    }

    /// Restores the shared direction selection and focuses the source widget.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync(window, cx);
        let draft = Draft::from_document(
            &self.document.read(cx).snapshot(),
            self.document.read(cx).selection(),
        );
        let range = draft.byte_selection();
        self.state.update(cx, |state, cx| {
            state.set_selected_range(range, cx);
            state.focus(window, cx);
        });
        self.document.update(cx, |document, cx| {
            document.select_source(draft.selection, window, cx)
        });
    }

    /// Commits native preedit exactly once before navigation, commands or a surface switch.
    pub fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(marked) = self.widget_marked(window, cx) {
            self.observe_composition(marked);
        }
        self.state
            .update(cx, |state, cx| state.unmark_text(window, cx));
        self.publish(window, cx);
    }

    fn before_command(&mut self, origin: Origin, window: &mut Window, cx: &mut Context<Self>) {
        self.leave(window, cx);
        self.bridge.command(origin);
    }

    fn before_navigation<A: Action>(&mut self, _: &A, window: &mut Window, cx: &mut Context<Self>) {
        self.leave(window, cx);
        self.normalized_selection = None;
        cx.propagate();
    }

    fn before_edit<A: Action>(&mut self, _: &A, window: &mut Window, cx: &mut Context<Self>) {
        self.before_command(Origin::Command, window, cx);
        cx.propagate();
    }
}

#[cfg(test)]
mod tests;
