//! Source commands and the native Editor surface.
use super::*;

impl Render for SourceEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme;
        use gpui_kit::component::input::*;
        div()
            .id("source-surface")
            .test_support()
            .key_context("MoinoSource")
            .size_full()
            .relative()
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Copy, window, cx| {
                    this.leave(window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(cx.listener(Self::before_edit::<Backspace>))
            .capture_action(cx.listener(Self::before_edit::<Delete>))
            .capture_action(cx.listener(Self::before_edit::<Indent>))
            .capture_action(cx.listener(Self::before_edit::<Outdent>))
            .capture_action(cx.listener(Self::before_edit::<DeleteToBeginningOfLine>))
            .capture_action(cx.listener(Self::before_edit::<DeleteToEndOfLine>))
            .capture_action(cx.listener(Self::before_edit::<DeleteToPreviousWordStart>))
            .capture_action(cx.listener(Self::before_edit::<DeleteToNextWordEnd>))
            .capture_action(cx.listener(Self::before_edit::<IndentInline>))
            .capture_action(cx.listener(Self::before_edit::<OutdentInline>))
            .on_action(cx.listener(|this, _: &crate::Undo, window, cx| {
                this.leave(window, cx);
                this.document
                    .update(cx, |document, cx| document.undo(window, cx));
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &crate::Redo, window, cx| {
                this.leave(window, cx);
                this.document
                    .update(cx, |document, cx| document.redo(window, cx));
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(Self::before_navigation::<MoveLeft>))
            .capture_action(cx.listener(Self::before_navigation::<MoveRight>))
            .capture_action(cx.listener(Self::before_navigation::<MoveUp>))
            .capture_action(cx.listener(Self::before_navigation::<MoveDown>))
            .capture_action(cx.listener(Self::before_navigation::<MoveHome>))
            .capture_action(cx.listener(Self::before_navigation::<MoveEnd>))
            .capture_action(cx.listener(Self::before_navigation::<MovePageUp>))
            .capture_action(cx.listener(Self::before_navigation::<MovePageDown>))
            .capture_action(cx.listener(Self::before_navigation::<MoveToStart>))
            .capture_action(cx.listener(Self::before_navigation::<MoveToEnd>))
            .capture_action(cx.listener(Self::before_navigation::<MoveToStartOfLine>))
            .capture_action(cx.listener(Self::before_navigation::<MoveToEndOfLine>))
            .capture_action(cx.listener(Self::before_navigation::<MoveToPreviousWord>))
            .capture_action(cx.listener(Self::before_navigation::<MoveToNextWord>))
            .capture_action(cx.listener(Self::before_navigation::<SelectToStart>))
            .capture_action(cx.listener(Self::before_navigation::<SelectToEnd>))
            .capture_action(cx.listener(Self::before_navigation::<SelectToStartOfLine>))
            .capture_action(cx.listener(Self::before_navigation::<SelectToEndOfLine>))
            .capture_action(cx.listener(Self::before_navigation::<SelectToPreviousWordStart>))
            .capture_action(cx.listener(Self::before_navigation::<SelectToNextWordEnd>))
            .capture_action(
                cx.listener(Self::before_navigation::<gpui_kit::base::actions::SelectLeft>),
            )
            .capture_action(
                cx.listener(Self::before_navigation::<gpui_kit::base::actions::SelectRight>),
            )
            .capture_action(
                cx.listener(Self::before_navigation::<gpui_kit::base::actions::SelectUp>),
            )
            .capture_action(
                cx.listener(Self::before_navigation::<gpui_kit::base::actions::SelectDown>),
            )
            .capture_key_down(cx.listener(|this, _, window, cx| {
                this.normalized_selection = None;
                this.publish_selection(window, cx);
            }))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::SelectAll, window, cx| {
                    this.leave(window, cx);
                    cx.propagate();
                },
            ))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    this.leave(window, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.publish_selection(window, cx)),
            )
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Paste, window, cx| {
                    this.before_command(Origin::Paste, window, cx);
                    cx.propagate();
                },
            ))
            .capture_action(
                cx.listener(|this, _: &gpui_kit::component::input::Cut, window, cx| {
                    this.before_command(Origin::Command, window, cx);
                    cx.propagate();
                }),
            )
            .capture_action(cx.listener(Self::before_edit::<Enter>))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Undo, window, cx| {
                    this.leave(window, cx);
                    this.document
                        .update(cx, |document, cx| document.undo(window, cx));
                    cx.stop_propagation();
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Redo, window, cx| {
                    this.leave(window, cx);
                    this.document
                        .update(cx, |document, cx| document.redo(window, cx));
                    cx.stop_propagation();
                },
            ))
            .child(
                Editor::new(&self.state)
                    .readonly(self.document.read(cx).is_read_only())
                    .appearance(false)
                    .bordered(false)
                    .context_menu(|menu, _, _| menu)
                    .h_full()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(moino_theme::code_size())
                    .line_height(relative(1.6))
                    .aria_label("Markdown source"),
            )
            .child(input::native_input(cx.entity(), self.state.clone()))
            .when_some(self.document.read(cx).error_code(), |surface, code| {
                let message = moino_ui::editing_error(code);
                surface.child(
                    div()
                        .id("source-error")
                        .test_support()
                        .role(Role::Alert)
                        .aria_label(message.clone())
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().danger)
                        .px_3()
                        .py_2()
                        .child(message),
                )
            })
    }
}
