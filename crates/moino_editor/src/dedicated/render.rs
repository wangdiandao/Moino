//! Mounted and static code share typography and the same fixed slot.
use super::*;
use crate::surface::HostSurface;
use gpui_kit::base::text::editor_host::{
    BlockNode, FlowPosition, InlineNode, Paragraph, RenderContext,
};
use gpui_kit::component::{ActiveTheme, Sizable, Size, button::Button};

impl Render for DedicatedProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::input::*;
        let content = if self.is_active() {
            div()
                .relative()
                .size_full()
                .capture_action(cx.listener(Self::before_action::<MoveLeft>))
                .capture_action(cx.listener(Self::before_action::<MoveRight>))
                .capture_action(cx.listener(Self::before_action::<MoveUp>))
                .capture_action(cx.listener(Self::before_action::<MoveDown>))
                .capture_action(cx.listener(Self::before_action::<MoveHome>))
                .capture_action(cx.listener(Self::before_action::<MoveEnd>))
                .capture_action(cx.listener(Self::before_action::<MoveToStart>))
                .capture_action(cx.listener(Self::before_action::<MoveToEnd>))
                .capture_action(cx.listener(Self::before_action::<MovePageUp>))
                .capture_action(cx.listener(Self::before_action::<MovePageDown>))
                .capture_action(cx.listener(Self::before_action::<MoveToStartOfLine>))
                .capture_action(cx.listener(Self::before_action::<MoveToEndOfLine>))
                .capture_action(cx.listener(Self::before_action::<MoveToPreviousWord>))
                .capture_action(cx.listener(Self::before_action::<MoveToNextWord>))
                .capture_action(cx.listener(Self::before_action::<SelectToStart>))
                .capture_action(cx.listener(Self::before_action::<SelectToEnd>))
                .capture_action(cx.listener(Self::before_action::<SelectToStartOfLine>))
                .capture_action(cx.listener(Self::before_action::<SelectToEndOfLine>))
                .capture_action(cx.listener(Self::before_action::<SelectToPreviousWordStart>))
                .capture_action(cx.listener(Self::before_action::<SelectToNextWordEnd>))
                .capture_action(
                    cx.listener(Self::before_action::<gpui_kit::base::actions::SelectLeft>),
                )
                .capture_action(
                    cx.listener(Self::before_action::<gpui_kit::base::actions::SelectRight>),
                )
                .capture_action(
                    cx.listener(Self::before_action::<gpui_kit::base::actions::SelectUp>),
                )
                .capture_action(
                    cx.listener(Self::before_action::<gpui_kit::base::actions::SelectDown>),
                )
                .capture_action(cx.listener(Self::before_action::<SelectAll>))
                .capture_action(cx.listener(Self::before_action::<Backspace>))
                .capture_action(cx.listener(Self::before_action::<Delete>))
                .capture_action(cx.listener(Self::before_action::<DeleteToBeginningOfLine>))
                .capture_action(cx.listener(Self::before_action::<DeleteToEndOfLine>))
                .capture_action(cx.listener(Self::before_action::<DeleteToPreviousWordStart>))
                .capture_action(cx.listener(Self::before_action::<DeleteToNextWordEnd>))
                .capture_action(cx.listener(Self::before_action::<Indent>))
                .capture_action(cx.listener(Self::before_action::<Outdent>))
                .capture_action(cx.listener(Self::before_action::<IndentInline>))
                .capture_action(cx.listener(Self::before_action::<OutdentInline>))
                .capture_action(cx.listener(Self::before_action::<Enter>))
                .capture_action(cx.listener(Self::before_action::<Undo>))
                .capture_action(cx.listener(Self::before_action::<Redo>))
                .capture_action(cx.listener(Self::before_action::<Paste>))
                .capture_action(cx.listener(Self::before_action::<Cut>))
                .capture_action(cx.listener(Self::before_action::<Copy>))
                .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.button == MouseButton::Left {
                        this.finish_preedit(window, cx);
                    }
                }))
                .child(
                    Editor::new(&self.editor)
                        .with_size(Size::XSmall)
                        .appearance(false)
                        .bordered(false)
                        .p_0()
                        .h_full()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_base()
                        .line_height(relative(1.5))
                        .aria_label("Code source"),
                )
                .child(crate::native_input::editor_input(
                    cx.entity(),
                    self.editor.clone(),
                ))
                .into_any_element()
        } else {
            let mut paragraph = Paragraph::default().with_leaf_key(2);
            paragraph.push(InlineNode::new(if self.source.is_empty() {
                " ".into()
            } else {
                self.source.clone()
            }));
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
                    .px(Size::XSmall.input_px())
                    .pl(Size::XSmall.input_px() + EditorState::text_gutter_margin())
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
                    .label(if self.is_active() {
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
