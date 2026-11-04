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
// Spec: docs/contracts/document-model.md#选择与映射
fn selection_events_during_undo_and_redo_map_through_the_actual_history_steps(
    cx: &mut TestAppContext,
) {
    let (handle, editor) = open(cx, "abcd");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(2..2, window, cx)
        });
        window.input("X", cx);
    })
    .expect("insert");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-z", cx);
        editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(3..3, window, cx)
        });
    })
    .expect("selection during undo");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "abcd");
        assert_eq!(editor.read(cx).selection().head().offset.value(), 2);
        assert!(editor.read(cx).error_code().is_none());
    });
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-shift-z", cx);
        editor.update(cx, |editor, cx| {
            editor.select_source(
                super::selection(
                    moino_text::ScalarOffset::new(3),
                    moino_text::ScalarOffset::new(1),
                ),
                window,
                cx,
            )
        });
    })
    .expect("backward selection during redo");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "abXcd");
        assert_eq!(editor.read(cx).selection().anchor().offset.value(), 4);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 1);
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
        moino_theme::register_fonts(cx).expect("bundled test fonts");
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
        .expect("editor window")
    })
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn heading_navigation_and_typing_use_content_boundaries_even_after_source_selection(
    cx: &mut TestAppContext,
) {
    for (source, start, end, expected) in [
        ("## 中😀 ##", 3, 5, "## X中😀 ##"),
        ("中😀\n===", 0, 2, "X中😀\n==="),
        ("## ", 2, 3, "##X "),
    ] {
        let (handle, editor) = open(cx, source);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press("secondary-home", cx);
            assert_eq!(editor.read(cx).selection().head().offset.value(), start);
            window.press("left", cx);
            assert_eq!(editor.read(cx).selection().head().offset.value(), start);
            window.press("secondary-end", cx);
            assert_eq!(editor.read(cx).selection().head().offset.value(), end);
            window.press("right", cx);
            assert_eq!(editor.read(cx).selection().head().offset.value(), end);
            editor.update(cx, |editor, cx| {
                editor.select_source(
                    moino_document::Selection::cursor(moino_text::ScalarOffset::new(0)),
                    window,
                    cx,
                );
            });
            window.input("X", cx);
        })
        .expect("navigate hidden heading markers and insert");
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), expected));
        cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
            .expect("undo insertion");
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), source));
    }
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn composing_in_a_heading_preserves_title_style_and_commits_inside_its_hidden_prefix(
    cx: &mut TestAppContext,
) {
    use gpui_kit::base::text::editor_host::BlockNode;
    let (handle, editor) = open(cx, "## Title");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(None, "中😀", Some(3..3), cx);
        assert_eq!(input.marked_text_range(cx), Some(3..6));
        let state = editor.read(cx);
        assert_eq!(state.leaves[0].text, "中😀Title");
        assert!(matches!(
            state.leaves[0].block,
            BlockNode::Heading { level: 2, .. }
        ));
        assert_eq!(state.snapshot().revision().value(), 0);
        input.commit_text("中😀", cx);
    })
    .expect("compose in rendered heading");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).snapshot().text().to_string(),
            "## 中😀Title"
        )
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入法组合
fn partial_preedit_updates_preserve_unreplaced_text_and_one_undo_unit(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "# original");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-a", cx);
        let bounds = window.find("document-editor").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
        input.compose_text(None, "中😀", Some(3..3), cx);
        input.compose_text(Some(1..3), "文", Some(1..1), cx);
        assert_eq!(editor.read(cx).display_text().to_string(), "中文");
        assert_eq!(input.marked_text_range(cx), Some(0..2));
        assert_eq!(
            input
                .selected_text_range(cx)
                .expect("native selection")
                .range,
            2..2
        );
        input.compose_text(Some(2..2), "末", Some(1..1), cx);
        assert_eq!(editor.read(cx).display_text().to_string(), "中文末");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
        input.unmark_text(cx);
    })
    .expect("replace part of preedit");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "中文末");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 1);
    });
    cx.update_window(handle, |_, window, cx| window.press("secondary-z", cx))
        .expect("undo whole composition");
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), "# original"));
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn navigation_skips_container_markers_and_empty_containers_accept_native_input(
    cx: &mut TestAppContext,
) {
    for (source, expected, start) in [
        ("- item", "- Xitem", 2),
        ("> text", "> Xtext", 2),
        ("> ", "> X", 2),
        ("- [ ] ", "- [ ] X", 6),
    ] {
        let (handle, editor) = open(cx, source);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press("secondary-home", cx);
            window.press("left", cx);
            assert_eq!(
                editor.read(cx).selection().head().offset.value(),
                start,
                "{source}"
            );
            window.input("X", cx);
        })
        .expect("edit a container's first content position");
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(editor.read(cx).snapshot().text().to_string(), expected));
    }
    let (handle, editor) = open(cx, "> a\n> b");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-end", cx);
        window.press("left", cx);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 6);
        window.press("left", cx);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 3);
        window.press("right", cx);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 6);
    })
    .expect("cross a hidden continuation prefix");
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn a_separator_caret_and_empty_row_triple_click_do_not_reenter_the_previous_heading(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::ClickOptions;
    use gpui_kit::{point, px};
    let (handle, editor) = open(cx, "## title\n\n");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        editor.update(cx, |editor, cx| {
            editor.set_selected_text_range(9..9, window, cx)
        });
        window.render_frame(cx);
        let position = {
            let state = editor.read(cx);
            assert!(state.leaves[0].heading_badge.is_none());
            let first = &state.geometry.borrow()[&state.leaves[0].key];
            let caret = state
                .caret(moino_text::ScalarOffset::new(9))
                .expect("separator caret");
            assert!(
                caret.top() > first.position_for_index(0).y + first.caret_height() / 2.,
                "separator caret must not stay on the title"
            );
            let last = state.leaves.last().expect("empty paragraph");
            let geometry = &state.geometry.borrow()[&last.key];
            geometry.position_for_index(0) + point(px(0.), geometry.caret_height() / 2.)
        };
        let bounds = window.find("document-editor").bounds();
        window.click_with_options(
            "document-editor",
            ClickOptions::new()
                .with_offset(position - bounds.origin)
                .with_count(3),
            cx,
        );
        assert!(
            editor.read(cx).selection().is_collapsed(),
            "selection {:?}, leaves {:?}",
            editor.read(cx).selection(),
            editor
                .read(cx)
                .leaves
                .iter()
                .map(|leaf| (leaf.raw.clone(), leaf.text.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(editor.read(cx).selection().head().offset.value(), 10);
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
    })
    .expect("separator geometry and empty-row selection");
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#交互语法转换
fn typing_heading_syntax_retains_a_candidate_until_a_block_boundary(cx: &mut TestAppContext) {
    use gpui_kit::base::text::editor_host::BlockNode;
    let (handle, editor) = open(cx, "");
    for hashes in ["#", "##"] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.input("#", cx);
        })
        .expect("type a hash");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let state = editor.read(cx);
            assert_eq!(state.leaves[0].text, hashes);
            assert!(state.leaves[0].heading_badge.is_none());
            assert!(matches!(state.leaves[0].block, BlockNode::Paragraph(_)));
            assert_eq!(state.snapshot().text().to_string(), hashes);
        })
        .expect("pending prefix stays ordinary text");
    }
    cx.update_window(handle, |_, window, cx| window.input(" ", cx))
        .expect("type separator");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).leaves[0].heading_badge, None);
        assert_eq!(editor.read(cx).leaves[0].text, "## ");
        assert!(matches!(
            editor.read(cx).leaves[0].block,
            BlockNode::Paragraph(_)
        ));
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "## ");
    });
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn heading_reentry_preserves_title_geometry_and_hides_block_markers_without_history(
    cx: &mut TestAppContext,
) {
    use gpui_kit::base::text::editor_host::BlockNode;
    use gpui_kit::test::ClickOptions;
    use gpui_kit::{point, px};
    for source in [
        "# Title\n\nbody",
        "## Title\n\nbody",
        "### Title\n\nbody",
        "#### Title\n\nbody",
        "##### Title\n\nbody",
        "###### Title\n\nbody",
    ] {
        let level =
            u8::try_from(source.chars().take_while(|ch| *ch == '#').count()).expect("level");
        let (handle, editor) = open(cx, source);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press("secondary-end", cx);
        })
        .expect("leave heading");
        cx.run_until_parked();
        let folded_height = cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                let state = editor.read(cx);
                let heading = &state.leaves[0];
                assert_eq!(heading.text, "Title");
                assert!(matches!(heading.block, BlockNode::Heading { .. }));
                assert!(heading.heading_badge.is_none());
                state.geometry.borrow()[&heading.key].caret_height()
            })
            .expect("measure folded heading");
        for pointer in [false, true] {
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                if pointer {
                    let position = {
                        let state = editor.read(cx);
                        let geometry = &state.geometry.borrow()[&state.leaves[0].key];
                        geometry.position_for_index(2) + point(px(0.), geometry.caret_height() / 2.)
                    };
                    let bounds = window.find("document-editor").bounds();
                    window.click_with_options(
                        "document-editor",
                        ClickOptions::new().with_offset(position - bounds.origin),
                        cx,
                    );
                } else {
                    window.press("secondary-home", cx);
                }
            })
            .expect("reenter heading with pointer or keyboard");
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(10));
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                let state = editor.read(cx);
                let heading = &state.leaves[0];
                assert_eq!(heading.text, "Title");
                assert_eq!(heading.heading_badge, Some(level));
                assert!(matches!(heading.block, BlockNode::Heading { .. }));
                let geometry = state.geometry.borrow();
                assert!((geometry[&heading.key].caret_height() - folded_height).abs() <= px(0.5));
                assert_eq!(state.snapshot().revision().value(), 0);
                assert!(!state.is_dirty() && !state.document.has_undo());
            })
            .expect("stable title geometry on reentry");
            cx.update_window(handle, |_, window, cx| window.press("secondary-end", cx))
                .expect("leave again");
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                let state = editor.read(cx);
                assert_eq!(state.leaves[0].text, "Title");
                assert!(
                    (state.geometry.borrow()[&state.leaves[0].key].caret_height() - folded_height)
                        .abs()
                        <= px(0.5)
                );
                assert_eq!(state.snapshot().text().to_string(), source);
            })
            .expect("restore exact heading geometry");
        }
    }
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#多击与拖选
fn stationary_pointer_reentry_preserves_the_original_hit_across_reflow(cx: &mut TestAppContext) {
    use gpui_kit::test::TestEventExt;
    use gpui_kit::{MouseButton, point, px};
    for source in [
        "# Heading\n\nBody",
        "## Heading\n\nBody",
        "### Heading\n\nBody",
        "#### Heading\n\nBody",
        "##### Heading\n\nBody",
        "###### Heading\n\nBody",
        "**bold** tail\n\nBody",
        "[title](target) tail\n\nBody",
    ] {
        for enter_first in [true, false] {
            let (handle, editor) = open(cx, source);
            if enter_first {
                cx.update_window(handle, |_, window, cx| {
                    window.press("secondary-end", cx);
                })
                .expect("fold syntax before pointer entry");
                cx.run_until_parked();
            }
            let (position, expected) = cx
                .update_window(handle, |_, window, cx| {
                    window.render_frame(cx);
                    let position = {
                        let state = editor.read(cx);
                        let leaf = if enter_first {
                            &state.leaves[0]
                        } else {
                            state.leaves.last().expect("body leaf")
                        };
                        let geometry = &state.geometry.borrow()[&leaf.key];
                        geometry.position_for_index(3) + point(px(0.), geometry.caret_height() / 2.)
                    };
                    let expected = editor.read(cx).hit(position).expect("folded text hit");
                    window.pointer_down(position, MouseButton::Left, cx);
                    (position, expected)
                })
                .expect("press on folded syntax");
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                window.pointer_move(position, Some(MouseButton::Left), cx);
                window.pointer_up(position, MouseButton::Left, cx);
                let state = editor.read(cx);
                assert_eq!(
                    state.selection(),
                    moino_document::Selection::cursor(expected),
                    "{source}"
                );
                assert_eq!(state.snapshot().revision().value(), 0);
                assert!(!state.document.has_undo());
                window.input("X", cx);
            })
            .expect("stationary movement after the new layout");
            cx.run_until_parked();
            cx.update_window(handle, |_, _, cx| {
                let mut text = moino_text::TextBuffer::new(source);
                text.replace(expected..expected, "X")
                    .expect("expected insertion");
                assert_eq!(
                    editor.read(cx).snapshot().text().to_string(),
                    text.to_string()
                );
            })
            .expect("typing inserts without replacing existing letters");
        }
    }
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
// Spec: docs/architecture.md#关键接缝
fn cut_and_copy_observe_the_selection_after_queued_edits(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("中😀a", cx);
        window.press("secondary-a", cx);
        window.press("secondary-c", cx);
    })
    .expect("queued copy");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("中😀a")
        )
    });
    cx.update_window(handle, |_, window, cx| {
        window.input("replacement", cx);
        window.press("secondary-a", cx);
        window.press("secondary-x", cx);
    })
    .expect("queued cut");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("replacement")
        );
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "");
    });
}

