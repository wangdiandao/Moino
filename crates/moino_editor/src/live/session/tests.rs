//! Public input and pointer lifecycles exercise retained block state through rendered output.
use super::super::{LiveEditor, Options};
use gpui_kit::base::text::editor_host::BlockNode;
use gpui_kit::test::{TestEventExt, TestInput, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, ElementInputHandler, Entity, MouseButton, Pixels, TestAppContext,
    point, px,
};
use std::time::Duration;

fn open(cx: &mut TestAppContext, text: &'static str) -> (AnyWindowHandle, Entity<LiveEditor>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        moino_theme::register_fonts(cx).expect("test fonts");
        moino_theme::apply(moino_theme::BuiltinTheme::Paper, cx);
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
        .expect("session test window")
    })
}

fn advance(cx: &mut TestAppContext, milliseconds: u64) {
    cx.run_until_parked();
    cx.executor()
        .advance_clock(Duration::from_millis(milliseconds));
    cx.run_until_parked();
}

fn row(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    editor: &Entity<LiveEditor>,
    index: usize,
) -> (String, Pixels, bool) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let state = editor.read(cx);
        let leaf = &state.leaves[index];
        (
            leaf.text.clone(),
            state.geometry.borrow()[&leaf.key].caret_height(),
            matches!(leaf.block, BlockNode::Heading { .. }),
        )
    })
    .expect("rendered row")
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#交互语法转换
fn keyboard_departure_commits_heading_and_history_restores_its_presentation(
    cx: &mut TestAppContext,
) {
    let (handle, editor) = open(cx, "\n\nbody");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("# Title", cx);
    })
    .expect("type heading candidate");
    cx.run_until_parked();
    let candidate = row(cx, handle, &editor, 0);
    assert_eq!(candidate.0, "# Title");
    assert!(!candidate.2);
    let body = cx.update(|cx| editor.read(cx).leaves.len() - 1);
    assert!((candidate.1 - row(cx, handle, &editor, body).1).abs() <= px(0.5));
    cx.update(|cx| editor.update(cx, |editor, _| editor.document.mark_saved()));
    cx.update_window(handle, |_, window, cx| window.press("secondary-end", cx))
        .expect("leave using keyboard");
    cx.run_until_parked();
    let committed = row(cx, handle, &editor, 0);
    assert_eq!(committed.0, "Title");
    assert!(committed.2 && committed.1 > candidate.1);
    cx.update(|cx| {
        assert!(!editor.read(cx).is_dirty());
        assert_eq!(
            editor.read(cx).snapshot().text().to_string(),
            "# Title\n\nbody"
        );
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-home", cx))
        .expect("reenter title");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0), committed);
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo semantic conversion");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0), candidate);
    cx.update(|cx| assert!(!editor.read(cx).is_dirty()));
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-shift-z", cx)
    })
    .expect("redo semantic conversion");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0), committed);
    cx.update(|cx| assert!(!editor.read(cx).is_dirty()));
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#交互语法转换
fn enter_finishes_a_candidate_and_undo_recovers_the_editable_prefix(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("# Title", cx);
        window.press("enter", cx);
    })
    .expect("type and enter without waiting for the worker");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "Title");
    assert!(row(cx, handle, &editor, 0).2);
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo split and conversion");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "# Title");
    assert!(!row(cx, handle, &editor, 0).2);
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "# Title"));
}

#[gpui_kit::test]
// Spec: docs/modules/view-modes.md#源码模式
fn source_barrier_commits_queued_candidate_before_returning_to_live(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("# Title", cx);
        editor.update(cx, |editor, cx| editor.commit_composition(window, cx));
    })
    .expect("surface transition queues behind native input");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "Title");
    assert!(row(cx, handle, &editor, 0).2);
    cx.update(|cx| assert!(!editor.read(cx).is_pending()));
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo surface conversion");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "# Title");
}

