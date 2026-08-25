//! Native controls used to prove the Wayland-only GPUI foundation.

use std::{collections::BTreeSet, path::Path, sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, Context, Entity, FocusHandle, Image, ImageFormat,
    IntoElement, KeyDownEvent, ParentElement, Render, Role, SharedString, Subscription, Window,
    div, img, prelude::*, px, rgb, text,
};
use gpui_component::{
    Disableable, IndexPath,
    badge::Badge,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    color_picker::{ColorPicker, ColorPickerState},
    date_picker::{DatePicker, DatePickerState},
    input::{Input, InputEvent, InputState},
    popover::Popover,
    progress::{Progress, ProgressCircle},
    radio::Radio,
    rating::Rating,
    select::{Select, SelectEvent, SelectState},
    separator::Separator,
    skeleton::Skeleton,
    slider::{Slider, SliderEvent, SliderState},
    spinner::Spinner,
    switch::Switch,
    tag::Tag,
};
use studio_actions::{Checkout, Money};
use studio_components::{InputAction, PropertyTransition, RuntimeControl};
use studio_navigation::MotionPreference;
use studio_protocol::NodeKind;

use crate::{
    NativeCheckoutShell,
    plugin_surface::{PluginRenderNode, PluginSurface},
};

const ACCESSIBILITY_LABELS: [&str; 3] = ["Increment counter", "Operator note", "Open details"];

// Studio's native palette. Components consume these tokens instead of inventing per-node
// colors; the protocol remains renderer-independent while the host owns visual styling.
const COLOR_BACKGROUND: u32 = 0x00f5_f7f8;
const COLOR_SURFACE: u32 = 0x00ff_ffff;
const COLOR_SURFACE_VARIANT: u32 = 0x00f8_f9fa;
const COLOR_CARD: u32 = 0x00ff_ffff;
const COLOR_BORDER: u32 = 0x00df_e3e6;
const COLOR_BORDER_SUBTLE: u32 = 0x00e9_edef;
const COLOR_TEXT: u32 = 0x0018_2735;
const COLOR_MUTED: u32 = 0x008b_949e;
const COLOR_SUCCESS: u32 = 0x00dc_fce7;
const COLOR_WARNING: u32 = 0x0085_3b00;
const COLOR_ERROR: u32 = 0x00fe_f2f2;

fn node_opacity(node: &PluginRenderNode) -> f32 {
    node.props
        .get("opacity")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0) as f32
}

