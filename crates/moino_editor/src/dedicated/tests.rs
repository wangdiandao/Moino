use super::DedicatedProbe;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Render, ScrollHandle, TestAppContext, TestSupportExt,
    Window, div, point, prelude::*, px,
};

#[gpui_kit::test]
// Spec: docs/acceptance/m0.md#技术原型
fn mounted_native_input_cancels_then_commits_and_rejects_events_after_unmount(
    cx: &mut TestAppContext,
) {
    use gpui_kit::test::TestInput;
    use gpui_kit::{ElementInputHandler, EntityInputHandler};
    cx.update(gpui_kit::init);
    let (handle, probe) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            cx.new(|cx| DedicatedProbe::new(window, cx))
        })
        .expect("probe window")
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(probe.read(cx).geometry.borrow().contains_key(&2));
        window.click("mount-code", cx);
        assert!(probe.read(cx).geometry.borrow().is_empty());
        probe.update(cx, |probe, cx| {
            let end = probe.editor.read(cx).value().encode_utf16().count();
            probe.replace_text_in_range(Some(0..end), "ab😀中", window, cx);
        });
        window.press("secondary-end", cx);
        window.press("shift-left", cx);
        window.press("shift-left", cx);
        let bounds = window.find("code-slot").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, probe.clone()), window);
        input.compose_text(Some(3..3), "invalid", None, cx);
        assert_eq!(probe.read(cx).editor.read(cx).value().as_ref(), "ab😀中");
        assert_eq!(input.marked_text_range(cx), None);
        input.compose_text(None, "临时", None, cx);
        input.compose_text(None, "", None, cx);
        assert_eq!(probe.read(cx).editor.read(cx).value().as_ref(), "ab😀中");
        let selected = input.selected_text_range(cx).expect("backward selection");
        assert_eq!(selected.range, 2..5);
        assert!(selected.reversed);
        input.compose_text(None, "中\r\n😀", Some(3..3), cx);
        assert_eq!(
            input.selected_text_range(cx).expect("preedit caret").range,
            4..4
        );
        window.click("mount-code", cx);
        assert_eq!(probe.read(cx).source.as_ref(), "ab中\n😀");
        assert!(!probe.read(cx).is_active());
        let mut stale = TestInput::new(ElementInputHandler::new(bounds, probe.clone()), window);
        stale.commit_text("must not insert", cx);
        stale.compose_text(None, "", None, cx);
        assert_eq!(probe.read(cx).editor.read(cx).value().as_ref(), "ab中\n😀");
        assert!(stale.selected_text_range(cx).is_none());
        window.click("mount-code", cx);
        let mut input = TestInput::new(ElementInputHandler::new(bounds, probe.clone()), window);
        assert_eq!(
            input
                .selected_text_range(cx)
                .expect("remounted caret")
                .range,
            4..4
        );
        assert_eq!(input.marked_text_range(cx), None);
        probe.update(cx, |probe, cx| {
            probe.set_selected_text_range(6..6, window, cx)
        });
        let mut input = TestInput::new(ElementInputHandler::new(bounds, probe.clone()), window);
        assert_eq!(
            input
                .selected_text_range(cx)
                .expect("native caret move")
                .range,
            6..6
        );
        probe.update(cx, |probe, cx| {
            probe.set_selected_text_range(4..4, window, cx)
        });
        let mut input = TestInput::new(ElementInputHandler::new(bounds, probe.clone()), window);
        input.commit_text("X", cx);
        input.compose_text(None, "Q", None, cx);
        window.press("shift-left", cx);
        let mut input = TestInput::new(ElementInputHandler::new(bounds, probe.clone()), window);
        assert_eq!(input.marked_text_range(cx), None);
        input.compose_text(None, "", None, cx);
        window.click("mount-code", cx);
        assert_eq!(probe.read(cx).source.as_ref(), "ab中\nXQ😀");
        window.render_frame(cx);
        assert!(probe.read(cx).geometry.borrow().contains_key(&2));
    })
    .expect("mounting input lifecycle");
}

struct ScrolledProbe {
    probe: Entity<DedicatedProbe>,
    scroll: ScrollHandle,
}

impl Render for ScrolledProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("probe-viewport")
            .test_support()
            .h(px(180.))
            .w_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(div().h(px(200.)))
            .child(self.probe.clone())
            .child(div().h(px(200.)))
    }
}

