//! L3 M0 validation workspace: native window, menu overlay and actual rendering probes.
use gpui_kit::component::{ActiveTheme, menu::AppMenuBar};
use gpui_kit::{prelude::*, *};
use moino_editor::{DedicatedProbe, PrototypeEditor};
use std::sync::Arc;

actions!(
    m0,
    [
        /// Shows or hides the prototype menu overlay.
        ToggleMenu,
        /// Dismisses the prototype menu overlay.
        DismissMenu,
        /// Quits the validation application.
        Quit
    ]
);

/// Runs the native validation application with Kit-owned root and native decorations.
pub fn run_prototype() {
    gpui_kit::application().run(|cx| {
        init(cx);
        gpui_kit::open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("墨 · M0 技术验证".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| PrototypeWorkspace::new(window, cx)),
        )
        .expect("native validation window");
        cx.activate(true);
    });
}

fn init(cx: &mut App) {
    gpui_kit::init(cx);
    moino_editor::init(cx);
    cx.bind_keys([
        KeyBinding::new("f10", ToggleMenu, None),
        KeyBinding::new("escape", DismissMenu, Some("MoinoWorkspace")),
        KeyBinding::new("escape", DismissMenu, Some("AppMenuBar")),
        KeyBinding::new(moino_platform::quit_shortcut(), Quit, None),
    ]);
    let menus = || vec![Menu::new("文件").items([MenuItem::action("退出", Quit)])];
    cx.set_menus(menus());
    gpui_kit::component::GlobalState::global_mut(cx)
        .set_app_menus(menus().into_iter().map(Menu::owned).collect());
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// Owns the independent M0 validation surfaces and asynchronous engine results.
pub struct PrototypeWorkspace {
    editor: Entity<PrototypeEditor>,
    dedicated: Entity<DedicatedProbe>,
    menu: Entity<AppMenuBar>,
    menu_visible: bool,
    menu_focus: FocusHandle,
    previous_focus: Option<FocusHandle>,
    math: Option<Arc<Image>>,
    diagram: Option<Arc<Image>>,
    preview: Option<Arc<Image>>,
    error: Option<SharedString>,
    _render: Task<()>,
}

impl PrototypeWorkspace {
    /// Creates input surfaces immediately and runs expensive rendering on the Kit executor.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(PrototypeEditor::new);
        let focus = editor.read(cx).focus_handle();
        window.focus(&focus, cx);
        let dedicated = cx.new(|cx| DedicatedProbe::new(window, cx));
        let menu = AppMenuBar::new(cx);
        let render = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async {
                    let math = moino_math::render_svg(r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}")
                        .map_err(|error| error.to_string())?;
                    let diagram = moino_diagram::render_svg(
                        "flowchart LR\nA[Markdown] --> B[Native layout] --> C[PDF]",
                    )
                    .map_err(|error| error.to_string())?;
                let pdf = moino_export::compile_prototype("#set page(width: 260pt, height: 160pt, margin: 20pt)\n= Moino\n中文 · 日本語 · 한국어\n\nPDF → hayro → GPUI")
                    .map_err(|error| error.to_string())?;
                let preview = moino_preview::render_pdf_page(pdf.bytes().to_vec(), 0)
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>((math, diagram, preview))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((math, diagram, preview)) => {
                        this.math = Some(Arc::new(Image::from_bytes(
                            ImageFormat::Svg,
                            math.into_bytes(),
                        )));
                        this.diagram = Some(Arc::new(Image::from_bytes(
                            ImageFormat::Svg,
                            diagram.into_bytes(),
                        )));
                        this.preview = Some(Arc::new(Image::from_bytes(ImageFormat::Png, preview)));
                    }
                    Err(error) => this.error = Some(error.into()),
                }
                cx.notify();
            }); // A closed validation window has no result owner.
        });
        Self {
            editor,
            dedicated,
            menu,
            menu_visible: false,
            menu_focus: cx.focus_handle(),
            previous_focus: None,
            math: None,
            diagram: None,
            preview: None,
            error: None,
            _render: render,
        }
    }
    fn show_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_visible || !moino_platform::has_window_menu() {
            return;
        }
        self.previous_focus = window.focused(cx);
        self.menu_visible = true;
        let focus = self.menu_focus.clone();
        let owner = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            if let Some(view) = owner.upgrade()
                && view.read(cx).menu_visible
            {
                window.focus(&focus, cx);
                window.focus_next(cx);
            }
        });
        cx.notify();
    }
    fn toggle_menu(&mut self, _: &ToggleMenu, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_visible {
            self.dismiss_menu(&DismissMenu, window, cx);
        } else {
            self.show_menu(window, cx);
        }
    }
    fn dismiss_menu(&mut self, _: &DismissMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_visible = false;
        let focus = self
            .previous_focus
            .take()
            .unwrap_or_else(|| self.editor.read(cx).focus_handle());
        window.focus(&focus, cx);
        cx.notify();
    }
}

