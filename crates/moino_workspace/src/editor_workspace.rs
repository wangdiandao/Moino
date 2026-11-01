//! Formal document window, shared surfaces and native menu dispatch.
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    menu::AppMenuBar,
    status_bar::StatusBar,
};
use gpui_kit::{prelude::*, *};
use moino_editor::{LiveEditor, Redo, SourceEditor, Undo};
use moino_ui::text;

actions!(
    view,
    [
        /// Switches the shared document editing surface.
        ToggleSource,
        /// Changes the built-in appearance.
        ToggleAppearance,
        /// Opens the application menu.
        ToggleMenu,
        /// Dismisses the top-level application menu.
        DismissMenu,
    ]
);
actions!(
    app,
    [
        /// Quits the application.
        Quit,
    ]
);

/// Runs the document workspace with bundled product fonts and native decorations.
pub fn run() {
    gpui_kit::application().run(|cx| {
        init(cx);
        gpui_kit::open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some(
                        format!("{} — {}", text("document.untitled"), text("product.name")).into(),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| EditorWorkspace::new(window, cx)),
        )
        .expect("document window");
        cx.activate(true);
    });
}

pub(super) fn init(cx: &mut App) {
    moino_ui::set_language(&moino_platform::system_language());
    gpui_kit::init(cx);
    moino_theme::register_fonts(cx).expect("bundled product fonts");
    moino_theme::apply(
        moino_theme::BuiltinTheme::from_appearance(cx.window_appearance()),
        cx,
    );
    moino_editor::init(cx);
    let source = KeyBinding::load("secondary-/", Box::new(ToggleSource), None, false, None, cx)
        .expect("static source shortcut");
    cx.bind_keys([
        source,
        KeyBinding::new("f10", ToggleMenu, None),
        KeyBinding::new("escape", DismissMenu, Some("MoinoWindow")),
        KeyBinding::new("escape", DismissMenu, Some("AppMenuBar")),
        KeyBinding::new(moino_platform::quit_shortcut(), Quit, None),
    ]);
    let menus = || {
        vec![
            Menu::new(text("menu.file")).items([MenuItem::action(text("menu.quit"), Quit)]),
            Menu::new(text("menu.edit")).items([
                MenuItem::action(text("menu.undo"), Undo),
                MenuItem::action(text("menu.redo"), Redo),
            ]),
            Menu::new(text("menu.view")).items([
                MenuItem::action(text("mode.source"), ToggleSource),
                MenuItem::action(text("theme.inkstone"), ToggleAppearance),
            ]),
        ]
    };
    cx.set_menus(menus());
    gpui_kit::component::GlobalState::global_mut(cx)
        .set_app_menus(menus().into_iter().map(Menu::owned).collect());
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// Owns one document's mode, focus and status presentation.
pub struct EditorWorkspace {
    editor: Entity<LiveEditor>,
    source: Entity<SourceEditor>,
    is_source: bool,
    menu: Entity<AppMenuBar>,
    menu_visible: bool,
    menu_focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    alt_active: bool,
    alt_pending: bool,
    requested_mode: Option<bool>,
    _subscriptions: Vec<Subscription>,
}
impl EditorWorkspace {
    /// Opens a blank, focused live document.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(LiveEditor::empty);
        let source = cx.new(|cx| SourceEditor::new(editor.clone(), window, cx));
        window.focus(&editor.read(cx).focus_handle(), cx);
        let subscriptions = vec![
            cx.observe_in(&editor, window, |this, _, window, cx| {
                if !this.editor.read(cx).is_pending()
                    && let Some(mode) = this.requested_mode.take()
                    && mode != this.is_source
                {
                    this.toggle_source(&ToggleSource, window, cx);
                }
                cx.notify();
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                moino_theme::sync_system(window.appearance(), cx);
            }),
        ];
        Self {
            editor,
            source,
            is_source: false,
            menu: AppMenuBar::new(cx),
            menu_visible: false,
            menu_focus: cx.focus_handle(),
            previous_focus: None,
            alt_active: false,
            alt_pending: false,
            requested_mode: None,
            _subscriptions: subscriptions,
        }
    }
    fn toggle_source(&mut self, _: &ToggleSource, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.read(cx).is_pending() {
            self.requested_mode = Some(!self.requested_mode.unwrap_or(self.is_source));
            cx.notify();
            return;
        }
        if self.is_source {
            self.source
                .update(cx, |source, cx| source.leave(window, cx));
            window.focus(&self.editor.read(cx).focus_handle(), cx);
        } else {
            self.source
                .update(cx, |source, cx| source.focus(window, cx));
        }
        self.is_source = !self.is_source;
        cx.notify();
    }
    fn toggle_appearance(&mut self, _: &ToggleAppearance, _: &mut Window, cx: &mut Context<Self>) {
        let current = cx.global::<moino_theme::ThemeState>().theme();
        moino_theme::set_mode(
            if current == moino_theme::BuiltinTheme::Paper {
                moino_theme::AppearanceMode::Dark
            } else {
                moino_theme::AppearanceMode::Light
            },
            cx,
        );
    }
    fn dismiss_menu(&mut self, _: &DismissMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_visible = false;
        if let Some(focus) = self.previous_focus.take() {
            window.focus(&focus, cx);
        }
        cx.notify();
    }
    fn toggle_menu(&mut self, _: &ToggleMenu, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_visible {
            self.dismiss_menu(&DismissMenu, window, cx);
            return;
        }
        self.previous_focus = window.focused(cx);
        self.menu_visible = true;
        window.focus(&self.menu_focus, cx);
        let focus = self.menu_focus.clone();
        window.on_next_frame(move |window, cx| {
            window.focus(&focus, cx);
            window.focus_next(cx);
        });
        cx.notify();
    }
}
impl Render for EditorWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.editor.read(cx).snapshot();
        let count = snapshot.text().len().value();
        div()
            .id("editor-workspace")
            .test_support()
            .key_context("MoinoWindow")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::toggle_source))
            .on_action(cx.listener(Self::toggle_appearance))
            .on_action(cx.listener(Self::toggle_menu))
            .on_action(cx.listener(Self::dismiss_menu))
            .on_action(cx.listener(|this, _: &Undo, window, cx| {
                this.editor.update(cx, |editor, cx| editor.undo(window, cx))
            }))
            .on_action(cx.listener(|this, _: &Redo, window, cx| {
                this.editor.update(cx, |editor, cx| editor.redo(window, cx))
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, _| {
                if event.keystroke.modifiers.alt {
                    this.alt_pending = false;
                }
            }))
            .on_modifiers_changed(
                cx.listener(|this, event: &ModifiersChangedEvent, window, cx| {
                    if event.modifiers.alt && !this.alt_active {
                        this.alt_pending = true;
                    }
                    if event.modifiers.shift || event.modifiers.control || event.modifiers.platform
                    {
                        this.alt_pending = false;
                    }
                    if !event.modifiers.alt
                        && this.alt_active
                        && this.alt_pending
                        && !this.menu_visible
                        && moino_platform::has_window_menu()
                    {
                        this.toggle_menu(&ToggleMenu, window, cx);
                    }
                    this.alt_active = event.modifiers.alt;
                }),
            )
            .child(
                div()
                    .id("content-viewport")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(if self.is_source {
                        self.source.clone().into_any_element()
                    } else {
                        self.editor.clone().into_any_element()
                    }),
            )
            .child(
                StatusBar::new()
                    .left(
                        Button::new("application-menu")
                            .ghost()
                            .small()
                            .label(text("menu.application"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_menu(&ToggleMenu, window, cx)
                            })),
                    )
                    .left(
                        Button::new("toggle-source")
                            .ghost()
                            .small()
                            .label(text(if self.is_source {
                                "mode.source"
                            } else {
                                "mode.preview"
                            }))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_source(&ToggleSource, window, cx)
                            })),
                    )
                    .right(
                        div()
                            .id("character-count")
                            .test_support()
                            .role(Role::Status)
                            .aria_label(format!("{} {count}", text("status.characters")))
                            .child(format!("{} {count}", text("status.characters"))),
                    )
                    .right(div().text_xs().child(text("status.spelling")))
                    .right(
                        Button::new("appearance")
                            .ghost()
                            .small()
                            .label(text(
                                if cx.global::<moino_theme::ThemeState>().theme()
                                    == moino_theme::BuiltinTheme::Paper
                                {
                                    "theme.paper"
                                } else {
                                    "theme.inkstone"
                                },
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_appearance(&ToggleAppearance, window, cx)
                            })),
                    ),
            )
            .when(self.menu_visible, |this| {
                this.child(
                    div()
                        .id("menu-overlay")
                        .test_support()
                        .track_focus(&self.menu_focus)
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bg(cx.theme().popover)
                        .child(self.menu.clone()),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorWorkspace, init};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};

    #[gpui_kit::test]
    // Spec: docs/modules/view-modes.md#源码模式
    fn source_roundtrip_uses_one_document_and_one_history(cx: &mut TestAppContext) {
        cx.update(init);
        let (handle, workspace) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| EditorWorkspace::new(window, cx))
            })
            .expect("workspace")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.input("hello", cx);
        })
        .expect("live typing");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.click("toggle-source", cx);
            window.input("😀", cx);
        })
        .expect("source typing");
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                workspace
                    .read(cx)
                    .editor
                    .read(cx)
                    .snapshot()
                    .text()
                    .to_string(),
                "hello😀"
            )
        });
        cx.update_window(handle, |_, window, cx| {
            window.press("secondary-z", cx);
        })
        .expect("source undo");
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                workspace
                    .read(cx)
                    .editor
                    .read(cx)
                    .snapshot()
                    .text()
                    .to_string(),
                "hello"
            )
        });
        cx.update_window(handle, |_, window, cx| {
            window.click("toggle-source", cx);
            window.press("secondary-z", cx);
        })
        .expect("live undo");
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                workspace
                    .read(cx)
                    .editor
                    .read(cx)
                    .snapshot()
                    .text()
                    .to_string(),
                ""
            )
        });
    }

    #[gpui_kit::test]
    // Spec: docs/design/layout-geometry.md#窗口装饰与菜单
    fn theme_and_menu_keep_document_viewport_geometry(cx: &mut TestAppContext) {
        cx.update(init);
        let (handle, workspace) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| EditorWorkspace::new(window, cx))
            })
            .expect("workspace")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let before = window.find("content-viewport").bounds();
            window.click("appearance", cx);
            assert_eq!(window.find("content-viewport").bounds(), before);
            window.press("f10", cx);
            assert_eq!(window.find("content-viewport").bounds(), before);
            window.press("escape", cx);
            assert_eq!(window.find("content-viewport").bounds(), before);
            assert_eq!(
                workspace
                    .read(cx)
                    .editor
                    .read(cx)
                    .snapshot()
                    .revision()
                    .value(),
                0
            );
        })
        .expect("layout invariants");
    }

    #[gpui_kit::test]
    // Spec: docs/modules/view-modes.md#源码模式
    fn switching_waits_for_pending_input_and_preserves_backward_selection(cx: &mut TestAppContext) {
        cx.update(init);
        let (handle, workspace) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| EditorWorkspace::new(window, cx))
            })
            .expect("workspace")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.input("中😀a", cx);
            assert!(workspace.read(cx).editor.read(cx).is_pending());
            window.click("toggle-source", cx);
            assert_eq!(workspace.read(cx).requested_mode, Some(true));
        })
        .expect("switch during derivation");
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(workspace.read(cx).is_source);
            assert_eq!(
                workspace
                    .read(cx)
                    .editor
                    .read(cx)
                    .snapshot()
                    .text()
                    .to_string(),
                "中😀a"
            );
        });
        cx.update_window(handle, |_, window, cx| {
            window.click("toggle-source", cx);
        })
        .expect("return to live");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.press("shift-left", cx);
            window.press("shift-left", cx);
            window.click("toggle-source", cx);
            window.click("toggle-source", cx);
        })
        .expect("selection roundtrip");
        cx.run_until_parked();
        cx.update(|cx| {
            let selected = workspace.read(cx).editor.read(cx).selection();
            assert_eq!(selected.anchor().offset.value(), 3);
            assert_eq!(selected.head().offset.value(), 1);
        });
    }
}
