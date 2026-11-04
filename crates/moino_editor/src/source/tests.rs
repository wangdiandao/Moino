use super::SourceEditor;
use crate::LiveEditor;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext};

#[gpui_kit::test]
// Spec: docs/modules/view-modes.md#源码模式
fn source_focus_notification_does_not_restore_the_caret_after_navigation(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(|cx| {
                LiveEditor::new(
                    "# Title\n\nbody",
                    moino_editing::Options::new("    ", false).expect("policy"),
                    cx,
                )
            });
            document.update(cx, |document, cx| {
                document.select_source(
                    moino_document::Selection::cursor(moino_text::ScalarOffset::new(13)),
                    window,
                    cx,
                )
            });
            cx.new(|cx| SourceEditor::new(document, window, cx))
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        source.update(cx, |source, cx| source.focus(window, cx));
        window.render_frame(cx);
        window.press("secondary-home", cx);
        assert_eq!(source.read(cx).state.read(cx).selected_range(), 0..0);
    })
    .expect("navigate while owner focus notification is pending");
    cx.run_until_parked();
    cx.update(|cx| {
        let source = source.read(cx);
        assert_eq!(source.state.read(cx).selected_range(), 0..0);
        assert_eq!(
            source.document.read(cx).selection().head().offset.value(),
            0
        );
        assert_eq!(
            source.document.read(cx).snapshot().text().to_string(),
            "# Title\n\nbody"
        );
        assert_eq!(source.document.read(cx).snapshot().revision().value(), 0);
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_cancelled_preedit_restores_replaced_text_and_backward_selection(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(|cx| {
                LiveEditor::new(
                    "ab😀中",
                    moino_editing::Options::new("    ", false).expect("policy"),
                    cx,
                )
            });
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-end", cx);
        window.press("shift-left", cx);
        window.press("shift-left", cx);
        let bounds = window.find("source-surface").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, source.clone()), window);
        input.compose_text(None, "临时", Some(2..2), cx);
        input.compose_text(None, "", None, cx);
    })
    .expect("cancel replacement composition");
    cx.run_until_parked();
    cx.update(|cx| {
        let source = source.read(cx);
        let document = source.document.read(cx);
        assert_eq!(source.state.read(cx).value().as_ref(), "ab😀中");
        assert_eq!(document.snapshot().text().to_string(), "ab😀中");
        assert_eq!(document.snapshot().revision().value(), 0);
        assert_eq!(document.selection().anchor().offset.value(), 4);
        assert_eq!(document.selection().head().offset.value(), 2);
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_native_unmark_publishes_preedit_without_a_followup_edit(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("source-surface").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, source.clone()), window);
        input.compose_text(None, "日本語😀", Some(5..5), cx);
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .revision()
                .value(),
            0
        );
        input.unmark_text(cx);
        input.unmark_text(cx);
        input.compose_text(None, "", None, cx);
        assert_eq!(input.marked_text_range(cx), None);
    })
    .expect("native unmark without replacement");
    cx.run_until_parked();
    cx.update(|cx| {
        let document = source.read(cx).document.read(cx);
        assert_eq!(document.snapshot().text().to_string(), "日本語😀");
        assert_eq!(document.snapshot().revision().value(), 1);
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo native composition");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .is_empty()
        )
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_invalid_native_events_preserve_preedit_and_cancel_baseline(cx: &mut TestAppContext) {
    use gpui_kit::test::TestInput;
    use gpui_kit::{ElementInputHandler, EntityInputHandler};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(|cx| {
                LiveEditor::new(
                    "ab😀中",
                    moino_editing::Options::new("    ", false).expect("policy"),
                    cx,
                )
            });
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-end", cx);
        window.press("shift-left", cx);
        window.press("shift-left", cx);
        for invalid in [3..3, std::ops::Range { start: 4, end: 2 }, 6..6] {
            source.update(cx, |source, cx| {
                source.replace_text_in_range(Some(invalid.clone()), "X", window, cx);
                source.replace_and_mark_text_in_range(Some(invalid), "X", None, window, cx);
                assert_eq!(source.state.read(cx).value().as_ref(), "ab😀中");
                assert!(!source.composition.is_active());
            });
        }
        for invalid in [4..4, std::ops::Range { start: 5, end: 3 }, 6..6] {
            source.update(cx, |source, cx| {
                source.replace_and_mark_text_in_range(None, "中\r\n😀", Some(invalid), window, cx);
                assert_eq!(source.state.read(cx).value().as_ref(), "ab😀中");
                assert!(!source.composition.is_active());
            });
        }
        let bounds = window.find("source-surface").bounds();
        TestInput::new(ElementInputHandler::new(bounds, source.clone()), window).compose_text(
            None,
            "中\r\n😀",
            Some(3..3),
            cx,
        );
        source.update(cx, |source, cx| {
            assert_eq!(source.state.read(cx).value().as_ref(), "ab中\n😀");
            assert_eq!(
                source
                    .selected_text_range(false, window, cx)
                    .expect("native selection")
                    .range,
                4..4
            );
            source.set_selected_text_range(5..5, window, cx);
            source.replace_text_in_range(Some(5..5), "X", window, cx);
            assert!(source.composition.is_active());
            assert_eq!(source.state.read(cx).value().as_ref(), "ab中\n😀");
        });
        TestInput::new(ElementInputHandler::new(bounds, source.clone()), window)
            .compose_text(None, "", None, cx);
    })
    .expect("invalid native events");
    cx.run_until_parked();
    cx.update(|cx| {
        let source = source.read(cx);
        let document = source.document.read(cx);
        assert_eq!(source.state.read(cx).value().as_ref(), "ab😀中");
        assert_eq!(document.snapshot().revision().value(), 0);
        assert_eq!(document.selection().anchor().offset.value(), 4);
        assert_eq!(document.selection().head().offset.value(), 2);
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_same_turn_typing_survives_preedit_cancellation(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("source-surface").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, source.clone()), window);
        input.commit_text("a", cx);
        input.compose_text(None, "临时", None, cx);
        input.compose_text(None, "", None, cx);
        input.unmark_text(cx);
    })
    .expect("one native event turn");
    cx.run_until_parked();
    cx.update(|cx| {
        let source = source.read(cx);
        assert_eq!(source.state.read(cx).value().as_ref(), "a");
        assert_eq!(source.document.read(cx).snapshot().text().to_string(), "a");
        assert_eq!(source.document.read(cx).snapshot().revision().value(), 1);
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo typing");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .is_empty()
        )
    });
}

