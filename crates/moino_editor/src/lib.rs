//! L2 native live/source editing, block projection and input geometry over the document core.
mod accessibility;
pub mod commands;
mod dedicated;
mod input;
mod live;
mod live_surface;
mod native_input;
mod projection;
mod source;
mod source_change;
mod source_highlight;
pub use live::{LiveEditor, Redo, Undo};
pub use source::SourceEditor;
mod surface;
pub use dedicated::DedicatedProbe;

use gpui_kit::base::text::editor_host::{
    BlockNode, FlowPosition, InlineNode, LeafGeometry, Paragraph, RenderContext, TextMark,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};
use moino_text::{TextBuffer, Utf16Offset};
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
    selection_reversed: bool,
    composition: native_input::PreeditSession,
    focus: FocusHandle,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
    accessible_runs: Rc<RefCell<accessibility::TextRuns>>,
}

impl PrototypeEditor {
    /// Builds the prototype and its stable focus/geometry ownership.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            text: "墨 Moino · 中文 日本語 한국어 😀".into(),
            selection: 0..0,
            selection_reversed: false,
            composition: native_input::PreeditSession::default(),
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

    fn finish_composition(&mut self) {
        self.composition.finish();
    }

    fn adjacent_grapheme(&self, offset: usize, forward: bool) -> Option<usize> {
        let text = TextBuffer::new(&self.text);
        let at = text.utf16_to_scalar(Utf16Offset::new(offset)).ok()?;
        let next = if forward {
            text.next_grapheme(at)
        } else {
            text.previous_grapheme(at)
        }
        .ok()?;
        text.to_utf16(next).ok().map(Utf16Offset::value)
    }

    fn edit(&mut self, range: Option<Range<usize>>, text: &str) -> Option<Range<usize>> {
        let target = range
            .or_else(|| self.composition.marked_range())
            .unwrap_or_else(|| self.selection.clone());
        let buffer = TextBuffer::new(&self.text);
        let range = native_input::native_range(&buffer, target.clone())?;
        let start = buffer.to_byte(range.start).ok()?.value();
        let end = buffer.to_byte(range.end).ok()?.value();
        let input = native_input::NormalizedInput::new(text, None)?;
        self.text.replace_range(start..end, input.text());
        let cursor = target.start + input.selection().end;
        self.selection = cursor..cursor;
        self.selection_reversed = false;
        Some(target.start..cursor)
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition();
        if self.selection.is_empty() && self.selection.start > 0 {
            let Some(previous) = self.adjacent_grapheme(self.selection.start, false) else {
                return;
            };
            self.selection.start = previous;
        }
        if self.edit(None, "").is_some() {
            self.changed(window, cx);
        }
    }
    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition();
        if self.edit(None, "\n").is_some() {
            self.changed(window, cx);
        }
    }
    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition();
        let at = if self.selection.is_empty() {
            self.adjacent_grapheme(self.selection.start, false)
        } else {
            Some(self.selection.start)
        };
        if let Some(at) = at {
            self.selection = at..at;
            self.selection_reversed = false;
            self.changed(window, cx);
        }
    }
    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition();
        let at = if self.selection.is_empty() {
            self.adjacent_grapheme(self.selection.end, true)
        } else {
            Some(self.selection.end)
        };
        if let Some(at) = at {
            self.selection = at..at;
            self.selection_reversed = false;
            self.changed(window, cx);
        }
    }
}

/// Registers the prototype's input actions once at application startup.
pub fn init(cx: &mut App) {
    live::init(cx);
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
        let mut marks: Vec<_> = self
            .text
            .find("Moino")
            .map(|start| (start..start + "Moino".len(), TextMark::default().bold()))
            .into_iter()
            .collect();
        if let Some(range) = self.composition.marked_range()
            && let (Some(start), Some(end)) = (
                byte_from_utf16(&self.text, range.start),
                byte_from_utf16(&self.text, range.end),
            )
        {
            marks.push((start..end, TextMark::default().underline()));
        }
        paragraph.push(
            InlineNode::new(if self.text.is_empty() {
                " ".into()
            } else {
                self.text.clone()
            })
            .marks(marks),
        );
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
        let accessibility_reversed = self.selection_reversed;
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
                            accessibility_reversed,
                            geometry,
                        );
                    }
                })
                .on_a11y_action(accesskit::Action::SetValue, move |data, window, cx| {
                    if let Some(accesskit::ActionData::Value(value)) = data {
                        value_owner.update(cx, |editor, cx| {
                            let end = editor.text.encode_utf16().count();
                            if editor.edit(Some(0..end), value).is_some() {
                                editor.finish_composition();
                                editor.changed(window, cx);
                            }
                        });
                    }
                })
                .on_a11y_action(
                    accesskit::Action::SetTextSelection,
                    move |data, window, cx| {
                        if let Some(accesskit::ActionData::SetTextSelection(selection)) = data {
                            selection_owner.update(cx, |editor, cx| {
                                let selection = accessibility::native_selection(
                                    &editor.text,
                                    selection,
                                    &editor.accessible_runs.borrow(),
                                );
                                if let Some((range, reversed)) = selection {
                                    editor.finish_composition();
                                    editor.selection = range;
                                    editor.selection_reversed = reversed;
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
                        this.finish_composition();
                        if let Some(geometry) = this.geometry.borrow().get(&1) {
                            let byte = geometry
                                .index_for_position(event.position)
                                .min(this.text.len());
                            if this.text.is_char_boundary(byte) {
                                let offset = this.text[..byte].encode_utf16().count();
                                this.selection = offset..offset;
                                this.selection_reversed = false;
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