#[gpui_kit::test]
// Spec: docs/modules/editing.md#输入路由
fn readonly_surface_rejects_native_preedit_typing_and_history(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "protected");
    cx.update_window(handle, |_, window, cx| {
        editor.update(cx, |editor, _| {
            editor.options = editor.options.clone().read_only(true)
        });
        window.render_frame(cx);
        let bounds = window.find("document-editor").bounds();
        {
            let mut input =
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
            input.compose_text(None, "中", None, cx);
            assert!(input.marked_text_range(cx).is_none());
        }
        window.input("X", cx);
        window.press("secondary-z", cx);
        window.press("secondary-shift-z", cx);
    })
    .expect("readonly inputs");
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "protected");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let alert = window.find("editor-error");
        assert!(alert.visible());
        assert_eq!(
            alert.label(),
            Some(moino_ui::editing_error("editing.unavailable").as_ref())
        );
        assert!(alert.bounds().bottom() <= window.find("document-editor").bounds().bottom());
        editor.update(cx, |editor, _| {
            editor.options = editor.options.clone().read_only(false)
        });
        window.input("X", cx);
    })
    .expect("visible error and retry");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("editor-error").is_none());
        assert_eq!(editor.read(cx).snapshot().text().to_string(), "Xprotected");
    })
    .expect("successful retry clears the error");
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#标记展开与折叠
fn a_collapsed_thematic_rule_keeps_native_hit_geometry_and_reveals_its_source(
    cx: &mut TestAppContext,
) {
    let (handle, editor) = open(cx, "---\n\nend");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("secondary-end", cx);
        window.render_frame(cx);
        let first = &editor.read(cx).leaves[0];
        assert!(first.rule);
        assert!(first.text.is_empty());
        assert!(editor.read(cx).geometry.borrow().contains_key(&first.key));
        window.press("secondary-home", cx);
        window.render_frame(cx);
        assert!(!editor.read(cx).leaves[0].rule);
        assert_eq!(editor.read(cx).leaves[0].text, "---");
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
    })
    .expect("rule projection and navigation");
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
// Spec: docs/acceptance/m2.md#windows-原生输入
fn preedit_preserves_other_blocks_their_geometry_and_virtualization(cx: &mut TestAppContext) {
    let (handle, editor) = open(cx, "# Title\n\nFirst paragraph\n\nLast paragraph");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        editor.update(cx, |editor, cx| {
            editor.select_source(
                super::selection(
                    moino_text::ScalarOffset::new(12),
                    moino_text::ScalarOffset::new(12),
                ),
                window,
                cx,
            );
        });
        window.render_frame(cx);
        let first_key = editor.read(cx).leaves[0].key;
        let last_key = editor.read(cx).leaves[4].key;
        let before = editor.read(cx).geometry.borrow()[&first_key].bounds();
        let bounds = window.find("document-editor").bounds();
        {
            let mut input =
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
            input.compose_text(None, "に😀", Some(3..3), cx);
        }
        window.render_frame(cx);
        let state = editor.read(cx);
        assert_eq!(state.leaves.len(), 5);
        assert_eq!(state.leaves[0].key, first_key);
        assert_eq!(state.leaves[4].key, last_key);
        assert_eq!(state.leaves[0].text, "Title");
        assert_eq!(state.leaves[2].text, "Firに😀st paragraph");
        assert_eq!(state.leaves[4].text, "Last paragraph");
        assert_eq!(state.geometry.borrow()[&first_key].bounds(), before);
        assert_eq!(state.leaves[4].source(0).value(), 28);
        assert_eq!(state.snapshot().revision().value(), 0);
        {
            let mut input =
                TestInput::new(ElementInputHandler::new(bounds, editor.clone()), window);
            input.compose_text(None, "", None, cx);
        }
        window.render_frame(cx);
        assert_eq!(editor.read(cx).leaves[2].text, "First paragraph");
        assert_eq!(editor.read(cx).leaves[4].source(0).value(), 26);
        assert!(!editor.read(cx).document.has_undo());
    })
    .expect("preedit block geometry");
}

