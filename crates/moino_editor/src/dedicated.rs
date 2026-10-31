use crate::surface::HostSurface;
use gpui_kit::base::text::editor_host::{
    BlockNode, FlowPosition, InlineNode, LeafGeometry, Paragraph, RenderContext,
};
use gpui_kit::component::{
    ActiveTheme, Sizable, Size,
    button::Button,
    input::{Editor, EditorState},
};
use gpui_kit::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Static-to-Editor mounting probe with one retained input state and shared typography.
pub struct DedicatedProbe {
    editor: Entity<EditorState>,
    source: SharedString,
    active: bool,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
}

impl DedicatedProbe {
    /// Creates a two-line source sample without folding or line numbers.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source: SharedString = "let 墨 = 42;\n// 日本語 한국어 😀".into();
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(false)
                .folding(false)
                .default_value(source.clone())
        });
        Self {
            editor,
            source,
            active: false,
            geometry: Rc::default(),
        }
    }
    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active = !self.active;
        if self.active {
            self.editor.update(cx, |editor, cx| {
                editor.set_value(self.source.clone(), window, cx);
                editor.focus(window, cx);
            });
        } else {
            self.source = self.editor.read(cx).value();
        }
        cx.notify();
    }
}

impl Render for DedicatedProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.active {
            Editor::new(&self.editor)
                .with_size(Size::XSmall)
                .appearance(false)
                .bordered(false)
                .p_0()
                .h_full()
                .font_family(cx.theme().mono_font_family.clone())
                .text_base()
                .line_height(relative(1.5))
                .aria_label("Code source")
                .into_any_element()
        } else {
            let mut paragraph = Paragraph::default().with_leaf_key(2);
            paragraph.push(InlineNode::new(self.source.clone()));
            let block = BlockNode::Paragraph(paragraph);
            let context = RenderContext::from_app(cx);
            let child = block.render_with_id(
                "static-code",
                FlowPosition::at(&[&block], 0, true),
                &context,
                window,
                cx,
            );
            HostSurface::geometry_only(
                div()
                    .w_full()
                    // Match the external Editor's public density geometry.
                    .px(Size::XSmall.input_px())
                    .py(Size::XSmall.input_py())
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_base()
                    .line_height(relative(1.5))
                    .child(child)
                    .into_any_element(),
                self.geometry.clone(),
            )
            .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Button::new("mount-code")
                    .label(if self.active {
                        "结束编辑"
                    } else {
                        "编辑代码"
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx))),
            )
            .child(
                div()
                    .id("code-slot")
                    .test_support()
                    .w_full()
                    .h_24()
                    .p_4()
                    .bg(cx.theme().muted)
                    .child(content),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::DedicatedProbe;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, px};

    #[gpui_kit::test]
    fn mounted_editor_preserves_text_origin_and_surrounding_geometry(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (handle, probe) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| DedicatedProbe::new(window, cx))
            })
            .expect("window")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let before = window.find("code-slot").bounds();
            let text_origin = probe.read(cx).geometry.borrow()[&2].position_for_index(0);
            window.click("mount-code", cx);
            let after = window.find("code-slot").bounds();
            assert_eq!(before, after);
            let active = probe.read(cx).editor.read(cx).input_bounds().origin;
            assert!(
                (active.x - text_origin.x).abs() <= px(1.),
                "horizontal text drift: {active:?} vs {text_origin:?}"
            );
            assert!(
                (active.y - text_origin.y).abs() <= px(1.),
                "vertical text drift: {active:?} vs {text_origin:?}"
            );
            window.click("mount-code", cx);
            assert_eq!(window.find("code-slot").bounds(), before);
        })
        .expect("update");
    }
}
