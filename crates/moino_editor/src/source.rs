//! Full-source Kit Editor adapter; the document remains the sole history owner.
use crate::LiveEditor;
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::{prelude::*, *};
use moino_document::{Edit, Origin, Selection, Snapshot, Transaction};
use moino_text::{ScalarOffset, TextBuffer, Utf16Offset};

pub(super) fn transaction(
    snapshot: &Snapshot,
    text: &str,
    selection: Selection,
) -> Option<Transaction> {
    let old: Vec<_> = snapshot.text().to_string().chars().collect();
    let new: Vec<_> = text.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    if prefix == old.len() && prefix == new.len() {
        return None;
    }
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let inserted: String = new[prefix..new.len() - suffix].iter().collect();
    Some(
        Transaction::new(snapshot, Origin::Command)
            .with_edit(Edit::new(
                ScalarOffset::new(prefix)..ScalarOffset::new(old.len() - suffix),
                &inserted,
            ))
            .with_selection(selection),
    )
}

/// Retains the full-source widget while routing committed edits to the same document.
pub struct SourceEditor {
    state: Entity<EditorState>,
    document: Entity<LiveEditor>,
    subscriptions: Vec<Subscription>,
    published: String,
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
        };
        this.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            |this, state, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let marked = state.update(cx, |state, cx| state.marked_text_range(window, cx));
                    if marked.is_none() {
                        this.publish(window, cx);
                    }
                }
            },
        ));
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
            }
            state.set_highlighter_factory(
                std::rc::Rc::new(move |_| {
                    Some(Box::new(super::source_highlight::SourceHighlighter::new(
                        &snapshot,
                    )))
                }),
                cx,
            );
        });
        self.published = self.state.read(cx).value().to_string();
    }
    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.state.read(cx).value().to_string();
        self.published = source.clone();
        let Some(selected) = self
            .state
            .update(cx, |state, cx| state.selected_text_range(false, window, cx))
        else {
            return;
        };
        let text = TextBuffer::new(&source);
        if let (Ok(start), Ok(end)) = (
            text.utf16_to_scalar(Utf16Offset::new(selected.range.start)),
            text.utf16_to_scalar(Utf16Offset::new(selected.range.end)),
        ) {
            let (anchor, head) = if selected.reversed {
                (end, start)
            } else {
                (start, end)
            };
            self.document.update(cx, |document, cx| {
                document.replace_source(source, super::live::selection(anchor, head), window, cx)
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
    }
    /// Commits native preedit and transfers the source selection before leaving.
    pub fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, cx| state.unmark_text(window, cx));
        self.publish(window, cx);
    }
}
impl Render for SourceEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme;
        div()
            .id("source-surface")
            .test_support()
            .size_full()
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
                    .appearance(false)
                    .bordered(false)
                    .context_menu(|menu, _, _| menu)
                    .h_full()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(moino_theme::code_size())
                    .line_height(relative(1.6))
                    .aria_label("Markdown source"),
            )
    }
}