#[gpui_kit::test]
// Spec: docs/modules/selection.md#多击与拖选
fn pointer_clicks_preserve_one_directional_selection(cx: &mut TestAppContext) {
    use gpui_kit::test::{ClickOptions, TestEventExt};
    use gpui_kit::{Modifiers, MouseButton, point, px};
    let (handle, editor) = open(cx, "hello world\n\nnext block");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let click = |window: &mut gpui_kit::Window,
                     cx: &mut gpui_kit::App,
                     at: usize,
                     modifiers: Modifiers,
                     count| {
            let position = {
                let state = editor.read(cx);
                let leaf = &state.leaves[0];
                let geometry = &state.geometry.borrow()[&leaf.key];
                geometry.position_for_index(at) + point(px(0.), geometry.caret_height() / 2.)
            };
            let bounds = window.find("document-editor").bounds();
            window.click_with_options(
                "document-editor",
                ClickOptions::new()
                    .with_offset(position - bounds.origin)
                    .with_modifiers(modifiers)
                    .with_count(count),
                cx,
            );
        };
        click(window, cx, 2, Modifiers::default(), 1);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 2);
        click(
            window,
            cx,
            8,
            Modifiers {
                shift: true,
                ..Default::default()
            },
            1,
        );
        assert_eq!(editor.read(cx).selection().anchor().offset.value(), 2);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 8);
        click(
            window,
            cx,
            1,
            Modifiers {
                control: true,
                ..Default::default()
            },
            1,
        );
        assert_eq!(editor.read(cx).selection().head().offset.value(), 1);
        assert!(editor.read(cx).selection().is_collapsed());
        click(window, cx, 8, Modifiers::default(), 2);
        assert_eq!(editor.read(cx).selection().range().start.value(), 6);
        assert_eq!(editor.read(cx).selection().range().end.value(), 11);
        click(window, cx, 8, Modifiers::default(), 3);
        assert_eq!(
            editor.read(cx).selection().range(),
            editor.read(cx).leaves[0].raw
        );
        assert_eq!(editor.read(cx).snapshot().revision().value(), 0);
        let (start, end) = {
            let state = editor.read(cx);
            let geometry = state.geometry.borrow();
            let first = &geometry[&state.leaves[0].key];
            let last = &geometry[&state.leaves.last().expect("last leaf").key];
            (
                first.position_for_index(0) + point(px(0.), first.caret_height() / 2.),
                last.position_for_index(4) + point(px(0.), last.caret_height() / 2.),
            )
        };
        window.pointer_down(start, MouseButton::Left, cx);
        window.pointer_move(end, Some(MouseButton::Left), cx);
        window.pointer_up(end, MouseButton::Left, cx);
        assert_eq!(editor.read(cx).selection().anchor().offset.value(), 0);
        assert_eq!(editor.read(cx).selection().head().offset.value(), 17);
        assert!(!editor.read(cx).document.has_undo());
    })
    .expect("native pointer events");
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
    let source = "A paragraph whose line wraps when the window becomes narrow, preserving its block anchor.\n\n".repeat(1000);
    cx.update(|cx| {
        gpui_kit::init(cx);
        moino_theme::register_fonts(cx).expect("bundled test fonts");
        moino_theme::apply(moino_theme::BuiltinTheme::Paper, cx);
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
    cx.update_window(handle, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.select_source(
                super::selection(
                    editor.snapshot().text().len(),
                    moino_text::ScalarOffset::new(0),
                ),
                window,
                cx,
            );
            assert!(
                editor
                    .native_selection()
                    .expect("native selection")
                    .reversed
            );
        });
    })
    .expect("backward selection to document start");
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("reveal backward selection head");
    }
    cx.update(|cx| {
        let editor = editor.read(cx);
        let caret = editor
            .caret(editor.selection().head().offset)
            .expect("head geometry");
        let viewport = editor.list.viewport_bounds();
        assert!(caret.top() >= viewport.top() && caret.bottom() <= viewport.bottom());
        assert!(editor.geometry.borrow().len() < 100);
    });
    let anchor = cx
        .update_window(handle, |_, window, cx| {
            editor.update(cx, |editor, _| {
                editor.reveal_caret = false;
                editor.list.scroll_to(gpui_kit::ListOffset {
                    item_ix: 500,
                    offset_in_item: gpui_kit::px(5.),
                });
            });
            window.render_frame(cx);
            let editor = editor.read(cx);
            let top = editor.list.logical_scroll_top();
            (editor.leaves[top.item_ix].key, top.offset_in_item)
        })
        .expect("middle scroll anchor");
    cx.update(|cx| moino_theme::apply(moino_theme::BuiltinTheme::Inkstone, cx));
    cx.simulate_window_resize(
        handle,
        gpui_kit::size(gpui_kit::px(380.), gpui_kit::px(600.)),
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let editor = editor.read(cx);
        let top = editor.list.logical_scroll_top();
        assert_eq!(editor.leaves[top.item_ix].key, anchor.0);
        assert!((top.offset_in_item - anchor.1).abs() <= gpui_kit::px(1.));
        assert!(editor.geometry.borrow().len() < 100);
    })
    .expect("theme and width retain scroll anchor");
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("settle reflow");
    }
    let text_anchor = cx
        .update_window(handle, |_, _, cx| {
            let editor = editor.read(cx);
            let top = editor.list.logical_scroll_top();
            let leaf = &editor.leaves[top.item_ix];
            let geometry = &editor.geometry.borrow()[&leaf.key];
            let byte = geometry.index_for_position(gpui_kit::point(
                geometry.bounds().left(),
                editor.list.viewport_bounds().top(),
            ));
            (
                leaf.key,
                leaf.source(byte),
                geometry.position_for_index(byte).y - editor.list.viewport_bounds().top(),
            )
        })
        .expect("observe semantic scroll anchor");
    cx.update(|cx| {
        gpui_kit::component::Theme::update(cx, |theme| theme.font_size = gpui_kit::px(18.))
    });
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("settle font scale");
    }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let editor = editor.read(cx);
        let top = editor.list.logical_scroll_top();
        assert_eq!(editor.leaves[top.item_ix].key, anchor.0);
        let leaf = editor
            .leaves
            .iter()
            .find(|leaf| leaf.key == text_anchor.0)
            .expect("anchored leaf");
        let geometry = &editor.geometry.borrow()[&leaf.key];
        let y = geometry
            .position_for_index(leaf.byte(text_anchor.1, moino_document::Affinity::After))
            .y
            - editor.list.viewport_bounds().top();
        assert!((y - text_anchor.2).abs() <= gpui_kit::px(1.));
        assert!(editor.geometry.borrow().len() < 100);
    })
    .expect("font scale retains scroll anchor");
}
