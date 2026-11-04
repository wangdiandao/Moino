//! One layout/geometry/input lifecycle for host-owned native editing surfaces.
use crate::{PrototypeEditor, byte_from_utf16};
use gpui_kit::base::text::editor_host::{LeafGeometry, collect_leaf_geometry};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{prelude::*, *};
use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

pub(super) type Geometry = Rc<RefCell<HashMap<u64, LeafGeometry>>>;
pub(super) type Decorations = HashMap<u64, Vec<(Range<usize>, Hsla)>>;

/// Owners supply editing policy; this seam only orders GPUI layout and paint.
pub(super) trait SurfaceOwner {
    fn decorations(&self, cx: &App) -> Decorations;
    fn after_layout(&self, _window: &mut Window, _cx: &mut App) {}
    fn paint(&self, bounds: Bounds<Pixels>, geometry: &Geometry, window: &mut Window, cx: &mut App);
}

pub(super) struct LayoutSurface<O> {
    child: Option<AnyElement>,
    owner: O,
    geometry: Geometry,
}

impl<O: SurfaceOwner> LayoutSurface<O> {
    pub(super) fn owned(child: AnyElement, owner: O, geometry: Geometry) -> Self {
        Self {
            child: Some(child),
            owner,
            geometry,
        }
    }
}

impl<O: SurfaceOwner + 'static> IntoElement for LayoutSurface<O> {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl<O: SurfaceOwner + 'static> Element for LayoutSurface<O> {
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
        // Replacement, rather than incremental insertion, retires unmounted leaves.
        let (_, geometry) =
            collect_leaf_geometry(self.owner.decorations(cx), || child.prepaint(window, cx));
        *self.geometry.borrow_mut() = geometry;
        self.owner.after_layout(window, cx);
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
        // The owner registers after children: nested renderers cannot steal its input.
        self.owner.paint(bounds, &self.geometry, window, cx);
    }
}

pub(super) struct PrototypeOwner(Option<Entity<PrototypeEditor>>);
pub(super) type HostSurface = LayoutSurface<PrototypeOwner>;

impl HostSurface {
    pub(super) fn new(
        child: AnyElement,
        editor: Entity<PrototypeEditor>,
        geometry: Geometry,
    ) -> Self {
        Self::owned(child, PrototypeOwner(Some(editor)), geometry)
    }
    pub(super) fn geometry_only(child: AnyElement, geometry: Geometry) -> Self {
        Self::owned(child, PrototypeOwner(None), geometry)
    }
}

impl SurfaceOwner for PrototypeOwner {
    fn decorations(&self, cx: &App) -> Decorations {
        let Some(entity) = &self.0 else {
            return HashMap::new();
        };
        let editor = entity.read(cx);
        let range = &editor.selection;
        if range.is_empty() {
            return HashMap::new();
        }
        match (
            byte_from_utf16(&editor.text, range.start),
            byte_from_utf16(&editor.text, range.end),
        ) {
            (Some(start), Some(end)) => {
                HashMap::from([(1, vec![(start..end, cx.theme().selection)])])
            }
            _ => HashMap::new(),
        }
    }
    fn paint(
        &self,
        bounds: Bounds<Pixels>,
        geometry: &Geometry,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(entity) = &self.0 else {
            return;
        };
        let editor = entity.read(cx);
        window.handle_input(
            &editor.focus,
            ElementInputHandler::new(bounds, entity.clone()),
            cx,
        );
        let head = if editor.selection_reversed {
            editor.selection.start
        } else {
            editor.selection.end
        };
        if editor.focus.is_focused(window)
            && let Some(geometry) = geometry.borrow().get(&1)
            && let Some(byte) = byte_from_utf16(&editor.text, head)
        {
            window.paint_quad(fill(
                Bounds::new(
                    geometry.position_for_index(byte),
                    size(px(1.), geometry.caret_height()),
                ),
                cx.theme().foreground,
            ));
        }
    }
}
