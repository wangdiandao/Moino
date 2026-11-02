use super::*;

impl Render for LiveEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.projection_rem != window.rem_size() {
            self.remember_scroll_anchor();
            self.projection_rem = window.rem_size();
            self.list.remeasure_items(0..self.leaves.len());
        }
        let theme = effective_theme(cx);
        if self.projection_theme != theme && self.composition.is_none() {
            self.remember_scroll_anchor();
            self.projection_theme = theme;
            self.leaves =
                projection::project(&self.document.snapshot(), self.document.selection(), theme);
            self.list.remeasure_items(0..self.leaves.len());
        }
        let owner = cx.entity();
        let rows = list(self.list.clone(), move |ix, window, cx| {
            owner.update(cx, |this, cx| this.render_leaf(ix, window, cx))
        })
        .size_full()
        .py(moino_theme::body_size() * 1.6);
        let source = self.source_text.clone();
        let content = div()
            .id("document-editor")
            .test_support()
            .role(Role::MultilineTextInput)
            .aria_label("Document")
            .aria_value(source)
            .track_focus(&self.focus)
            .key_context("MoinoEditor")
            .on_action(cx.listener(|this, _: &MoveToStart, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(
                    Operation::DocumentEdge {
                        end: false,
                        extend: false,
                    },
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &MoveToEnd, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(
                    Operation::DocumentEdge {
                        end: true,
                        extend: false,
                    },
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &SelectToStart, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(
                    Operation::DocumentEdge {
                        end: false,
                        extend: true,
                    },
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &SelectToEnd, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(
                    Operation::DocumentEdge {
                        end: true,
                        extend: true,
                    },
                    window,
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &commands::Bold, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Bold), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Italic, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Italic), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Code, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Code), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Strike, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Strike), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Highlight, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Highlight), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Subscript, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Subscript), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Superscript, window, cx| {
                this.dispatch(Intent::Inline(InlineFormat::Superscript), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Paragraph, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Paragraph), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Heading1, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Heading(1)), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Heading2, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Heading(2)), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Heading3, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Heading(3)), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Heading4, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Heading(4)), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Heading5, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Heading(5)), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Heading6, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Heading(6)), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::Quote, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::Quote), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::BulletList, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::BulletList), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::OrderedList, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::OrderedList), window, cx)
            }))
            .on_action(cx.listener(|this, _: &commands::TaskList, window, cx| {
                this.dispatch(Intent::Block(BlockFormat::TaskList), window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &commands::ClearFormatting, window, cx| {
                    this.dispatch(Intent::ClearFormatting, window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &Split, window, cx| this.dispatch(Intent::Enter, window, cx)),
            )
            .on_action(cx.listener(|this, _: &DeleteBackward, window, cx| {
                this.dispatch(Intent::Backspace, window, cx)
            }))
            .on_action(cx.listener(|this, _: &DeleteForward, window, cx| {
                this.dispatch(Intent::Delete, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Indent, window, cx| {
                this.dispatch(Intent::Tab { reverse: false }, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Outdent, window, cx| {
                this.dispatch(Intent::Tab { reverse: true }, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Undo, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(Operation::Undo, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Redo, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(Operation::Redo, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(Operation::SelectAll, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Copy, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(Operation::Copy { cut: false }, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Cut, window, cx| {
                this.finish_composition(window, cx);
                this.enqueue(Operation::Copy { cut: true }, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Paste, window, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    this.dispatch(Intent::Paste(text), window, cx);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "left" | "right" => this.move_horizontal(
                        event.keystroke.key == "right",
                        event.keystroke.modifiers.shift,
                        window,
                        cx,
                    ),
                    "up" | "down" => {
                        this.finish_composition(window, cx);
                        this.enqueue(
                            Operation::Vertical {
                                forward: event.keystroke.key == "down",
                                extend: event.keystroke.modifiers.shift,
                            },
                            window,
                            cx,
                        );
                    }
                    "home" | "end" => {
                        this.finish_composition(window, cx);
                        this.enqueue(
                            Operation::LineEdge {
                                end: event.keystroke.key == "end",
                                extend: event.keystroke.modifiers.shift,
                            },
                            window,
                            cx,
                        );
                    }
                    _ => {}
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.drag_position = None;
                    this.focus.focus(window, cx);
                    if let Some(at) = this.hit(event.position) {
                        if event.click_count >= 3 {
                            if let Some(leaf) = this
                                .leaves
                                .iter()
                                .find(|leaf| leaf.raw.start <= at && at <= leaf.raw.end)
                            {
                                this.select(selection(leaf.raw.start, leaf.raw.end), window, cx);
                            }
                            return;
                        }
                        if event.click_count == 2 {
                            if let Some(range) = this
                                .document
                                .snapshot()
                                .text()
                                .words()
                                .into_iter()
                                .find(|range| range.start <= at && at < range.end)
                            {
                                this.select(selection(range.start, range.end), window, cx);
                            }
                            return;
                        }
                        let anchor = if event.modifiers.shift {
                            this.document.selection().anchor().offset
                        } else {
                            at
                        };
                        this.select(selection(anchor, at), window, cx);
                        this.drag_position = Some(event.position);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                let Some(previous) = this.drag_position else {
                    return;
                };
                if event.pressed_button != Some(MouseButton::Left) {
                    this.drag_position = None;
                    return;
                }
                if event.position == previous {
                    return;
                }
                this.drag_position = Some(event.position);
                if let Some(at) = this.hit(event.position) {
                    this.select(
                        selection(this.document.selection().anchor().offset, at),
                        window,
                        cx,
                    );
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.drag_position = None),
            )
            .size_full()
            .relative()
            .text_size(moino_theme::body_size())
            .line_height(relative(1.6))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(rows)
            .when_some(self.error.clone(), |this, code| {
                let message = moino_ui::editing_error(&code);
                this.child(
                    div()
                        .id("editor-error")
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
            .into_any_element();
        crate::live_surface::LiveSurface::new(content, cx.entity(), self.geometry.clone())
    }
}
