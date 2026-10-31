use crate::{PrototypeEditor, byte_from_utf16};
use gpui_kit::{
    Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, px, size,
};
use std::ops::Range;

impl EntityInputHandler for PrototypeEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
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
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        self.original_composition = None;
        self.changed(window, cx);
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(range, text);
        self.marked = None;
        self.original_composition = None;
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
            if let Some((text, selection)) = self.original_composition.take() {
                self.text = text;
                self.selection = selection;
            }
            self.marked = None;
        } else {
            if self.original_composition.is_none() {
                self.original_composition = Some((self.text.clone(), self.selection.clone()));
            }
            let start = range
                .as_ref()
                .or(self.marked.as_ref())
                .unwrap_or(&self.selection)
                .start;
            self.edit(range, text);
            let end = start + text.encode_utf16().count();
            self.marked = Some(start..end);
            if let Some(selection) = selection
                && selection.start <= selection.end
                && selection.end <= end - start
            {
                self.selection = start + selection.start..start + selection.end;
            }
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
            self.selection = range;
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
    fn utf16_boundaries_do_not_split_surrogate_pairs() {
        assert_eq!(byte_from_utf16("中😀a", 1), Some(3));
        assert_eq!(byte_from_utf16("中😀a", 2), None);
        assert_eq!(byte_from_utf16("中😀a", 3), Some(7));
        assert_eq!(byte_from_utf16("中😀a", 4), Some(8));
        assert_eq!(byte_from_utf16("中😀a", 5), None);
    }

    #[gpui_kit::test]
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
            let mut input =
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
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
        })
        .expect("window update");
    }
}