#[gpui_kit::test]
// Spec: docs/contracts/history.md#撤销单元
fn source_native_composition_add_merges_with_adjacent_typing_once(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("a", cx);
    })
    .expect("type before composition");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        let bounds = window.find("source-surface").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, source.clone()), window);
        input.compose_text(None, "中", None, cx);
        input.unmark_text(cx);
        input.unmark_text(cx);
        input.compose_text(None, "", None, cx);
    })
    .expect("composition ADD");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.input("b", cx);
    })
    .expect("type after composition");
    cx.run_until_parked();
    cx.update(|cx| {
        let document = source.read(cx).document.read(cx);
        assert_eq!(document.snapshot().text().to_string(), "a中b");
        assert_eq!(document.snapshot().revision().value(), 3);
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo ADD group");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .is_empty()
        )
    });
}

#[gpui_kit::test]
// Spec: docs/contracts/history.md#撤销单元
fn source_explicit_native_replacement_remains_a_separate_undo_group(cx: &mut TestAppContext) {
    use gpui_kit::test::TestInput;
    use gpui_kit::{ElementInputHandler, EntityInputHandler};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    for composition in [false, true] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.input("ab", cx);
        })
        .expect("initial ADD");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            if composition {
                let bounds = window.find("source-surface").bounds();
                let mut input =
                    TestInput::new(ElementInputHandler::new(bounds, source.clone()), window);
                input.compose_text(Some(1..2), "中", None, cx);
                input.unmark_text(cx);
            } else {
                source.update(cx, |source, cx| {
                    source.replace_text_in_range(Some(1..2), "中", window, cx);
                });
            }
        })
        .expect("native REPLACE");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.input("c", cx))
            .expect("following ADD");
        cx.run_until_parked();
        for expected in ["a中", "ab", ""] {
            cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
                .expect("undo one group");
            cx.run_until_parked();
            cx.update(|cx| {
                assert_eq!(
                    source
                        .read(cx)
                        .document
                        .read(cx)
                        .snapshot()
                        .text()
                        .to_string(),
                    expected
                )
            });
        }
    }
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_pointer_move_commits_preedit_before_relocating_the_caret(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("source-surface").bounds();
        TestInput::new(ElementInputHandler::new(bounds, source.clone()), window).compose_text(
            None,
            "中😀",
            Some(3..3),
            cx,
        );
        window.click_at(
            "source-surface",
            gpui_kit::point(gpui_kit::px(1.), gpui_kit::px(10.)),
            cx,
        );
        assert_eq!(
            TestInput::new(ElementInputHandler::new(bounds, source.clone()), window)
                .marked_text_range(cx),
            None
        );
        window.input("X", cx);
    })
    .expect("click during composition");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "X中😀"
        );
    });
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#多击与拖选
fn source_rejects_additional_cursors_before_the_next_input(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(|cx| {
                LiveEditor::new(
                    "one\ntwo",
                    moino_editing::Options::new("    ", false).expect("policy"),
                    cx,
                )
            });
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("shift-alt-down", cx);
        window.input("X", cx);
    })
    .expect("single selection editing");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "Xone\ntwo"
        );
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_navigation_commits_preedit_before_moving_and_typing(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("source-surface").bounds();
        let state = source.read(cx).state.clone();
        TestInput::new(ElementInputHandler::new(bounds, state.clone()), window).compose_text(
            None,
            "中😀",
            Some(3..3),
            cx,
        );
        window.press("left", cx);
        assert_eq!(
            TestInput::new(ElementInputHandler::new(bounds, state), window).marked_text_range(cx),
            None,
            "navigation must commit the previous replacement window"
        );
        window.input("X", cx);
    })
    .expect("navigate during composition");
    cx.run_until_parked();
    cx.update(|cx| {
        let source = source.read(cx);
        assert_eq!(
            source.document.read(cx).snapshot().text().to_string(),
            "中X😀"
        );
        assert_eq!(
            source.document.read(cx).selection().head().offset.value(),
            2
        );
    });
    for expected in ["中😀", ""] {
        cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
            .expect("undo composition and later input separately");
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                source
                    .read(cx)
                    .document
                    .read(cx)
                    .snapshot()
                    .text()
                    .to_string(),
                expected
            );
        });
    }
}

