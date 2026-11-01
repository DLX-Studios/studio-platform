//! Runtime shell around the shared host-owned native renderer.
use crate::plugin_surface::{PluginSurface, SurfaceError};
use gpui_kit::{Context, Entity, IntoElement, Render, Window, prelude::*};
use studio_components::native_view::NativeRuntimeView;
pub use studio_components::native_view::{
    AnimationPolicy, FoundationFeature, FoundationGalleryModel, FoundationSnapshot,
};

/// Runtime window content and reload route ownership.
pub struct FoundationGallery {
    view: Entity<NativeRuntimeView<PluginSurface>>,
    last_reload_route: Option<crate::reload::ResolvedRoute>,
}
impl FoundationGallery {
    /// Create the native foundation gallery.
    #[must_use]
    pub fn new(reduced_motion: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            view: cx.new(|cx| NativeRuntimeView::new(reduced_motion, window, cx)),
            last_reload_route: None,
        }
    }
    /// Mount an admitted guest using the shared native renderer.
    #[must_use]
    pub fn with_plugin_surface(
        reduced_motion: bool,
        surface: PluginSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            view: cx.new(|cx| NativeRuntimeView::with_surface(reduced_motion, surface, window, cx)),
            last_reload_route: None,
        }
    }
    /// Drain completed host actions and invalidate the native view when needed.
    pub fn pump_host_actions(&mut self, cx: &mut Context<Self>) -> Result<bool, SurfaceError> {
        self.view.update(cx, |view, cx| {
            let changed = view.pump_host_actions()?;
            if changed {
                cx.notify();
            }
            Ok(changed)
        })
    }
    /// Install an admitted reload while retaining the window and native identities.
    pub fn apply_reload_swap(&mut self, swap: crate::reload::PreparedSwap, cx: &mut Context<Self>) {
        self.last_reload_route = Some(swap.route);
        self.view
            .update(cx, |view, cx| view.replace_surface(swap.surface, cx));
        cx.notify();
    }
    /// Route chosen by the most recent reload.
    #[must_use]
    pub fn last_reload_route(&self) -> Option<&crate::reload::ResolvedRoute> {
        self.last_reload_route.as_ref()
    }
}
impl Render for FoundationGallery {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.view.clone()
    }
}
