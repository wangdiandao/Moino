//! Isolated M0 mounting probe; production blocks remain owned by the live document.
mod input;
mod render;
#[cfg(test)]
mod tests;

use crate::native_input::{PreeditSession, native_range};
use gpui_kit::base::text::editor_host::LeafGeometry;
use gpui_kit::component::input::EditorState;
use gpui_kit::{prelude::*, *};
use moino_text::TextBuffer;
use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

#[derive(Default)]
enum Mount {
    #[default]
    Static,
    Editing(PreeditSession),
}

/// Static-to-Editor probe with one retained widget and an explicit mounting boundary.
pub struct DedicatedProbe {
    editor: Entity<EditorState>,
    source: SharedString,
    mount: Mount,
    selection: UTF16Selection,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
}

impl DedicatedProbe {
    /// Creates a literal source sample, independent from Markdown projection.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source: SharedString = "let 墨 = 42;\n// 日本語 한국어 😀".into();
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(false)
                .folding(false)
                .default_value(source.clone())
        });
        Self {
            editor,
            source,
            mount: Mount::Static,
            selection: UTF16Selection {
                range: 0..0,
                reversed: false,
            },
            geometry: Rc::default(),
        }
    }
    fn is_active(&self) -> bool {
        matches!(self.mount, Mount::Editing(_))
    }
    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_active() {
            self.leave(window, cx);
        } else {
            self.enter(window, cx);
        }
    }
    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_active() {
            return;
        }
        let text = TextBuffer::new(self.source.as_ref());
        let range =
            native_range(&text, self.selection.range.clone()).expect("retained probe selection");
        let start = text
            .to_byte(range.start)
            .expect("probe selection start")
            .value();
        let end = text
            .to_byte(range.end)
            .expect("probe selection end")
            .value();
        self.editor.update(cx, |editor, cx| {
            if editor.value() != self.source {
                editor.set_value(self.source.clone(), window, cx);
            }
            editor.set_selected_range(
                if self.selection.reversed {
                    end..start
                } else {
                    start..end
                },
                cx,
            );
            editor.focus(window, cx);
        });
        self.mount = Mount::Editing(PreeditSession::default());
        self.geometry.borrow_mut().clear();
        cx.notify();
    }
    fn finish_preedit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Mount::Editing(preedit) = &mut self.mount {
            preedit.finish();
        }
        self.editor
            .update(cx, |editor, cx| editor.unmark_text(window, cx));
    }
    fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_active() {
            return;
        }
        self.finish_preedit(window, cx);
        self.source = self.editor.read(cx).value();
        self.selection = self
            .editor
            .update(cx, |editor, cx| {
                editor.selected_text_range(false, window, cx)
            })
            .expect("mounted widget selection");
        self.mount = Mount::Static;
        self.geometry.borrow_mut().clear();
        cx.notify();
    }
    fn before_action<A: Action>(&mut self, _: &A, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_preedit(window, cx);
        cx.propagate();
    }
    fn replacement(
        &self,
        requested: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        if !self.is_active() {
            return None;
        }
        self.editor.update(cx, |editor, cx| {
            let range = requested
                .or_else(|| editor.marked_text_range(window, cx))
                .or_else(|| {
                    editor
                        .selected_text_range(false, window, cx)
                        .map(|selected| selected.range)
                })?;
            native_range(&TextBuffer::new(editor.value().as_ref()), range.clone())?;
            Some(range)
        })
    }
}