#[gpui_kit::test]
// Spec: docs/contracts/document-model.md#文本与位置
fn source_crlf_paste_normalizes_text_and_preserves_the_unicode_caret(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
            "中\r\n😀\r尾".to_owned(),
        ));
        window.press("secondary-v", cx);
    })
    .expect("paste CRLF and bare CR");
    cx.run_until_parked();
    cx.update(|cx| {
        let source = source.read(cx);
        assert_eq!(
            source.document.read(cx).snapshot().text().to_string(),
            "中\n😀\n尾"
        );
        assert_eq!(source.state.read(cx).value().as_ref(), "中\n😀\n尾");
        assert_eq!(
            source.document.read(cx).selection().head().offset.value(),
            5
        );
        assert_eq!(source.state.read(cx).selected_range(), 12..12);
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo normalized paste");
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .is_empty()
        )
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn source_preedit_is_temporary_and_commits_as_one_shared_history_unit(cx: &mut TestAppContext) {
    use gpui_kit::ElementInputHandler;
    use gpui_kit::test::TestInput;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(|cx| {
                LiveEditor::new(
                    "seed",
                    moino_editing::Options::new("    ", true).expect("policy"),
                    cx,
                )
            });
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("source-surface").bounds();
        let state = source.read(cx).state.clone();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, state), window);
        input.compose_text(None, "に😀", Some(3..3), cx);
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .revision()
                .value(),
            0
        );
        input.compose_text(None, "", None, cx);
    })
    .expect("cancel preedit");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "seed"
        );
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .revision()
                .value(),
            0
        );
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("source-surface").bounds();
        let state = source.read(cx).state.clone();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, state), window);
        input.compose_text(None, "中", Some(1..1), cx);
        input.commit_text("中文", cx);
    })
    .expect("commit preedit");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "中文seed"
        );
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .revision()
                .value(),
            1
        );
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo composition");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "seed"
        )
    });
}

