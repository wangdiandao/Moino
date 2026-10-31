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
        KeyBinding::new(moino_platform::quit_shortcut(), Quit, None),
    ]);
    cx.set_menus(vec![
        Menu::new("文件").items([MenuItem::action("退出", Quit)]),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// Owns the independent M0 validation surfaces and asynchronous engine results.
pub struct PrototypeWorkspace {
    editor: Entity<PrototypeEditor>,
    dedicated: Entity<DedicatedProbe>,
    menu: Entity<AppMenuBar>,
    menu_visible: bool,
    math: Option<Arc<Image>>,
    diagram: Option<Arc<Image>>,
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
                    Ok::<_, String>((math, diagram))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((math, diagram)) => {
                        this.math = Some(Arc::new(Image::from_bytes(
                            ImageFormat::Svg,
                            math.into_bytes(),
                        )));
                        this.diagram = Some(Arc::new(Image::from_bytes(
                            ImageFormat::Svg,
                            diagram.into_bytes(),
                        )));
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
            math: None,
            diagram: None,
            error: None,
            _render: render,
        }
    }
    fn toggle_menu(&mut self, _: &ToggleMenu, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_visible = !self.menu_visible;
        cx.notify();
    }
    fn dismiss_menu(&mut self, _: &DismissMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_visible = false;
        let focus = self.editor.read(cx).focus_handle();
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
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                if event.modifiers.alt && moino_platform::has_window_menu() {
                    this.menu_visible = true;
                    cx.notify();
                }
            }))
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
    fn menu_overlay_does_not_move_document(cx: &mut TestAppContext) {
        cx.update(init);
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| PrototypeWorkspace::new(window, cx))
            })
            .expect("window")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let before = window.find("live-editor").bounds();
            window.press("f10", cx);
            assert_eq!(window.find("live-editor").bounds(), before);
            window.press("escape", cx);
            assert!(window.try_find("menu-overlay").is_none());
            assert_eq!(window.find("live-editor").bounds(), before);
        })
        .expect("update");
    }
}
