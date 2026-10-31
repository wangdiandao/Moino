//! L2 editor_host input and geometry prototype; only GPUI Kit is used at this seam.
mod accessibility;
mod dedicated;
mod input;
mod surface;
pub use dedicated::DedicatedProbe;

use gpui_kit::base::text::editor_host::{
    BlockNode, FlowPosition, InlineNode, LeafGeometry, Paragraph, RenderContext,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

actions!(
    probe,
    [
        /// Deletes text before the prototype caret.
        Backspace,
        /// Inserts a line break in the prototype.
        Enter,
        /// Moves the prototype caret left.
        Left,
        /// Moves the prototype caret right.
        Right
    ]
);

/// Retained host-owned editable paragraph used to validate the native text seam.
pub struct PrototypeEditor {
    text: String,
    selection: Range<usize>,
    marked: Option<Range<usize>>,
    original_composition: Option<(String, Range<usize>)>,
    focus: FocusHandle,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
    accessible_runs: Rc<RefCell<HashMap<accesskit::NodeId, Range<usize>>>>,
}

impl PrototypeEditor {
    /// Builds the prototype and its stable focus/geometry ownership.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            text: "墨 Moino · 中文 日本語 한국어 😀".into(),
            selection: 0..0,
            marked: None,
            original_composition: None,
            focus: cx.focus_handle().tab_stop(true),
            geometry: Rc::default(),
            accessible_runs: Rc::default(),
        }
    }
    /// Current host text, independent of rendered leaves.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Transfers keyboard ownership to this surface.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_input_changed(&self.focus);
        cx.notify();
    }

    fn edit(&mut self, range: Option<Range<usize>>, text: &str) {
        let target = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let Some(start) = byte_from_utf16(&self.text, target.start) else {
            return;
        };
        let Some(end) = byte_from_utf16(&self.text, target.end) else {
            return;
        };
        if start > end {
            return;
        }
        self.text
            .replace_range(start..end, &text.replace("\r\n", "\n").replace('\r', "\n"));
        let cursor = target.start
            + text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .encode_utf16()
                .count();
        self.selection = cursor..cursor;
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() && self.selection.start > 0 {
            let Some(byte) = byte_from_utf16(&self.text, self.selection.start) else {
                return;
            };
            if let Some(ch) = self.text[..byte].chars().next_back() {
                self.selection.start -= ch.len_utf16();
            }
        }
        self.edit(None, "");
        self.changed(window, cx);
    }
    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(None, "\n");
        self.changed(window, cx);
    }
    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(byte) = byte_from_utf16(&self.text, self.selection.start) {
            let at = self.selection.start.saturating_sub(
                self.text[..byte]
                    .chars()
                    .next_back()
                    .map_or(0, char::len_utf16),
            );
            self.selection = at..at;
            self.changed(window, cx);
        }
    }
    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(byte) = byte_from_utf16(&self.text, self.selection.end) {
            let at =
                self.selection.end + self.text[byte..].chars().next().map_or(0, char::len_utf16);
            self.selection = at..at;
            self.changed(window, cx);
        }
    }
}

/// Registers the prototype's input actions once at application startup.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("MoinoPrototype")),
        KeyBinding::new("enter", Enter, Some("MoinoPrototype")),
        KeyBinding::new("left", Left, Some("MoinoPrototype")),
        KeyBinding::new("right", Right, Some("MoinoPrototype")),
    ]);
}

impl Render for PrototypeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut paragraph = Paragraph::default().with_leaf_key(1);
        paragraph.push(InlineNode::new(if self.text.is_empty() {
            " ".into()
        } else {
            self.text.clone()
        }));
        let block = BlockNode::Paragraph(paragraph);
        let render_context = RenderContext::from_app(cx);
        let child = block.render_with_id(
            "paragraph",
            FlowPosition::at(&[&block], 0, true),
            &render_context,
            window,
            cx,
        );
        let accessibility_text = self.text.clone();
        let accessibility_selection = self.selection.clone();
        let accessibility_geometry = self.geometry.clone();
        let runs = self.accessible_runs.clone();
        let selection_owner = cx.entity();
        let value_owner = cx.entity();
        surface::HostSurface::new(
            div()
                .id("live-editor")
                .test_support()
                .accessibility_id("moino-live-editor")
                .role(Role::MultilineTextInput)
                .aria_label("Document")
                .aria_value(self.text.clone())
                .a11y_synthetic_children(move |builder| {
                    if let Some(geometry) = accessibility_geometry.borrow().get(&1) {
                        *runs.borrow_mut() = accessibility::build_runs(
                            builder,
                            &accessibility_text,
                            &accessibility_selection,
                            geometry,
                        );
                    }
                })
                .on_a11y_action(accesskit::Action::SetValue, move |data, window, cx| {
                    if let Some(accesskit::ActionData::Value(value)) = data {
                        value_owner.update(cx, |editor, cx| {
                            let end = editor.text.encode_utf16().count();
                            editor.edit(Some(0..end), value);
                            editor.changed(window, cx);
                        });
                    }
                })
                .on_a11y_action(
                    accesskit::Action::SetTextSelection,
                    move |data, window, cx| {
                        if let Some(accesskit::ActionData::SetTextSelection(selection)) = data {
                            selection_owner.update(cx, |editor, cx| {
                                if let Some(range) = accessibility::native_selection(
                                    &editor.text,
                                    selection,
                                    &editor.accessible_runs.borrow(),
                                ) {
                                    editor.selection = range;
                                }
                                editor.changed(window, cx);
                            });
                        }
                    },
                )
                .track_focus(&self.focus)
                .key_context("MoinoPrototype")
                .on_action(cx.listener(Self::backspace))
                .on_action(cx.listener(Self::enter))
                .on_action(cx.listener(Self::left))
                .on_action(cx.listener(Self::right))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        this.focus.focus(window, cx);
                        if let Some(geometry) = this.geometry.borrow().get(&1) {
                            let byte = geometry
                                .index_for_position(event.position)
                                .min(this.text.len());
                            if this.text.is_char_boundary(byte) {
                                let offset = this.text[..byte].encode_utf16().count();
                                this.selection = offset..offset;
                            }
                        }
                        this.changed(window, cx);
                    }),
                )
                .w_full()
                .min_h_24()
                .p_4()
                .text_base()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(child)
                .into_any_element(),
            cx.entity(),
            self.geometry.clone(),
        )
    }
}

fn byte_from_utf16(text: &str, offset: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Some(byte);
        }
        units += ch.len_utf16();
        if units > offset {
            return None;
        }
    }
    (units == offset).then_some(text.len())
}