#[gpui_kit::test]
// Spec: docs/modules/view-modes.md#源码模式
fn source_barrier_commits_native_preedit_before_its_heading_conversion(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window).compose_text(
            None,
            "# 中😀",
            Some(5..5),
            cx,
        );
        editor.update(cx, |editor, cx| editor.commit_composition(window, cx));
    })
    .expect("leave while native composition is still marked");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "中😀");
    assert!(row(cx, handle, &editor, 0).2);
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "# 中😀");
        assert!(!editor.read(cx).is_pending());
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo conversion only");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "# 中😀");
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo committed composition");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "");
    cx.update(|cx| assert!(!editor.read(cx).is_dirty()));
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn preedit_heading_syntax_stays_temporary_and_cancel_creates_no_history(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window).compose_text(
            None,
            "# 中😀",
            Some(5..5),
            cx,
        );
    })
    .expect("compose heading syntax");
    advance(cx, 500);
    let preedit = row(cx, handle, &editor, 0);
    assert_eq!(preedit.0, "# 中😀");
    assert!(!preedit.2);
    cx.update(|cx| {
        let editor = editor.read(cx);
        assert_eq!(editor.snapshot().text().to_string(), "");
        assert_eq!(editor.snapshot().revision().value(), 0);
        assert!(!editor.is_dirty() && !editor.document.has_undo());
    });
    cx.update_window(handle, |_, window, cx| {
        let bounds = window.find("document-editor").bounds();
        TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window).compose_text(
            None,
            "",
            Some(0..0),
            cx,
        );
    })
    .expect("cancel native preedit");
    cx.run_until_parked();
    assert_eq!(row(cx, handle, &editor, 0).0, "");
    cx.update(|cx| {
        let editor = editor.read(cx);
        assert_eq!(editor.snapshot().revision().value(), 0);
        assert!(!editor.is_dirty() && !editor.document.has_undo());
    });
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#多击与拖选
fn pointer_holds_candidate_layout_until_release_then_waits_for_click_settlement(
    cx: &mut TestAppContext,
) {
    let (handle, editor) = open(cx, "\n\nbody");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("# Title", cx);
    })
    .expect("candidate");
    cx.run_until_parked();
    let candidate = row(cx, handle, &editor, 0);
    let position = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let position = {
                let state = editor.read(cx);
                let geometry = &state.geometry.borrow()[&state.leaves.last().expect("body").key];
                geometry.position_for_index(2) + point(px(0.), geometry.caret_height() / 2.)
            };
            window.pointer_down(position, MouseButton::Left, cx);
            position
        })
        .expect("press another block");
    advance(cx, 500);
    assert_eq!(row(cx, handle, &editor, 0), candidate);
    cx.update_window(handle, |_, window, cx| {
        window.pointer_up(position, MouseButton::Left, cx)
    })
    .expect("release pointer");
    advance(cx, 9);
    assert_eq!(row(cx, handle, &editor, 0), candidate);
    advance(cx, 1);
    assert_eq!(row(cx, handle, &editor, 0), candidate);
    advance(cx, 169);
    assert_eq!(row(cx, handle, &editor, 0), candidate);
    advance(cx, 1);
    let committed = row(cx, handle, &editor, 0);
    assert_eq!(committed.0, "Title");
    assert!(committed.2 && committed.1 > candidate.1);
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#多击与拖选
fn returning_before_click_settlement_cancels_the_old_heading_transition(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "\n\nbody");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("# Title", cx);
    })
    .expect("candidate");
    cx.run_until_parked();
    let candidate = row(cx, handle, &editor, 0);
    for (index, wait) in [(2, 50), (0, 200)] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let position = {
                let state = editor.read(cx);
                let leaf = if index == 0 {
                    &state.leaves[0]
                } else {
                    state.leaves.last().expect("body")
                };
                let geometry = &state.geometry.borrow()[&leaf.key];
                geometry.position_for_index(2) + point(px(0.), geometry.caret_height() / 2.)
            };
            window.pointer_down(position, MouseButton::Left, cx);
            window.pointer_up(position, MouseButton::Left, cx);
        })
        .expect("click another block or return to candidate");
        advance(cx, wait);
        assert_eq!(row(cx, handle, &editor, 0), candidate);
    }
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo original typing without a phantom conversion");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "\n\nbody"));
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn drag_expands_endpoint_outer_spans_and_nested_markers_but_not_middle_spans(
    cx: &mut TestAppContext,
) {
    let source = "**left _nested_** x **middle** x **right _deep_**\n\nbody";
    let (handle, editor) = open(cx, source);
    cx.update_window(handle, |_, window, cx| window.press("secondary-end", cx))
        .expect("fold all inline spans");
    cx.run_until_parked();
    let collapsed = row(cx, handle, &editor, 0);
    assert_eq!(collapsed.0, "left nested x middle x right deep");
    let end = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let (start, end) = {
                let state = editor.read(cx);
                let leaf = &state.leaves[0];
                let geometry = &state.geometry.borrow()[&leaf.key];
                let middle = point(px(0.), geometry.caret_height() / 2.);
                (
                    geometry.position_for_index(1) + middle,
                    geometry.position_for_index(leaf.text.len() - 1) + middle,
                )
            };
            window.pointer_down(start, MouseButton::Left, cx);
            window.pointer_move(end, Some(MouseButton::Left), cx);
            end
        })
        .expect("drag across three spans");
    advance(cx, 500);
    assert_eq!(row(cx, handle, &editor, 0), collapsed);
    cx.update_window(handle, |_, window, cx| {
        window.pointer_up(end, MouseButton::Left, cx)
    })
    .expect("release selection");
    advance(cx, 9);
    assert_eq!(row(cx, handle, &editor, 0), collapsed);
    advance(cx, 1);
    assert_eq!(
        row(cx, handle, &editor, 0).0,
        "**left _nested_** x middle x **right _deep_**"
    );
    cx.update(|cx| {
        let editor = editor.read(cx);
        let BlockNode::Paragraph(paragraph) = &editor.leaves[0].block else {
            panic!("inline paragraph");
        };
        let nested = paragraph
            .children()
            .iter()
            .find(|run| run.text() == "nested")
            .expect("nested body");
        assert!(nested.mark_runs().iter().any(|(_, mark)| mark.bold));
        assert!(nested.mark_runs().iter().any(|(_, mark)| mark.italic));
        assert_eq!(editor.snapshot().text().to_string(), source);
        assert_eq!(editor.snapshot().revision().value(), 0);
        assert!(!editor.is_dirty() && !editor.document.has_undo());
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn pointer_commits_preedit_without_mapping_its_display_position_twice(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "ab");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("right", cx);
        let bounds = window.find("document-editor").bounds();
        TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window).compose_text(
            None,
            "中",
            Some(1..1),
            cx,
        );
        window.render_frame(cx);
        let position = {
            let state = editor.read(cx);
            let leaf = &state.leaves[0];
            let geometry = &state.geometry.borrow()[&leaf.key];
            geometry.position_for_index(leaf.text.len())
                + point(px(0.), geometry.caret_height() / 2.)
        };
        window.pointer_down(position, MouseButton::Left, cx);
        window.pointer_up(position, MouseButton::Left, cx);
    })
    .expect("click during native preedit");
    advance(cx, 200);
    cx.update(|cx| {
        let state = editor.read(cx);
        assert_eq!(state.snapshot().text().to_string(), "a中b");
        assert_eq!(state.selection().head().offset.value(), 3);
        assert_eq!(state.error_code(), None);
    });
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#多击与拖选
fn drag_uses_the_queued_pointer_anchor_while_typing_is_in_flight(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "abcdef");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-end", cx);
        let (start, end) = {
            let state = editor.read(cx);
            let geometry = &state.geometry.borrow()[&state.leaves[0].key];
            let middle = point(px(0.), geometry.caret_height() / 2.);
            (
                geometry.position_for_index(1) + middle,
                geometry.position_for_index(4) + middle,
            )
        };
        window.input("X", cx);
        assert!(editor.read(cx).is_pending());
        window.pointer_down(start, MouseButton::Left, cx);
        window.pointer_move(end, Some(MouseButton::Left), cx);
        window.pointer_up(end, MouseButton::Left, cx);
    })
    .expect("drag before typing publication");
    advance(cx, 200);
    cx.update(|cx| {
        let state = editor.read(cx);
        assert_eq!(state.snapshot().text().to_string(), "abcdefX");
        assert_eq!(state.selection().anchor().offset.value(), 1);
        assert_eq!(state.selection().head().offset.value(), 4);
        assert_eq!(state.error_code(), None);
    });
}
