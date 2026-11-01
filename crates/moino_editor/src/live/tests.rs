use super::{LiveEditor, Options};
use gpui_kit::test::{TestInput, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, ElementInputHandler, Entity, EntityInputHandler, TestAppContext,
};

#[gpui_kit::test]
// Spec: docs/contracts/document-model.md#选择与映射
fn queued_native_selection_maps_through_earlier_edits(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "hello");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("x", cx);
        editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(0..0, window, cx)
        });
        window.input("y", cx);
    })
    .expect("queued input and native selection");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "xyhello");
        assert!(editor.read(cx).error_code().is_none());
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn moving_the_native_caret_commits_preedit_before_selecting_display_coordinates(
    cx: &mut TestAppContext,
) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(None, "中", Some(1..1), cx);
    })
    .expect("preedit");
    cx.update_window(handle, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(0..0, window, cx)
        });
        window.input("X", cx);
    })
    .expect("move and type");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "X中"));
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#逻辑选择
fn vertical_navigation_waits_for_committed_geometry(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "a\nb");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("x", cx);
        window.press("down", cx);
    })
    .expect("queued vertical navigation");
    cx.run_until_parked();
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("navigation geometry");
    }
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "xa\nb");
        assert_eq!(editor.read(cx).selection().head().offset.value(), 4);
        assert!(!editor.read(cx).is_pending());
    });
}

fn open(cx: &mut TestAppContext, text: &'static str) -> (AnyWindowHandle, Entity<LiveEditor>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let editor = cx.new(|cx| {
                LiveEditor::new(text, Options::new("    ", true).expect("input policy"), cx)
            });
            window.focus(&editor.read(cx).focus_handle(), cx);
            editor
        })
        .expect("editor window")
    })
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入路由
fn queued_native_typing_and_block_commands_preserve_order(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("中文😀", cx);
        window.press("enter", cx);
        window.input("next", cx);
    })
    .expect("native events");
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = editor.read(cx);
        assert_eq!(editor.snapshot().text().to_string(), "中文😀\n\nnext");
        assert!(!editor.is_pending());
        assert!(editor.is_dirty());
    });
}

#[gpui_kit::test]
// Spec: docs/contracts/history.md#撤销单元
fn native_undo_and_redo_share_document_history(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("😀", cx);
    })
    .expect("typing");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), ""));
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-shift-z", cx)
    })
    .expect("redo");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "😀"));
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#逻辑选择
fn navigation_waits_for_earlier_typing_and_moves_over_whole_graphemes(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("a\u{301}😀", cx);
        window.press("left", cx);
        window.press("left", cx);
        window.input("b", cx);
    })
    .expect("queued navigation");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "ba\u{301}😀");
        assert_eq!(editor.read(cx).selection().head().offset.value(), 1);
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn preedit_cancel_is_revision_free_and_commit_is_one_transaction(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "abc");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(Some(0..0), "に😀", Some(3..3), cx);
        assert_eq!(input.marked_text_range(cx), Some(0..3));
        assert_eq!(
            input
                .selected_text_range(cx)
                .expect("preedit selection")
                .range,
            3..3
        );
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "abc");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
        input.compose_text(None, "", None, cx);
        assert!(input.marked_text_range(cx).is_none());
        input.compose_text(Some(0..0), "中", Some(1..1), cx);
        input.commit_text("中文😀", cx);
    })
    .expect("composition");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "中文😀abc");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 1);
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn composition_follows_queued_typing_and_explicit_preedit_ranges(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("a", cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(None, "中", Some(1..1), cx);
        input.commit_text("中", cx);
    })
    .expect("composition after queued text");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "a中"));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(Some(2..2), "に😀", Some(3..3), cx);
        input.compose_text(Some(2..5), "日本", Some(2..2), cx);
        input.commit_text("日本", cx);
    })
    .expect("explicit marked replacement");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "a中日本"));
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#滚动保障
fn long_documents_only_layout_visible_leaves_and_reveal_remote_cursors(cx: &mut TestAppContext) {
    let source = "paragraph\n\n".repeat(1000);
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, editor) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let editor = cx.new(|cx| {
                LiveEditor::new(&source, Options::new("    ", false).expect("policy"), cx)
            });
            window.focus(&editor.read(cx).focus_handle(), cx);
            editor
        })
        .expect("long document")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let visible = editor.read(cx).geometry.borrow().len();
        assert!(
            visible > 0 && visible < 100,
            "virtualized leaf count {visible}"
        );
        editor.update(cx, |editor, cx| {
            editor.select_source(
                moino_document::Selection::cursor(editor.snapshot().text().len()),
                window,
                cx,
            )
        });
    })
    .expect("select remote cursor");
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("layout remote caret");
    }
    cx.update(|cx| {
        let editor = editor.read(cx);
        let caret = editor
            .caret(editor.selection().head().offset)
            .expect("remote geometry");
        let viewport = editor.list.viewport_bounds();
        assert!(caret.top() >= viewport.top() && caret.bottom() <= viewport.bottom());
        assert!(editor.geometry.borrow().len() < 100);
    });
}
