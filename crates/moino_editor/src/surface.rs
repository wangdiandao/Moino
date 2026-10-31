use crate::{PrototypeEditor, byte_from_utf16};
use gpui_kit::base::text::editor_host::{LeafGeometry, collect_leaf_geometry};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) struct HostSurface {
    child: Option<AnyElement>,
    editor: Option<Entity<PrototypeEditor>>,
    geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
}

impl HostSurface {
    pub(super) fn new(
        child: AnyElement,
        editor: Entity<PrototypeEditor>,
        geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
    ) -> Self {
        Self {
            child: Some(child),
            editor: Some(editor),
            geometry,
        }
    }
    pub(super) fn geometry_only(
        child: AnyElement,
        geometry: Rc<RefCell<HashMap<u64, LeafGeometry>>>,
    ) -> Self {
        Self {
            child: Some(child),
            editor: None,
            geometry,
        }
    }
}

impl IntoElement for HostSurface {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for HostSurface {
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
        let mut child = self
            .child
            .take()
            .expect("surface lays out exactly once per render");
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
        let (_, geometry) = collect_leaf_geometry(HashMap::new(), || child.prepaint(window, cx));
        *self.geometry.borrow_mut() = geometry;
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
        let Some(entity) = &self.editor else {
            return;
        };
        let editor = entity.read(cx);
        window.handle_input(
            &editor.focus,
            ElementInputHandler::new(bounds, entity.clone()),
            cx,
        );
        if editor.focus.is_focused(window)
            && let Some(geometry) = self.geometry.borrow().get(&1)
            && let Some(byte) = byte_from_utf16(&editor.text, editor.selection.end)
        {
            let position = geometry.position_for_index(byte);
            // A caret is a physical hairline, independent of interface density.
            window.paint_quad(fill(
                Bounds::new(position, size(px(1.), geometry.caret_height())),
                cx.theme().foreground,
            ));
        }
    }
}
