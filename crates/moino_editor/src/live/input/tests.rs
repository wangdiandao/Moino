use super::super::{LiveEditor, Options};
use gpui_kit::test::{TestInput, TestWindowExt};
use gpui_kit::{AppContext, ElementInputHandler, EntityInputHandler, TestAppContext, px};

#[gpui_kit::test]
// Spec: docs/contracts/document-model.md#文本与位置
fn initial_accessible_value_matches_normalized_native_text_before_any_edit(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, editor) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let editor = cx.new(|cx| {
                LiveEditor::new(
                    "中\r\n😀\r尾",
                    Options::new("    ", false).expect("policy"),
                    cx,
                )
            });
            window.focus(&editor.read(cx).focus_handle(), cx);
            editor
        })
        .expect("editor window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let expected = "中\n😀\n尾";
        assert_eq!(window.find("document-editor").value(), Some(expected));
        editor.update(cx, |editor, cx| {
            assert_eq!(editor.text_length_utf16(window, cx), Some(6));
            assert_eq!(
                editor
                    .text_for_range(0..6, &mut None, window, cx)
                    .as_deref(),
                Some(expected)
            );
            assert_eq!(editor.snapshot().text().to_string(), expected);
            assert_eq!(editor.snapshot().revision().value(), 0);
            assert!(!editor.is_dirty() && !editor.document.has_undo());
        });
    })
    .expect("first-frame accessibility and native input agree");
}

#[gpui_kit::test]
// Spec: docs/contracts/document-model.md#文本与位置
fn preedit_crlf_selection_maps_before_emoji_and_rejects_invalid_native_boundaries(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        moino_theme::register_fonts(cx).expect("fonts");
        moino_theme::apply(moino_theme::BuiltinTheme::Paper, cx);
        crate::init(cx);
    });
    let (handle, editor) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let editor = cx
                .new(|cx| LiveEditor::new("seed", Options::new("    ", true).expect("policy"), cx));
            window.focus(&editor.read(cx).focus_handle(), cx);
            editor
        })
        .expect("editor window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(None, "中\r\n😀", Some(3..3), cx);
        assert_eq!(
            input.selected_text_range(cx).expect("selection").range,
            2..2
        );
        assert_eq!(input.marked_text_range(cx), Some(0..4));
        assert_eq!(editor.read(cx).display_text().to_string(), "中\n😀seed");
        input.compose_text(None, "中\r\n😀", Some(4..4), cx);
        assert_eq!(
            input
                .selected_text_range(cx)
                .expect("unchanged selection")
                .range,
            2..2
        );
        assert_eq!(input.marked_text_range(cx), Some(0..4));
        drop(input);
        // TestInput rejects these malformed ranges before the host callback.
        for invalid in [std::ops::Range { start: 5, end: 3 }, 6..6] {
            editor.update(cx, |editor, cx| {
                editor.replace_and_mark_text_in_range(None, "中\r\n😀", Some(invalid), window, cx);
                assert_eq!(
                    editor
                        .native_selection()
                        .expect("unchanged selection")
                        .range,
                    2..2
                );
            });
        }
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
        assert!(!editor.read(cx).document.has_undo());
    })
    .expect("mapped native preedit selection");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        let (caret, before) = editor.update(cx, |editor, cx| {
            (
                editor
                    .bounds_for_range(2..2, bounds, window, cx)
                    .expect("before emoji geometry"),
                editor
                    .bounds_for_range(1..1, bounds, window, cx)
                    .expect("preceding line geometry"),
            )
        });
        assert!(caret.top() > before.top());
        assert!(caret.size.height > px(0.));
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(None, "", None, cx);
        assert!(input.marked_text_range(cx).is_none());
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "seed");
        input.compose_text(None, "中\r\n😀", Some(3..5), cx);
        assert_eq!(
            input
                .selected_text_range(cx)
                .expect("emoji selection")
                .range,
            2..4
        );
        input.commit_text("中\r\n😀", cx);
    })
    .expect("preedit geometry, cancel and commit");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "中\n😀seed");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 1);
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo normalized composition");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "seed"));
}
