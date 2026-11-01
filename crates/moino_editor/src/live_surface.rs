use crate::live::LiveEditor;
use gpui_kit::base::text::editor_host::{LeafGeometry, collect_leaf_geometry};
use gpui_kit::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) struct LiveSurface {
    child: Option<AnyElement>,
    editor: Entity<LiveEditor>,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
}
impl LiveSurface {
    pub fn new(
        child: AnyElement,
        editor: Entity<LiveEditor>,
        geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
    ) -> Self {
        Self {
            child: Some(child),
            editor,
            geometry,
        }
    }
}
impl IntoElement for LiveSurface {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for LiveSurface {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let mut child = self.child.take().expect("surface lays out once per frame");
        (child.request_layout(window, cx), child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        let decorations = self.editor.read(cx).decorations(cx);
        let (_, geometry) = collect_leaf_geometry(decorations, || child.prepaint(window, cx));
        *self.geometry.borrow_mut() = geometry;
        let editor = self.editor.downgrade();
        window.on_next_frame(move |_, cx| {
            let _ = editor.update(cx, |editor, cx| editor.ensure_caret_visible(cx)); // Closed editor has no scroll target.
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
        self.editor.read(cx).paint_caret(window, cx);
        window.handle_input(
            &self.editor.read(cx).focus_handle(),
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
    }
}
