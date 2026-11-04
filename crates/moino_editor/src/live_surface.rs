//! Live editing policy plugged into the shared GPUI surface lifecycle.
use crate::live::LiveEditor;
use crate::surface::{Decorations, Geometry, LayoutSurface, SurfaceOwner};
use gpui_kit::*;

pub(super) struct LiveOwner(Entity<LiveEditor>);
pub(super) type LiveSurface = LayoutSurface<LiveOwner>;

impl LiveSurface {
    pub fn new(child: AnyElement, editor: Entity<LiveEditor>, geometry: Geometry) -> Self {
        Self::owned(child, LiveOwner(editor), geometry)
    }
}

impl SurfaceOwner for LiveOwner {
    fn decorations(&self, cx: &App) -> Decorations {
        self.0.read(cx).decorations(cx)
    }
    fn after_layout(&self, window: &mut Window, _: &mut App) {
        let editor = self.0.downgrade();
        window.on_next_frame(move |_, cx| {
            let _ = editor.update(cx, |editor, cx| {
                if !editor.restore_scroll_anchor(cx) {
                    editor.ensure_caret_visible(cx);
                }
            }); // A closed owner has no remaining scroll target.
        });
    }
    fn paint(&self, bounds: Bounds<Pixels>, _: &Geometry, window: &mut Window, cx: &mut App) {
        self.0.read(cx).paint_caret(window, cx);
        window.handle_input(
            &self.0.read(cx).focus_handle(),
            ElementInputHandler::new(bounds, self.0.clone()),
            cx,
        );
    }
}
