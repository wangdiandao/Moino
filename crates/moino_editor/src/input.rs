use crate::native_input::{NormalizedInput, native_range};
use crate::{PrototypeEditor, byte_from_utf16};
use gpui_kit::{
    Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, px, size,
};
use moino_text::TextBuffer;
use std::ops::Range;

impl EntityInputHandler for PrototypeEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        native_range(&TextBuffer::new(&self.text), range.clone())?;
        let start = byte_from_utf16(&self.text, range.start)?;
        let end = byte_from_utf16(&self.text, range.end)?;
        *adjusted = Some(range);
        self.text.get(start..end).map(str::to_owned)
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.selection.clone(),
            reversed: self.selection_reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.composition.marked_range()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition();
        self.changed(window, cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit(range, text).is_none() {
            return;
        }
        self.finish_composition();
        self.changed(window, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.is_empty() {
            if let Some(original) = self.composition.cancel() {
                self.text = original.text;
                self.selection = original.selection.range;
                self.selection_reversed = original.selection.reversed;
            }
        } else {
            let Some(input) = NormalizedInput::new(text, selection) else {
                return;
            };
            let target = range
                .or_else(|| self.composition.marked_range())
                .unwrap_or_else(|| self.selection.clone());
            if native_range(&TextBuffer::new(&self.text), target.clone()).is_none() {
                return;
            }
            if !self.composition.begin(
                &self.text,
                UTF16Selection {
                    range: self.selection.clone(),
                    reversed: self.selection_reversed,
                },
            ) {
                return;
            }
            let inserted = self
                .edit(Some(target), input.text())
                .expect("validated preedit replacement");
            let start = inserted.start;
            self.composition.update_marked(inserted);
            let selection = input.selection();
            self.selection = start + selection.start..start + selection.end;
        }
        self.changed(window, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        native_range(&TextBuffer::new(&self.text), range.clone())?;
        let geometry = self.geometry.borrow();
        let geometry = geometry.get(&1)?;
        let start = geometry.position_for_index(byte_from_utf16(&self.text, range.start)?);
        let end = geometry.position_for_index(byte_from_utf16(&self.text, range.end)?);
        Some(Bounds::new(
            start,
            size((end.x - start.x).max(px(1.)), geometry.caret_height()),
        ))
    }
    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let geometry = self.geometry.borrow();
        let byte = geometry
            .get(&1)?
            .index_for_position(position)
            .min(self.text.len());
        self.text
            .get(..byte)
            .map(|prefix| prefix.encode_utf16().count())
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if range.start <= range.end
            && byte_from_utf16(&self.text, range.start).is_some()
            && byte_from_utf16(&self.text, range.end).is_some()
        {
            self.finish_composition();
            self.selection = range;
            self.selection_reversed = false;
            self.changed(window, cx);
        }
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.text.encode_utf16().count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::{TestInput, TestWindowExt};
    use gpui_kit::{AppContext, ElementInputHandler, TestAppContext};

    #[test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn utf16_boundaries_do_not_split_surrogate_pairs() {
        assert_eq!(byte_from_utf16("中😀a", 1), Some(3));
        assert_eq!(byte_from_utf16("中😀a", 2), None);
        assert_eq!(byte_from_utf16("中😀a", 3), Some(7));
        assert_eq!(byte_from_utf16("中😀a", 4), Some(8));
        assert_eq!(byte_from_utf16("中😀a", 5), None);
    }

    #[gpui_kit::test]
    // Spec: docs/modules/editing.md#输入法组合
    fn navigation_finishes_composition_before_editing_and_late_cancel_preserves_text(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                let editor = cx.new(PrototypeEditor::new);
                window.focus(&editor.read(cx).focus_handle(), cx);
                editor
            })
            .expect("prototype window")
        });
        cx.update_window(handle, |_, window, cx| {
            for (key, expected) in [("left", "aX中b"), ("right", "a中bX"), ("enter", "a中\nXb")]
            {
                editor.update(cx, |editor, cx| {
                    let end = editor.text.encode_utf16().count();
                    editor.replace_text_in_range(Some(0..end), "ab", window, cx);
                });
                window.render_frame(cx);
                let bounds = window.find("live-editor").bounds();
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window)
                    .compose_text(Some(1..1), "中", Some(1..1), cx);
                window.press(key, cx);
                assert!(!editor.read(cx).composition.is_active());
                window.input("X", cx);
                assert_eq!(editor.read(cx).text(), expected);
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window)
                    .compose_text(None, "", None, cx);
                assert_eq!(editor.read(cx).text(), expected, "late cancel after {key}");
            }
        })
        .expect("composition navigation");
    }

    #[gpui_kit::test]
    // Spec: docs/modules/selection.md#逻辑选择
    fn native_probe_moves_and_deletes_extended_graphemes_and_collapses_selection(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                let editor = cx.new(PrototypeEditor::new);
                window.focus(&editor.read(cx).focus_handle(), cx);
                editor
            })
            .expect("prototype window")
        });
        cx.update_window(handle, |_, window, cx| {
            for cluster in ["e\u{301}", "👍🏽", "👨‍👩‍👧‍👦", "🇨🇳"] {
                let value = format!("a{cluster}b");
                let end = 1 + cluster.encode_utf16().count();
                editor.update(cx, |editor, cx| {
                    let old_end = editor.text.encode_utf16().count();
                    editor.replace_text_in_range(Some(0..old_end), &value, window, cx);
                    editor.set_selected_text_range(end..end, window, cx);
                });
                window.press("left", cx);
                assert_eq!(editor.read(cx).selection, 1..1);
                window.press("right", cx);
                assert_eq!(editor.read(cx).selection, end..end);
                window.press("backspace", cx);
                assert_eq!(editor.read(cx).text(), "ab");
                editor.update(cx, |editor, cx| {
                    editor.set_selected_text_range(0..1, window, cx);
                });
                window.press("right", cx);
                assert_eq!(editor.read(cx).selection, 1..1);
                editor.update(cx, |editor, cx| {
                    editor.set_selected_text_range(1..2, window, cx);
                });
                window.press("left", cx);
                assert_eq!(editor.read(cx).selection, 1..1);
            }
        })
        .expect("grapheme editing");
    }

    #[gpui_kit::test]
    // Spec: docs/modules/editing.md#输入法组合
    fn pointer_and_native_selection_finish_preedit_before_retargeting_input(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                let editor = cx.new(PrototypeEditor::new);
                window.focus(&editor.read(cx).focus_handle(), cx);
                editor
            })
            .expect("prototype window")
        });
        cx.update_window(handle, |_, window, cx| {
            for pointer in [false, true] {
                editor.update(cx, |editor, cx| {
                    let end = editor.text.encode_utf16().count();
                    editor.replace_text_in_range(Some(0..end), "ab", window, cx);
                });
                window.render_frame(cx);
                let bounds = window.find("live-editor").bounds();
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window)
                    .compose_text(Some(1..1), "中", Some(1..1), cx);
                if pointer {
                    let at = editor.read(cx).geometry.borrow()[&1].position_for_index(0);
                    window.click_at("live-editor", at - bounds.origin, cx);
                } else {
                    editor.update(cx, |editor, cx| {
                        editor.set_selected_text_range(0..0, window, cx);
                    });
                }
                assert!(!editor.read(cx).composition.is_active());
                window.input("X", cx);
                assert_eq!(editor.read(cx).text(), "Xa中b");
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window)
                    .compose_text(None, "", None, cx);
                assert_eq!(editor.read(cx).text(), "Xa中b");
            }
        })
        .expect("composition retargeting");
    }

    #[gpui_kit::test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn composition_commit_cancel_and_native_bounds_use_real_host(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::init(cx);
        });
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                let editor = cx.new(PrototypeEditor::new);
                let focus = editor.read(cx).focus_handle();
                window.focus(&focus, cx);
                editor
            })
            .expect("test window")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let original = editor.read(cx).text().to_owned();
            let bounds = window.find("live-editor").bounds();
            for invalid in [4..4, Range { start: 5, end: 3 }, 6..6] {
                editor.update(cx, |editor, cx| {
                    editor.replace_and_mark_text_in_range(
                        Some(0..0),
                        "中\r\n😀",
                        Some(invalid),
                        window,
                        cx,
                    );
                    assert_eq!(editor.text(), original);
                    assert!(!editor.composition.is_active());
                });
            }
            let mut input =
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
            input.compose_text(Some(9999..9999), "invalid", None, cx);
            assert_eq!(editor.read(cx).text(), original);
            assert!(input.marked_text_range(cx).is_none());
            input.compose_text(Some(0..0), "中\r\n😀", Some(3..3), cx);
            assert!(editor.read(cx).text().starts_with("中\n😀"));
            assert_eq!(input.marked_text_range(cx), Some(0..4));
            assert_eq!(
                input.selected_text_range(cx).expect("selection").range,
                2..2
            );
            input.compose_text(None, "", None, cx);
            assert_eq!(editor.read(cx).text(), original);
            input.compose_text(Some(0..0), "に", Some(1..1), cx);
            assert_eq!(input.marked_text_range(cx), Some(0..1));
            input.compose_text(None, "日本😀", Some(4..4), cx);
            assert!(editor.read(cx).text().starts_with("日本😀"));
            input.compose_text(None, "", None, cx);
            assert_eq!(editor.read(cx).text(), original);
            input.compose_text(Some(0..0), "中", Some(1..1), cx);
            input.commit_text("中文😀", cx);
            assert!(input.marked_text_range(cx).is_none());
            assert!(editor.read(cx).text().starts_with("中文😀"));
            let caret = editor
                .update(cx, |editor, cx| {
                    editor.bounds_for_range(4..4, bounds, window, cx)
                })
                .expect("leaf geometry");
            assert!(caret.origin.x >= bounds.origin.x);
            assert!(caret.size.height > px(0.));
            editor.update(cx, |editor, cx| {
                editor.selection_reversed = true;
                editor.set_selected_text_range(0..1, window, cx);
                let selection = editor
                    .selected_text_range(false, window, cx)
                    .expect("native selection");
                assert_eq!(selection.range, 0..1);
                assert!(!selection.reversed);
            });
        })
        .expect("window update");
    }
}