impl Render for PrototypeWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("workspace")
            .key_context("MoinoWorkspace")
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::toggle_menu))
            .on_action(cx.listener(Self::dismiss_menu))
            .on_modifiers_changed(
                cx.listener(|this, event: &ModifiersChangedEvent, window, cx| {
                    if event.modifiers.alt && moino_platform::has_window_menu() {
                        this.show_menu(window, cx);
                    }
                }),
            )
            .child(
                div()
                    .id("content-viewport")
                    .test_support()
                    .size_full()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .p_6()
                            .child(div().text_lg().child("M0 技术验证"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "点击正文测试输入法；Alt / F10 打开菜单，Escape 返回正文",
                                    ),
                            )
                            .child(self.editor.clone())
                            .child(self.dedicated.clone())
                            .when_some(self.math.clone(), |this, image| {
                                this.child(
                                    img(image).w_full().h_24().object_fit(ObjectFit::Contain),
                                )
                            })
                            .when_some(self.diagram.clone(), |this, image| {
                                this.child(
                                    img(image).w_full().h_40().object_fit(ObjectFit::Contain),
                                )
                            })
                            .when_some(self.preview.clone(), |this, image| {
                                this.child(
                                    div()
                                        .id("pdf-preview")
                                        .role(Role::Image)
                                        .aria_label("PDF page 1")
                                        .child(
                                            img(image)
                                                .w_full()
                                                .h_48()
                                                .object_fit(ObjectFit::Contain),
                                        ),
                                )
                            })
                            .when_some(self.error.clone(), |this, error| {
                                this.child(div().id("render-error").role(Role::Alert).child(error))
                            }),
                    ),
            )
            .when(
                self.menu_visible && moino_platform::has_window_menu(),
                |this| {
                    this.child(
                        div()
                            .id("menu-overlay")
                            .test_support()
                            .track_focus(&self.menu_focus)
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h_8()
                            .bg(cx.theme().popover)
                            .child(self.menu.clone()),
                    )
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{PrototypeWorkspace, init};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};
    #[gpui_kit::test]
    // Spec: docs/acceptance/m0.md#技术原型
    fn menu_overlay_does_not_move_document(cx: &mut TestAppContext) {
        cx.update(init);
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| PrototypeWorkspace::new(window, cx))
            })
            .expect("window")
        });
        let before = cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                window.find("live-editor").bounds()
            })
            .expect("initial geometry");
        cx.update_window(handle, |_, window, cx| {
            window.press("f10", cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            assert_eq!(window.find("live-editor").bounds(), before);
            if moino_platform::has_window_menu() {
                assert_eq!(window.find("menu").focused(), Some(true));
                window.press("enter", cx);
                assert!(window.try_find("popup-menu").is_some());
                window.press("down", cx);
                window.press("escape", cx);
                assert!(window.try_find("menu-overlay").is_some());
            }
        })
        .expect("menu navigation");
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("popup-menu").is_none());
            assert_eq!(window.find("menu").focused(), Some(true));
            window.press("escape", cx);
            assert!(window.try_find("menu-overlay").is_none());
            assert_eq!(window.find("live-editor").bounds(), before);
        })
        .expect("update");
    }
}