#[gpui_kit::test]
// Spec: docs/acceptance/m0.md#技术原型
fn mounting_and_leaving_the_editor_preserve_the_scrolled_text_anchor(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, fixture) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            cx.new(|cx| ScrolledProbe {
                probe: cx.new(|cx| DedicatedProbe::new(window, cx)),
                scroll: ScrollHandle::new(),
            })
        })
        .expect("scrolled probe window")
    });
    let (offset, slot, origin, text_width) = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            fixture.read(cx).scroll.set_offset(point(px(0.), px(-180.)));
            window.render_frame(cx);
            let fixture = fixture.read(cx);
            let offset = fixture.scroll.offset();
            assert!(offset.y < px(0.), "fixture must actually be scrolled");
            let origin = fixture.probe.read(cx).geometry.borrow()[&2].position_for_index(0);
            let end = fixture.probe.read(cx).geometry.borrow()[&2].position_for_index(3);
            (
                offset,
                window.find("code-slot").bounds(),
                origin,
                end.x - origin.x,
            )
        })
        .expect("scrolled static geometry");
    cx.update_window(handle, |_, window, cx| window.click("mount-code", cx))
        .expect("mount editor");
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("settle focus and geometry");
    }
    cx.update_window(handle, |_, window, cx| {
        let fixture = fixture.read(cx);
        assert_eq!(fixture.scroll.offset(), offset);
        assert_eq!(window.find("code-slot").bounds(), slot);
        let active = fixture
            .probe
            .read(cx)
            .editor
            .read(cx)
            .range_to_bounds(&(0..3))
            .expect("first Editor text range");
        assert!((active.origin.x - origin.x).abs() <= px(1.));
        assert!((active.origin.y - origin.y).abs() <= px(1.));
        assert!((active.size.width - text_width).abs() <= px(1.));
        window.click("mount-code", cx);
        window.render_frame(cx);
    })
    .expect("leave editor");
    cx.update_window(handle, |_, window, cx| {
        window.simulate_next_frame(cx);
        window.render_frame(cx);
        let fixture = fixture.read(cx);
        assert_eq!(fixture.scroll.offset(), offset);
        assert_eq!(window.find("code-slot").bounds(), slot);
        let restored = fixture.probe.read(cx).geometry.borrow()[&2].position_for_index(0);
        assert!((restored.x - origin.x).abs() <= px(1.));
        assert!((restored.y - origin.y).abs() <= px(1.));
    })
    .expect("restored static scroll anchor");
}

#[gpui_kit::test]
// Spec: docs/acceptance/m0.md#技术原型
fn mounted_editor_preserves_text_origin_and_surrounding_geometry(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (handle, probe) = cx.update(|cx| {
        gpui_kit::open_window(Default::default(), cx, |window, cx| {
            cx.new(|cx| DedicatedProbe::new(window, cx))
        })
        .expect("window")
    });
    cx.update_window(handle, |_, window, cx| {
        for font_size in [14., 18., 24.] {
            gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(font_size));
            window.render_frame(cx);
            let before = window.find("code-slot").bounds();
            let text_origin = probe.read(cx).geometry.borrow()[&2].position_for_index(0);
            let text_end = probe.read(cx).geometry.borrow()[&2].position_for_index(3);
            let line_height = probe.read(cx).geometry.borrow()[&2].line_height();
            window.click("mount-code", cx);
            let after = window.find("code-slot").bounds();
            assert_eq!(before, after);
            let active = probe
                .read(cx)
                .editor
                .read(cx)
                .range_to_bounds(&(0..3))
                .expect("first Editor text range");
            assert!(
                (active.origin.x - text_origin.x).abs() <= px(1.),
                "horizontal text drift: {active:?} vs {text_origin:?}"
            );
            assert!(
                (active.origin.y - text_origin.y).abs() <= px(1.),
                "vertical text drift: {active:?} vs {text_origin:?}"
            );
            assert!((active.size.width - (text_end.x - text_origin.x)).abs() <= px(1.));
            let active_line_height = probe
                .read(cx)
                .editor
                .read(cx)
                .line_height()
                .expect("Editor row geometry");
            assert!((active_line_height - line_height).abs() <= px(1.));
            window.click("mount-code", cx);
            assert_eq!(window.find("code-slot").bounds(), before);
        }
    })
    .expect("update");
}
