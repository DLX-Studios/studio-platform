//! Geometry-transparent authoring outline and pointer capture.
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AnyElement, App, BorderStyle, Bounds, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, Pixels,
    Window, outline,
};
use std::rc::Rc;

pub(super) struct SelectionElement {
    child: AnyElement,
    selected: bool,
    select: Rc<dyn Fn(&mut App)>,
}
impl SelectionElement {
    pub(super) fn new(
        child: AnyElement,
        selected: bool,
        select: impl Fn(&mut App) + 'static,
    ) -> Self {
        Self {
            child,
            selected,
            select: Rc::new(select),
        }
    }
}
impl IntoElement for SelectionElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for SelectionElement {
    type RequestLayoutState = ();
    type PrepaintState = (Bounds<Pixels>, Hitbox);
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
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        self.child.prepaint(window, cx);
        (bounds.intersect(&window.content_mask().bounds), hitbox)
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        if self.selected {
            window.paint_quad(outline(bounds, cx.theme().primary, BorderStyle::default()));
        }
        let select = self.select.clone();
        let visible_bounds = prepaint.0;
        let hitbox = prepaint.1.clone();
        // Child listeners are registered first, so the deepest authored node wins capture.
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if hitbox.is_hovered(window)
                && phase.capture()
                && event.button == MouseButton::Left
                && visible_bounds.contains(&event.position)
            {
                select(cx);
                cx.stop_propagation();
            }
        });
    }
}