#[gpui_kit::test]
// Spec: docs/contracts/markdown-dialect.md#增量解析与节点身份
fn source_edits_preserve_the_actual_surviving_duplicate_block(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(|cx| {
                LiveEditor::new(
                    "same\n\nsame",
                    moino_editing::Options::new("    ", false).expect("policy"),
                    cx,
                )
            });
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    let surviving = cx.update(|cx| {
        source
            .read(cx)
            .document
            .read(cx)
            .snapshot()
            .cst()
            .nodes()
            .iter()
            .filter(|node| matches!(node.syntax(), moino_markdown::Syntax::Paragraph))
            .nth(1)
            .expect("second paragraph")
            .id()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let state = source.read(cx).state.clone();
        state.update(cx, |state, cx| state.set_selected_range(0..6, cx));
        window.press("backspace", cx);
    })
    .expect("delete the first duplicate");
    cx.run_until_parked();
    cx.update(|cx| {
        let snapshot = source.read(cx).document.read(cx).snapshot();
        assert_eq!(snapshot.text().to_string(), "same");
        assert_eq!(snapshot.cst().nodes()[0].id(), surviving);
    });
}

#[gpui_kit::test]
// Spec: docs/contracts/history.md#历史边界
fn source_typing_coalesces_but_navigation_and_paste_cut_history(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        moino_theme::register_fonts(cx).expect("fonts");
        moino_theme::apply(moino_theme::BuiltinTheme::Paper, cx);
        crate::init(cx);
    });
    let (handle, source) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            let document = cx.new(LiveEditor::empty);
            let source = cx.new(|cx| SourceEditor::new(document, window, cx));
            source.update(cx, |source, cx| source.focus(window, cx));
            source
        })
        .expect("source window")
    });
    for text in ["a", "b", "中"] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.input(text, cx);
        })
        .expect("native typing");
        cx.run_until_parked();
        cx.update(|cx| {
            let source = source.read(cx);
            assert_eq!(
                source.document.read(cx).selection().head().offset.value(),
                source.bridge.submitted().text.chars().count(),
                "after {text}"
            );
        });
    }
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-z", cx);
    })
    .expect("shared undo");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            ""
        )
    });
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-shift-z", cx);
    })
    .expect("shared redo");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .selection()
                .head()
                .offset
                .value(),
            3
        );
        assert_eq!(source.read(cx).state.read(cx).selected_range(), 5..5);
        window.press("left", cx);
        assert_eq!(source.read(cx).state.read(cx).selected_range(), 2..2);
        window.input("X", cx);
    })
    .expect("move before input");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "abX中"
        )
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo after navigation");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "ab中"
        )
    });
    cx.update_window(handle, |_, window, cx| {
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("pasted".to_owned()));
        window.press("secondary-v", cx);
    })
    .expect("paste");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.input("Y", cx))
        .expect("type after paste");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo typing");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "abpasted中"
        )
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo paste");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            source
                .read(cx)
                .document
                .read(cx)
                .snapshot()
                .text()
                .to_string(),
            "ab中"
        )
    });
}