fn node_accessibility_label(node: &PluginRenderNode) -> Option<String> {
    node.props
        .get("accessibility_label")
        .and_then(serde_json::Value::as_str)
        .filter(|label| !label.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransitionCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl TransitionCurve {
    fn sample(self, delta: f32) -> f32 {
        match self {
            Self::Linear => delta,
            Self::EaseIn => delta * delta,
            Self::EaseOut => 1.0 - (1.0 - delta).powi(2),
            Self::EaseInOut if delta < 0.5 => 2.0 * delta * delta,
            Self::EaseInOut => 1.0 - (-2.0 * delta + 2.0).powi(2) / 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodeTransition {
    duration: Duration,
    curve: TransitionCurve,
}

fn node_transition(node: &PluginRenderNode, reduced_motion: bool) -> Option<NodeTransition> {
    let transition = node.props.get("transition")?.as_object()?;
    let duration = Duration::from_millis(transition.get("duration_ms")?.as_u64()?);
    let preference = if reduced_motion {
        MotionPreference::Reduced
    } else {
        MotionPreference::Standard
    };
    let duration = PropertyTransition::resolve(duration, preference).duration();
    let curve = match transition.get("curve")?.as_str()? {
        "ease_in" => TransitionCurve::EaseIn,
        "ease_out" => TransitionCurve::EaseOut,
        "ease_in_out" => TransitionCurve::EaseInOut,
        _ => TransitionCurve::Linear,
    };
    Some(NodeTransition { duration, curve })
}

fn semantic_background(value: Option<&str>) -> gpui::Hsla {
    match value {
        Some("surface_variant") => rgb(COLOR_SURFACE_VARIANT).into(),
        Some("success") => rgb(COLOR_SUCCESS).into(),
        Some("warning") => rgb(COLOR_WARNING).into(),
        Some("error") => rgb(COLOR_ERROR).into(),
        Some("transparent") => gpui::transparent_black(),
        _ => rgb(COLOR_SURFACE).into(),
    }
}

fn image_format(path: &str, bytes: &[u8]) -> Option<ImageFormat> {
    if let Some(extension) = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        let format = match extension.to_ascii_lowercase().as_str() {
            "png" => ImageFormat::Png,
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "webp" => ImageFormat::Webp,
            "gif" => ImageFormat::Gif,
            "svg" => ImageFormat::Svg,
            "bmp" => ImageFormat::Bmp,
            "tif" | "tiff" => ImageFormat::Tiff,
            "ico" => ImageFormat::Ico,
            "pbm" | "pgm" | "ppm" | "pnm" => ImageFormat::Pnm,
            _ => return image_format_from_bytes(bytes),
        };
        return Some(format);
    }
    image_format_from_bytes(bytes)
}

fn image_format_from_bytes(bytes: &[u8]) -> Option<ImageFormat> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(ImageFormat::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else if bytes.starts_with(b"BM") {
        Some(ImageFormat::Bmp)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(ImageFormat::Webp)
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some(ImageFormat::Tiff)
    } else if bytes.starts_with(b"\0\0\x01\0") || bytes.starts_with(b"\0\0\x02\0") {
        Some(ImageFormat::Ico)
    } else if is_svg_bytes(bytes) {
        Some(ImageFormat::Svg)
    } else if matches!(
        bytes.get(0..2),
        Some(b"P1" | b"P2" | b"P3" | b"P4" | b"P5" | b"P6")
    ) {
        Some(ImageFormat::Pnm)
    } else {
        None
    }
}

fn is_svg_bytes(bytes: &[u8]) -> bool {
    let text = match std::str::from_utf8(&bytes[..bytes.len().min(256)]) {
        Ok(text) => text.trim_start(),
        Err(_) => return false,
    };
    text.starts_with("<svg") || text.starts_with("<?xml")
}

/// Native behaviors demonstrated by the foundation gallery.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FoundationFeature {
    /// Static accessible text.
    Text,
    /// An activatable native control.
    Button,
    /// A focusable editable text surface.
    TextInput,
    /// A bounded scrolling viewport.
    Scroll,
    /// A host-owned overlay surface.
    Popup,
    /// Ordered keyboard focus movement.
    FocusTraversal,
    /// Meaningful accessibility labels and roles.
    AccessibleLabels,
    /// A host-clock animation with a reduced-motion equivalent.
    Animation,
}

/// Host animation behavior chosen for the current motion preference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationPolicy {
    /// GPUI schedules frames from the host clock.
    HostScheduled,
    /// The same final state is rendered without movement.
    Static,
}

/// Read-only state used by deterministic tests and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundationSnapshot {
    /// Number of button activations.
    pub button_activations: u32,
    /// Current non-secret text input.
    pub text_input: String,
    /// Logical vertical scroll offset.
    pub scroll_offset: f32,
    /// Whether the host popup is visible.
    pub popup_open: bool,
    /// Accessible label of the logically focused control.
    pub focused_label: Option<&'static str>,
}

/// Deterministic state behind the native foundation gallery.
#[derive(Debug)]
pub struct FoundationGalleryModel {
    reduced_motion: bool,
    button_activations: u32,
    text_input: String,
    scroll_offset: f32,
    popup_open: bool,
    focused_index: usize,
}

impl FoundationGalleryModel {
    /// Creates a gallery model for the selected motion preference.
    #[must_use]
    pub fn new(reduced_motion: bool) -> Self {
        Self {
            reduced_motion,
            button_activations: 0,
            text_input: String::new(),
            scroll_offset: 0.0,
            popup_open: false,
            focused_index: 0,
        }
    }

    /// Returns the complete foundation capability inventory.
    #[must_use]
    pub fn features(&self) -> BTreeSet<FoundationFeature> {
        BTreeSet::from([
            FoundationFeature::AccessibleLabels,
            FoundationFeature::Animation,
            FoundationFeature::Button,
            FoundationFeature::FocusTraversal,
            FoundationFeature::Popup,
            FoundationFeature::Scroll,
            FoundationFeature::Text,
            FoundationFeature::TextInput,
        ])
    }

    /// Returns stable labels exposed by the gallery's interactive controls.
    #[must_use]
    pub const fn accessibility_labels(&self) -> [&'static str; 3] {
        ACCESSIBILITY_LABELS
    }

    /// Returns whether reduced motion is active.
    #[must_use]
    pub const fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Returns the host animation policy for this model.
    #[must_use]
    pub const fn animation_policy(&self) -> AnimationPolicy {
        if self.reduced_motion {
            AnimationPolicy::Static
        } else {
            AnimationPolicy::HostScheduled
        }
    }

    /// Activates the gallery button once.
    pub const fn activate_button(&mut self) {
        self.button_activations = self.button_activations.saturating_add(1);
    }

    /// Replaces the non-secret input contents.
    pub fn replace_text(&mut self, text: impl Into<String>) {
        self.text_input = text.into();
    }

    /// Appends text received from the native input event path.
    fn append_text(&mut self, text: &str) {
        self.text_input.push_str(text);
    }

    /// Adjusts the logical scroll offset without allowing a negative value.
    pub fn scroll_by(&mut self, delta: f32) {
        self.scroll_offset = (self.scroll_offset + delta).max(0.0);
    }

    /// Toggles the host-owned popup.
    pub const fn toggle_popup(&mut self) {
        self.popup_open = !self.popup_open;
    }

    /// Moves logical focus to the next interactive control.
    pub fn focus_next(&mut self) {
        self.focused_index = (self.focused_index + 1) % ACCESSIBILITY_LABELS.len();
    }

    /// Captures the current deterministic state.
    #[must_use]
    pub fn snapshot(&self) -> FoundationSnapshot {
        FoundationSnapshot {
            button_activations: self.button_activations,
            text_input: self.text_input.clone(),
            scroll_offset: self.scroll_offset,
            popup_open: self.popup_open,
            focused_label: ACCESSIBILITY_LABELS.get(self.focused_index).copied(),
        }
    }
}

/// Native GPUI surface that renders the foundation model.
pub struct FoundationGallery {
    model: FoundationGalleryModel,
    root_focus: FocusHandle,
    controls: [FocusHandle; 3],
    component_input: Entity<InputState>,
    component_secret_input: Entity<InputState>,
    component_select: Entity<SelectState<Vec<SharedString>>>,
    component_slider: Entity<SliderState>,
    component_date_picker: Entity<DatePickerState>,
    component_color_picker: Entity<ColorPickerState>,
    _component_subscriptions: Vec<Subscription>,
    plugin_surface: Option<PluginSurface>,
    checkout_shell: Option<NativeCheckoutShell>,
}

impl FoundationGallery {
    /// Creates the gallery and its ordered focus stops.
    #[must_use]
    pub fn new(reduced_motion: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let component_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search services, price or duration")
        });
        let component_secret_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter payment PIN")
                .masked(true)
        });
        let component_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    SharedString::from("All categories"),
                    SharedString::from("Hair"),
                    SharedString::from("Beard"),
                ],
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let component_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(0.5)
                .step(0.05)
                .default_value(0.0)
        });
        let component_date_picker = cx.new(|cx| DatePickerState::new(window, cx));
        let component_color_picker = cx.new(|cx| ColorPickerState::new(window, cx));

        let mut component_subscriptions = Vec::new();
        component_subscriptions.push(cx.subscribe_in(&component_input, window, {
            let component_input = component_input.clone();
            move |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = component_input.read(cx).value().to_string();
                    if let Some(surface) = this.plugin_surface.as_mut() {
                        let _ = surface.process_input("search", InputAction::TextChanged { value });
                    }
                    cx.notify();
                }
            }
        }));
        component_subscriptions.push(cx.subscribe_in(&component_select, window, {
            move |this, _, event: &SelectEvent<Vec<SharedString>>, _, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                if let Some(surface) = this.plugin_surface.as_mut() {
                    let _ = surface.process_input(
                        "category",
                        InputAction::SelectionChanged {
                            value: value.to_string(),
                        },
                    );
                }
                cx.notify();
            }
        }));
        component_subscriptions.push(cx.subscribe_in(&component_slider, window, {
            move |this, _, event: &SliderEvent, _, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                if let Some(surface) = this.plugin_surface.as_mut() {
                    let _ = surface.process_input(
                        "discount",
                        InputAction::SliderDrag {
                            value: f64::from(value.end()),
                        },
                    );
                }
                cx.notify();
            }
        }));

        Self {
            model: FoundationGalleryModel::new(reduced_motion),
            root_focus: cx.focus_handle(),
            controls: [
                cx.focus_handle().tab_index(1).tab_stop(true),
                cx.focus_handle().tab_index(2).tab_stop(true),
                cx.focus_handle().tab_index(3).tab_stop(true),
            ],
            component_input,
            component_secret_input,
            component_select,
            component_slider,
            component_date_picker,
            component_color_picker,
            _component_subscriptions: component_subscriptions,
            plugin_surface: None,
            checkout_shell: None,
        }
    }

    /// Creates the native shell while retaining a fully prepared plugin surface for its lifetime.
    ///
    /// # Panics
    ///
    /// Panics if the internal money value is invalid (hard-coded valid amount).
    #[must_use]
    pub fn with_plugin_surface(
        reduced_motion: bool,
        plugin_surface: PluginSurface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut gallery = Self::new(reduced_motion, window, cx);
        let reduced = gallery.model.reduced_motion();
        let owner_id = plugin_surface.owner_id().as_str().to_owned();
        gallery.plugin_surface = Some(plugin_surface);
        let checkout = Checkout::new(
            format!("{owner_id}-sale"),
            "Studio Barber",
            "Verified POS",
            Money::new("USD", 5_724).unwrap(),
        )
        .ok();
        if let Some(checkout) = checkout
            && let Some(surface) = gallery.plugin_surface.as_ref()
            && let Ok(shell) = surface.checkout_shell(checkout, reduced)
        {
            gallery.checkout_shell = Some(shell);
        }
        gallery
    }

    /// Current host-owned route (visible navigation state).
    #[must_use]
    pub fn checkout_route(&self) -> Option<&str> {
        self.checkout_shell
            .as_ref()
            .map(NativeCheckoutShell::current_route)
    }

    /// Whether a host-owned checkout shell is active.
    #[must_use]
    pub fn has_checkout_shell(&self) -> bool {
        self.checkout_shell.is_some()
    }

    /// Mutable access to the host-owned checkout shell for trusted flows.
    pub fn checkout_shell_mut(&mut self) -> Option<&mut NativeCheckoutShell> {
        self.checkout_shell.as_mut()
    }

    fn animation_indicator(&self) -> AnyElement {
        let indicator = div().size(px(18.0)).rounded_full().bg(rgb(COLOR_SUCCESS));
        if self.model.reduced_motion() {
            indicator.into_any_element()
        } else {
            indicator
                .with_animation(
                    "foundation-pulse",
                    Animation::new(Duration::from_millis(800)).repeat(),
                    |element, delta| element.opacity(0.35 + 0.65 * delta),
                )
                .into_any_element()
        }
    }

    fn increment_button(activations: u32, cx: &mut Context<Self>) -> AnyElement {
        Button::new("foundation-button")
            .primary()
            .label(format!("Increment ({activations})"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.model.activate_button();
                cx.notify();
            }))
            .into_any_element()
    }

    fn operator_input(&self, value: &str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("foundation-input")
            .focusable()
            .tab_stop(true)
            .track_focus(&self.controls[1])
            .role(Role::TextInput)
            .aria_label(ACCESSIBILITY_LABELS[1])
            .aria_value(value.to_owned())
            .min_h(px(40.0))
            .px_3()
            .py_2()
            .border_1()
            .border_color(rgb(COLOR_BORDER))
            .rounded_md()
            .bg(rgb(COLOR_SURFACE))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if let Some(text) = event.keystroke.key_char.as_deref() {
                    this.model.append_text(text);
                    cx.notify();
                }
            }))
            .child(if value.is_empty() {
                "Type an operator note".to_owned()
            } else {
                value.to_owned()
            })
            .into_any_element()
    }

    fn popup_button(expanded: bool, cx: &mut Context<Self>) -> AnyElement {
        Button::new("foundation-popup-button")
            .secondary()
            .label(if expanded {
                "Close details"
            } else {
                "Open details"
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.model.toggle_popup();
                cx.notify();
            }))
            .into_any_element()
    }

    fn scrollable_items() -> AnyElement {
        div()
            .id("foundation-scroll")
            .role(Role::List)
            .aria_label("Scrollable native items")
            .max_h(px(96.0))
            .overflow_y_scroll()
            .children((1_usize..=12).map(|index| {
                div()
                    .id(("foundation-scroll-item", index))
                    .role(Role::ListItem)
                    .py_1()
                    .child(text!(format!("Native item {index}")))
            }))
            .into_any_element()
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::too_many_lines,
        clippy::only_used_in_recursion,
        reason = "closed component dispatch keeps the native mapping auditable in one place"
    )]
    fn plugin_node(
        &mut self,
        node: PluginRenderNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_root = node.id == "root";
        let is_order_pane = node.id == "order-pane";
        let is_order_content = node.id == "order-content";
        let is_order_summary = node.id == "order-summary";
        let is_catalog_pane = node.id == "catalog-pane";
        let is_product_image = node.id.ends_with("-img");
        let is_cart_image = node.id.ends_with("-cart-img");
        let is_product_meta = node.id.ends_with("-meta");
        let is_summary_row = matches!(
            node.id.as_str(),
            "order-head" | "subtotal-row" | "taxes-row" | "discount-row" | "total-row"
        );
        if node
            .props
            .get("visible")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
        {
            return div().id(node.id).hidden().into_any_element();
        }
        let opacity = node_opacity(&node);
        let accessibility_label = node_accessibility_label(&node);
        let transition = node_transition(&node, self.model.reduced_motion());
        let transition_id = format!("{}:transition", node.id);
        let gap = node
            .props
            .get("gap")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        let flex = node
            .props
            .get("flex")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0) as f32;
        let children = node
            .children
            .into_iter()
            .map(|child| self.plugin_node(child, window, cx))
            .collect::<Vec<_>>();
        let rendered = match node.kind {
            NodeKind::Column => {
                let alignment = node
                    .props
                    .get("alignment")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("stretch");
                div()
                .id(node.id)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, label| {
                    element.aria_label(label)
                })
                .w_full()
                .min_h_0()
                .flex_grow(flex)
                .when(is_order_content, |element| {
                    element.h_full().min_h_0().justify_between()
                })
                .flex()
                .flex_col()
                .when(alignment == "start", gpui::Styled::items_start)
                .when(alignment == "center", gpui::Styled::items_center)
                .when(alignment == "end", gpui::Styled::items_end)
                .when(alignment == "stretch", gpui::Styled::items_stretch)
                .when(alignment == "space_between", gpui::Styled::justify_between)
                .gap(px(gap))
                .children(children)
                .into_any_element()
            }
            NodeKind::Row => {
                let is_main_row = node.id == "main-row";
                let alignment = node
                    .props
                    .get("alignment")
                    .and_then(serde_json::Value::as_str);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .w_full()
                    .when(is_root, gpui::Styled::h_full)
                    .when(is_root, |element| element.min_h_0().flex_grow_1())
                    .when(is_main_row, |el| el.h_full().min_h_0().items_stretch())
                    .when(is_product_meta, gpui::Styled::justify_between)
                    .when(is_summary_row, gpui::Styled::justify_between)
                    .flex()
                    .when(
                        alignment.is_none() && !is_root && !is_main_row,
                        gpui::Styled::items_center,
                    )
                    .when(alignment == Some("start"), gpui::Styled::items_start)
                    .when(alignment == Some("center"), gpui::Styled::items_center)
                    .when(alignment == Some("end"), gpui::Styled::items_end)
                    .when(alignment == Some("stretch"), gpui::Styled::items_stretch)
                    .when(
                        alignment == Some("space_between"),
                        gpui::Styled::justify_between,
                    )
                    .gap(px(gap))
                    .children(children)
                    .into_any_element()
            }
            NodeKind::ListView => {
                let horizontal = node.props.get("axis").and_then(serde_json::Value::as_str)
                    == Some("horizontal");
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .flex()
                    .when(horizontal, gpui::Styled::flex_row)
                    .when(!horizontal, gpui::Styled::flex_col)
                    .min_h_0()
                    .min_w_0()
                    .flex_grow_1()
                    .flex_shrink_1()
                    .gap(px(gap))
                    .when(horizontal, gpui::Styled::overflow_x_scroll)
                    .when(!horizontal, gpui::Styled::overflow_y_scroll)
                    .children(children)
                    .into_any_element()
            }
            NodeKind::ScrollView => {
                let horizontal = node.props.get("axis").and_then(serde_json::Value::as_str)
                    == Some("horizontal");
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .w_full()
                    .min_h_0()
                    .min_w_0()
                    .flex_grow_1()
                    .when(horizontal, gpui::Styled::overflow_x_scroll)
                    .when(!horizontal, gpui::Styled::overflow_y_scroll)
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Grid => {
                let columns = node
                    .props
                    .get("columns")
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|columns| u16::try_from(columns).ok())
                    .unwrap_or(2);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .w_full()
                    .min_w_0()
                    .grid()
                    .grid_cols(columns)
                    .gap(px(gap))
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Stack => {
                let alignment = node
                    .props
                    .get("alignment")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("stretch");
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .relative()
                    .min_w_0()
                    .flex()
                    .when(alignment == "start", |element| {
                        element.items_start().justify_start()
                    })
                    .when(alignment == "center", |element| {
                        element.items_center().justify_center()
                    })
                    .when(alignment == "end", |element| {
                        element.items_end().justify_end()
                    })
                    .when(alignment == "stretch", gpui::Styled::items_stretch)
                    .when(alignment == "space_between", gpui::Styled::justify_between)
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Card => {
                let padding = node
                    .props
                    .get("padding")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(12.0) as f32;
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w(px(150.0))
                    .min_h(px(238.0))
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .overflow_hidden()
                    .p(px(padding))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER))
                    .bg(rgb(COLOR_CARD))
                    .shadow_sm()
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Box => {
                let padding = node
                    .props
                    .get("padding")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let background = match node
                    .props
                    .get("background")
                    .and_then(serde_json::Value::as_str)
                {
                    value => semantic_background(value),
                };
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .when(is_order_pane, |element| {
                        element.w(px(390.0)).h_full().flex_shrink_0()
                    })
                    .when(is_catalog_pane, |element| {
                        element.h_full().min_w_0().flex_grow(1.0).flex_shrink(1.0)
                    })
                    .when(is_catalog_pane, |element| element.flex_grow(1.0))
                    .when(is_order_pane, |element| element.flex_grow(0.0))
                    .when(!is_catalog_pane && !is_order_pane, |el| el.flex_grow(flex))
                    .flex()
                    .flex_col()
                    .when(is_order_summary, gpui::Styled::flex_shrink_0)
                    .p(px(padding))
                    .bg(background)
                    .when(is_order_summary, |element| {
                        element
                            .rounded_xl()
                            .border_1()
                            .border_color(rgb(COLOR_BORDER_SUBTLE))
                    })
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Spacer => {
                let size = node
                    .props
                    .get("size")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(8.0) as f32;
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .w(px(size))
                    .h(px(size))
                    .flex_shrink_0()
                    .into_any_element()
            }
            NodeKind::Divider => {
                let thickness = node
                    .props
                    .get("thickness")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1.0) as f32;
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .w_full()
                    .h(px(thickness))
                    .bg(rgb(COLOR_BORDER))
                    .into_any_element()
            }
            NodeKind::Icon => {
                let name = node
                    .props
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                div()
                    .id(node.id)
                    .role(Role::Image)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .size(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .bg(rgb(COLOR_SURFACE_VARIANT))
                    .text_color(rgb(COLOR_MUTED))
                    .text_xs()
                    .child(name)
                    .into_any_element()
            }
            NodeKind::Tag => {
                let label = node
                    .props
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let variant = node
                    .props
                    .get("variant")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("default");
                let tag = match variant {
                    "success" => Tag::success().child(label.to_owned()).into_any_element(),
                    "warning" => Tag::warning().child(label.to_owned()).into_any_element(),
                    "destructive" => Tag::danger().child(label.to_owned()).into_any_element(),
                    _ => Tag::secondary().child(label.to_owned()).into_any_element(),
                };
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .child(tag)
                    .into_any_element()
            }
            NodeKind::Badge => {
                let label = node
                    .props
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                // Numeric labels keep count badge semantics, text labels render as pill
                let badge = if let Ok(count) = label.parse::<usize>() {
                    Badge::new().count(count).into_any_element()
                } else if label.is_empty() {
                    Badge::new().count(0).into_any_element()
                } else {
                    div()
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded_full()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .bg(rgb(COLOR_SUCCESS))
                        .text_color(rgb(COLOR_TEXT))
                        .child(label)
                        .into_any_element()
                };
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .child(badge)
                    .into_any_element()
            }
            NodeKind::Skeleton => {
                let width = node
                    .props
                    .get("width")
                    .and_then(serde_json::Value::as_f64)
                    .map(|value| value as f32);
                let height = node
                    .props
                    .get("height")
                    .and_then(serde_json::Value::as_f64)
                    .map(|value| value as f32);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .child(
                        Skeleton::new()
                            .when_some(width, |element, width| element.w(px(width)))
                            .when_some(height, |element, height| element.h(px(height))),
                    )
                    .into_any_element()
            }
            NodeKind::Spinner => div()
                .id(node.id)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, label| {
                    element.aria_label(label)
                })
                .child(Spinner::new())
                .into_any_element(),
            NodeKind::Separator => div()
                .id(node.id)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, label| {
                    element.aria_label(label)
                })
                .child(Separator::horizontal())
                .into_any_element(),
            NodeKind::Image => {
                let path = node
                    .props
                    .get("asset")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let source = self
                    .plugin_surface
                    .as_ref()
                    .and_then(|surface| surface.asset(path))
                    .and_then(|bytes| {
                        image_format(path, bytes)
                            .map(|format| Arc::new(Image::from_bytes(format, bytes.to_vec())))
                    });
                match source {
                    Some(source) => img(source)
                        .id(node.id)
                        .role(Role::Image)
                        .opacity(opacity)
                        .when_some(
                            accessibility_label.clone().or_else(|| {
                                node.props
                                    .get("alt")
                                    .and_then(serde_json::Value::as_str)
                                    .map(ToOwned::to_owned)
                            }),
                            |element, label| element.aria_label(label),
                        )
                        .w_full()
                        .when(is_product_image || !is_cart_image, |element| {
                            element.h(px(128.0))
                        })
                        .when(is_cart_image, |element| {
                            element.w(px(72.0)).h(px(72.0)).flex_shrink_0()
                        })
                        .object_fit(gpui::ObjectFit::Cover)
                        .rounded_md()
                        .into_any_element(),
                    None => div()
                        .id(node.id)
                        .role(Role::Image)
                        .opacity(opacity)
                        .when_some(
                            accessibility_label.clone().or_else(|| {
                                node.props
                                    .get("alt")
                                    .and_then(serde_json::Value::as_str)
                                    .map(ToOwned::to_owned)
                            }),
                            |element, label| element.aria_label(label),
                        )
                        .w_full()
                        .h(px(128.0))
                        .rounded_md()
                        .bg(rgb(COLOR_SURFACE_VARIANT))
                        .into_any_element(),
                }
            }
            NodeKind::Rating => {
                let value = node
                    .props
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .or_else(|| {
                        node.props
                            .get("value")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|s| s.parse::<f64>().ok())
                    })
                    .unwrap_or(0.0);
                // gpui Rating expects usize 0..5, map f64 0-5 to 0-5
                let int_value = (value.clamp(0.0, 5.0).round() as usize).min(5);
                let rating_id = format!("{}:rating", node.id);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .child(Rating::new(rating_id).value(int_value))
                    .into_any_element()
            }
            NodeKind::ProgressIndicator => {
                let value = node
                    .props
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let progress_id = format!("{}:progress", node.id);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .child(Progress::new(progress_id).value(value * 100.0))
                    .into_any_element()
            }
            NodeKind::ProgressCircle => {
                let value = node
                    .props
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0) as f32;
                let progress_id = format!("{}:progress-circle", node.id);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .child(ProgressCircle::new(progress_id).value(value * 100.0))
                    .into_any_element()
            }
            NodeKind::Popover => {
                let open = node
                    .props
                    .get("open")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                Popover::new(node.id)
                    .default_open(open)
                    .trigger(Button::new("popover-trigger").secondary().label("Open"))
                    .content(|_, _, _| div().p_3().child("Popover content"))
                    .into_any_element()
            }
            NodeKind::Avatar => {
                let fallback = node
                    .props
                    .get("fallback")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let asset = node
                    .props
                    .get("asset")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let source = self
                    .plugin_surface
                    .as_ref()
                    .and_then(|surface| surface.asset(asset))
                    .and_then(|bytes| {
                        image_format(asset, bytes)
                            .map(|format| Arc::new(Image::from_bytes(format, bytes.to_vec())))
                    });
                let alt = accessibility_label.clone().or_else(|| {
                    node.props
                        .get("alt")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                });
                let content = match source {
                    Some(source) => img(source)
                        .size(px(40.0))
                        .object_fit(gpui::ObjectFit::Cover)
                        .rounded_full()
                        .into_any_element(),
                    None => div()
                        .size(px(40.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(COLOR_SURFACE_VARIANT))
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(fallback)
                        .into_any_element(),
                };
                div()
                    .id(node.id)
                    .role(Role::Image)
                    .opacity(opacity)
                    .when_some(alt, |element, label| element.aria_label(label))
                    .child(content)
                    .into_any_element()
            }
            NodeKind::Empty => {
                let title = node
                    .props
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Empty")
                    .to_owned();
                let description = node
                    .props
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .w_full()
                    .p_4()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER_SUBTLE))
                    .bg(rgb(COLOR_SURFACE_VARIANT))
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
                    .when(!description.is_empty(), |element| {
                        element.child(
                            div()
                                .text_sm()
                                .text_color(rgb(COLOR_MUTED))
                                .child(description),
                        )
                    })
                    .into_any_element()
            }
            NodeKind::Kbd
            | NodeKind::Alert
            | NodeKind::Attachment
            | NodeKind::Command
            | NodeKind::NativeSelect
            | NodeKind::Message
            | NodeKind::Sonner => {
                let label = node
                    .props
                    .get("label")
                    .or_else(|| node.props.get("title"))
                    .or_else(|| node.props.get("content"))
                    .or_else(|| node.props.get("message"))
                    .or_else(|| node.props.get("value"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                div()
                    .id(node.id)
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER_SUBTLE))
                    .bg(rgb(COLOR_SURFACE_VARIANT))
                    .when(!label.is_empty(), |element| element.child(label))
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Sidebar => div()
                .id(node.id)
                .w(px(220.0))
                .h_full()
                .min_h_0()
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .border_r_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .children(children)
                .into_any_element(),
            NodeKind::MenuBar => div()
                .id(node.id)
                .w_full()
                .h(px(32.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_4()
                .px_3()
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .border_b_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .children(children)
                .into_any_element(),
            NodeKind::AppBar => div()
                .id(node.id)
                .w_full()
                .h(px(56.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_4()
                .bg(rgb(COLOR_SURFACE))
                .border_b_1()
                .border_color(rgb(COLOR_BORDER))
                .children(children)
                .into_any_element(),
            NodeKind::Scaffold => div()
                .id(node.id)
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .bg(rgb(COLOR_BACKGROUND))
                .children(children)
                .into_any_element(),
            NodeKind::Tabs => div()
                .id(node.id)
                .w_full()
                .flex()
                .flex_row()
                .gap_2()
                .items_center()
                .p_1()
                .rounded_lg()
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .children(children)
                .into_any_element(),
            NodeKind::Breadcrumb => div()
                .id(node.id)
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(rgb(COLOR_MUTED))
                .children(children)
                .into_any_element(),
            NodeKind::StatusBar => div()
                .id(node.id)
                .w_full()
                .h(px(24.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .text_xs()
                .text_color(rgb(COLOR_MUTED))
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .border_t_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .children(children)
                .into_any_element(),
            NodeKind::NavigationBar | NodeKind::NavigationRail => div()
                .id(node.id)
                .flex()
                .items_center()
                .gap_2()
                .p_2()
                .rounded_lg()
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .children(children)
                .into_any_element(),
            NodeKind::Accordion
            | NodeKind::Collapsible
            | NodeKind::HoverCard
            | NodeKind::KeyboardShortcuts
            | NodeKind::Resizable
            | NodeKind::Dock
            | NodeKind::Chart
            | NodeKind::Editor
            | NodeKind::RichText
            | NodeKind::Carousel
            | NodeKind::DragDrop
            | NodeKind::Theme
            | NodeKind::AspectRatio
            | NodeKind::Bubble
            | NodeKind::NavigationMenu
            | NodeKind::ScrollArea
            | NodeKind::Item
            | NodeKind::MessageScroller
            | NodeKind::ToggleGroup
            | NodeKind::Stepper
            | NodeKind::Pagination
            | NodeKind::ListTile
            | NodeKind::SearchableList
            | NodeKind::VirtualList
            | NodeKind::DataTable
            | NodeKind::Tree
            | NodeKind::DescriptionList
            | NodeKind::TimePicker => div()
                .id(node.id)
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .p_2()
                .rounded_lg()
                .border_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .bg(rgb(COLOR_SURFACE))
                .children(children)
                .into_any_element(),
            NodeKind::Dialog => {
                let open = node
                    .props
                    .get("open")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if !open {
                    return div().id(node.id).hidden().into_any_element();
                }
                let title = node
                    .props
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Dialog")
                    .to_owned();
                div()
                    .id(node.id)
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
                    .child(
                        div()
                            .w(px(480.0))
                            .max_w(px(520.0))
                            .p_6()
                            .rounded_xl()
                            .bg(rgb(COLOR_SURFACE))
                            .border_1()
                            .border_color(rgb(COLOR_BORDER))
                            .shadow_lg()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .mb_4()
                                    .child(title),
                            )
                            .children(children),
                    )
                    .into_any_element()
            }
            NodeKind::AlertDialog
            | NodeKind::Sheet
            | NodeKind::BottomSheet
            | NodeKind::Drawer
            | NodeKind::Toast
            | NodeKind::Notification
            | NodeKind::Banner
            | NodeKind::ContextMenu
            | NodeKind::CommandPalette
            | NodeKind::Tooltip => div()
                .id(node.id)
                .w_full()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(rgb(COLOR_BORDER))
                .bg(rgb(COLOR_CARD))
                .children(children)
                .into_any_element(),
            NodeKind::ButtonGroup
            | NodeKind::RangeSlider
            | NodeKind::Combobox
            | NodeKind::NumberInput
            | NodeKind::TextArea
            | NodeKind::Field
            | NodeKind::InputGroup
            | NodeKind::OtpInput => div()
                .id(node.id)
                .w_full()
                .min_w_0()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .children(children)
                .into_any_element(),
            NodeKind::Text => {
                let role = node
                    .props
                    .get("typography_role")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("body");
                let raw = node
                    .props
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                // Host displays dot while wasm emits comma to stay under 10M fuel
                let value = if raw.contains('$') {
                    raw.replace(',', ".")
                } else {
                    raw
                };
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .overflow_hidden()
                    .when(role == "caption", |element| {
                        element.text_sm().text_color(rgb(COLOR_MUTED))
                    })
                    .when(role == "title", |element| {
                        element.text_xl().text_color(rgb(COLOR_MUTED))
                    })
                    .when(role == "label", |el| {
                        el.text_base().font_weight(gpui::FontWeight::MEDIUM)
                    })
                    .when(role == "headline", |el| {
                        el.text_lg().font_weight(gpui::FontWeight::SEMIBOLD)
                    })
                    .when(role == "display", |el| {
                        el.text_2xl().font_weight(gpui::FontWeight::BOLD)
                    })
                    .child(value)
                    .into_any_element()
            }
            NodeKind::Button if node.control == Some(RuntimeControl::Button) => {
                let node_id = node.id;
                let click_id = node_id.clone();
                let label = node
                    .props
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Button")
                    .to_owned();
                let variant = node
                    .props
                    .get("variant")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("primary");
                let enabled = node
                    .props
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true);
                let is_card_action = node_id.starts_with("add-");
                let button = Button::new(node_id)
                    .label(label)
                    .disabled(!enabled)
                    .when(is_card_action, gpui::Styled::w_full);
                let button = match variant {
                    "secondary" => button.secondary(),
                    _ => button.primary(),
                };
                button
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(surface) = this.plugin_surface.as_mut() {
                            let _ = surface.process_input(&click_id, InputAction::PointerClick);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            }
            NodeKind::TextInput if node.control == Some(RuntimeControl::Input) => div()
                .id(node.id)
                .flex_grow_1()
                .child(Input::new(&self.component_input))
                .into_any_element(),
            NodeKind::SecretInput if node.control == Some(RuntimeControl::Input) => {
                let label = node
                    .props
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Trusted input")
                    .to_owned();
                div()
                    .id(node.id)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().text_color(rgb(COLOR_MUTED)).child(label))
                    .child(Input::new(&self.component_secret_input))
                    .into_any_element()
            }
            NodeKind::Slider if node.control == Some(RuntimeControl::Slider) => {
                let value = node
                    .props
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                div()
                    .id(node.id)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(format!("Discount: {:.0}%", value * 100.0))
                    .child(Slider::new(&self.component_slider).horizontal())
                    .into_any_element()
            }
            NodeKind::Select if node.control == Some(RuntimeControl::Select) => {
                let value = node
                    .props
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Select")
                    .to_owned();
                div()
                    .id(node.id)
                    .w(px(190.0))
                    .child(Select::new(&self.component_select).placeholder(value))
                    .into_any_element()
            }
            NodeKind::Checkbox => Checkbox::new(node.id)
                .label(
                    node.props
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                )
                .checked(
                    node.props
                        .get("value")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                )
                .into_any_element(),
            NodeKind::Radio => Radio::new(node.id)
                .label(
                    node.props
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                )
                .checked(
                    node.props
                        .get("value")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                )
                .into_any_element(),
            NodeKind::Switch | NodeKind::Toggle => Switch::new(node.id)
                .label(
                    node.props
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                )
                .checked(
                    node.props
                        .get("value")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                )
                .into_any_element(),
            NodeKind::DatePicker | NodeKind::Calendar => {
                DatePicker::new(&self.component_date_picker).into_any_element()
            }
            NodeKind::ColorPicker => {
                ColorPicker::new(&self.component_color_picker).into_any_element()
            }
            _ => div().id(node.id).children(children).into_any_element(),
        };
        match transition {
            Some(transition) if !transition.duration.is_zero() => {
                // UNVERIFIED: the serialized runner must confirm the GPUI animation wrapper's
                // retained identity and layout behavior across targeted property patches.
                div()
                    .id(transition_id.clone())
                    .min_w_0()
                    .child(rendered)
                    .with_animation(
                        transition_id,
                        Animation::new(transition.duration)
                            .with_easing(move |delta| transition.curve.sample(delta)),
                        |element, delta| element.opacity(delta),
                    )
                    .into_any_element()
            }
            _ => rendered,
        }
    }
}

impl Render for FoundationGallery {
    #[allow(
        clippy::too_many_lines,
        clippy::used_underscore_binding,
        reason = "foundation fallback and mounted plugin shell remain visually co-located"
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(root) = self
            .plugin_surface
            .as_ref()
            .and_then(PluginSurface::render_tree)
        {
            let warning = self
                .plugin_surface
                .as_ref()
                .and_then(PluginSurface::warning)
                .unwrap_or_default()
                .to_owned();
            let checkout_route = self
                .checkout_shell
                .as_ref()
                .map(|shell| shell.current_route().to_owned());
            let plugin = self.plugin_node(root, window, cx);
            let route_bar = checkout_route.clone().map(|route| {
                div()
                    .id("checkout-route")
                    .role(Role::Navigation)
                    .aria_label(format!("Current route {route}"))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(COLOR_SURFACE_VARIANT))
                    .border_1()
                    .border_color(rgb(COLOR_BORDER_SUBTLE))
                    .flex()
                    .justify_between()
                    .child(div().child(format!("Route: {route}")))
                    .child(div().text_xs().text_color(rgb(COLOR_MUTED)).child(
                        if route.starts_with("/receipts/") {
                            "Receipt — host-owned immutable record"
                        } else if route == "/checkout/payment" {
                            "Checkout — trusted confirmation required"
                        } else if route == "/cart" {
                            "Cart — host-owned route state"
                        } else {
                            "Host-owned navigation"
                        },
                    ))
                    .into_any_element()
            });
            let confirmation_overlay = checkout_route
                .as_deref()
                .filter(|route| *route == "/checkout/payment")
                .map(|_| {
                    div()
                        .id("trusted-confirmation-overlay")
                        .role(Role::Dialog)
                        .aria_label("Trusted Studio confirmation")
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.35))
                        .child(
                            div()
                                .w(px(420.0))
                                .p_6()
                                .rounded_xl()
                                .bg(rgb(COLOR_SURFACE))
                                .border_1()
                                .border_color(rgb(COLOR_BORDER))
                                .shadow_lg()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child("Studio — trusted confirmation"),
                                )
                                .child(
                                    div()
                                        .mt_2()
                                        .text_sm()
                                        .text_color(rgb(COLOR_MUTED))
                                        .child("Verified merchant, exact amount, and offline simulator status are host-owned."),
                                ),
                        )
                        .into_any_element()
                });
            return div()
                .id("plugin-gallery")
                .size_full()
                .flex()
                .flex_col()
                .gap_2()
                .min_h_0()
                .p_2()
                .bg(rgb(COLOR_BACKGROUND))
                .text_color(rgb(COLOR_TEXT))
                .when(!warning.is_empty(), |element| {
                    element.child(
                        div()
                            .id("plugin-development-warning")
                            .role(Role::Alert)
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(COLOR_WARNING))
                            .child(warning),
                    )
                })
                .children(route_bar)
                .child(plugin)
                .children(confirmation_overlay);
        }
        let snapshot = self.model.snapshot();
        let button = Self::increment_button(snapshot.button_activations, cx);
        let input = self.operator_input(&snapshot.text_input, cx);
        let popup_button = Self::popup_button(snapshot.popup_open, cx);
        let development_warning = self
            .plugin_surface
            .as_ref()
            .and_then(PluginSurface::warning)
            .map(|warning| {
                div()
                    .id("development-warning")
                    .role(Role::Alert)
                    .aria_label(warning)
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x0085_3b00))
                    .child(warning)
            });

        div()
            .id("foundation-gallery")
            .role(Role::Application)
            .aria_label("Studio native foundation gallery")
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                        this.model.focus_next();
                    }
                    cx.notify();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .bg(rgb(0x0010_1418))
            .text_color(rgb(0x00f3_f6f8))
            .children(development_warning)
            .child(
                div()
                    .id("foundation-heading")
                    .role(Role::Heading)
                    .aria_level(1)
                    .text_2xl()
                    .child(text!("Studio Runtime")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.animation_indicator())
                    .child(if self.model.reduced_motion() {
                        "Reduced motion: static state"
                    } else {
                        "Host-scheduled animation"
                    }),
            )
            .child(button)
            .child(input)
            .child(popup_button)
            .child(Self::scrollable_items())
            .when(snapshot.popup_open, |root| {
                root.child(
                    div()
                        .id("foundation-popup")
                        .role(Role::Dialog)
                        .aria_label("Foundation details")
                        .absolute()
                        .right_6()
                        .top_6()
                        .p_4()
                        .rounded_md()
                        .bg(rgb(0x0024_2c33))
                        .child("Host-owned popup surface"),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{ImageFormat, image_format};

    #[test]
    fn detects_common_image_formats_from_extension_or_bytes() {
        assert_eq!(
            image_format("assets/photo.png", b"\x89PNG\r\n\x1a\nrest"),
            Some(ImageFormat::Png)
        );
        assert_eq!(
            image_format("assets/photo.jpeg", b"\xff\xd8\xffrest"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            image_format("assets/photo", b"RIFFxxxxWEBPrest"),
            Some(ImageFormat::Webp)
        );
        assert_eq!(
            image_format("assets/icon.svg", br#"<svg viewBox="0 0 1 1"></svg>"#),
            Some(ImageFormat::Svg)
        );
        assert_eq!(image_format("assets/file.bin", b"not an image"), None);
    }
}
