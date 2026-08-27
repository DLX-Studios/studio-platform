//! Native controls used to prove the Wayland-only GPUI foundation.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::Duration,
};

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
    input::{Input, InputEvent, InputState, NumberInput, OtpInput, OtpState},
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

#[allow(
    clippy::cast_possible_truncation,
    reason = "opacity is clamped to [0, 1]"
)]
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

fn prop_bool(props: &BTreeMap<String, serde_json::Value>, key: &str, default: bool) -> bool {
    props
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(default)
}

fn prop_str<'a>(props: &'a BTreeMap<String, serde_json::Value>, key: &str) -> Option<&'a str> {
    props
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

fn prop_f64(props: &BTreeMap<String, serde_json::Value>, key: &str, default: f64) -> f64 {
    props
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default)
}

fn prop_u64(props: &BTreeMap<String, serde_json::Value>, key: &str, default: u64) -> u64 {
    props
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(default)
}

/// Declared string-list properties (`items`, `options`, `columns`, `commands`).
fn prop_strings(props: &BTreeMap<String, serde_json::Value>, key: &str) -> Vec<String> {
    props
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Parse one numeric input buffer for `NumberInput` change dispatch.
fn parse_number_input(raw: &str) -> Option<f64> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Stable-ID handling for retained form widgets (ticket 32 decision): every stateful widget is
/// keyed by the stable protocol node ID and kept in a retained map across targeted property
/// patches, so GPUI focus follows the same entity and mounted state survives re-renders. Entries
/// are pruned when a render pass no longer visits their node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputBinding {
    Text,
    Multiline,
    Secret,
    Number,
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
    plugin_inputs: BTreeMap<String, Entity<InputState>>,
    plugin_selects: BTreeMap<String, Entity<SelectState<Vec<SharedString>>>>,
    plugin_sliders: BTreeMap<String, Entity<SliderState>>,
    plugin_otps: BTreeMap<String, Entity<OtpState>>,
    plugin_state_subscriptions: BTreeMap<String, Vec<Subscription>>,
    visited_input_ids: BTreeSet<String>,
    overlay_depth: usize,
    dismissed_overlays: BTreeSet<String>,
    overlay_focus: BTreeMap<String, FocusHandle>,
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
        let component_date_picker = cx.new(|cx| DatePickerState::new(window, cx));
        let component_color_picker = cx.new(|cx| ColorPickerState::new(window, cx));

        Self {
            model: FoundationGalleryModel::new(reduced_motion),
            root_focus: cx.focus_handle(),
            controls: [
                cx.focus_handle().tab_index(1).tab_stop(true),
                cx.focus_handle().tab_index(2).tab_stop(true),
                cx.focus_handle().tab_index(3).tab_stop(true),
            ],
            plugin_inputs: BTreeMap::new(),
            plugin_selects: BTreeMap::new(),
            plugin_sliders: BTreeMap::new(),
            plugin_otps: BTreeMap::new(),
            plugin_state_subscriptions: BTreeMap::new(),
            visited_input_ids: BTreeSet::new(),
            overlay_depth: 0,
            dismissed_overlays: BTreeSet::new(),
            overlay_focus: BTreeMap::new(),
            component_date_picker,
            component_color_picker,
            _component_subscriptions: Vec::new(),
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

    fn dispatch_input(&mut self, node_id: &str, action: InputAction, cx: &mut Context<Self>) {
        if let Some(surface) = self.plugin_surface.as_mut() {
            let _ = surface.process_input(node_id, action);
        }
        cx.notify();
    }

    /// Retain (or create) one stable-ID text input state for a plugin node.
    fn plugin_input(
        &mut self,
        node_id: &str,
        placeholder: &str,
        initial_value: &str,
        binding: InputBinding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        self.visited_input_ids.insert(node_id.to_owned());
        if let Some(state) = self.plugin_inputs.get(node_id) {
            return state.clone();
        }
        let placeholder = placeholder.to_owned();
        let initial_value = initial_value.to_owned();
        let state = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(placeholder);
            match binding {
                // Secret inputs are masked at the native layer and their buffers are never
                // mirrored into host state or events.
                InputBinding::Secret => state = state.masked(true),
                InputBinding::Multiline => state = state.multi_line(true),
                InputBinding::Text | InputBinding::Number => {}
            }
            if !initial_value.is_empty() && binding != InputBinding::Secret {
                state.set_value(initial_value, window, cx);
            }
            state
        });
        let change_subscription = cx.subscribe_in(&state, window, {
            let node_id = node_id.to_owned();
            let state = state.clone();
            move |this, _, event: &InputEvent, _, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let raw = state.read(cx).value().to_string();
                let action = match binding {
                    // Secret input values must never enter the protocol event path; only the
                    // separate HostSecretInput ready flow crosses the boundary.
                    InputBinding::Secret => None,
                    InputBinding::Number => {
                        parse_number_input(&raw).map(|value| InputAction::SliderDrag { value })
                    }
                    InputBinding::Text | InputBinding::Multiline => {
                        Some(InputAction::TextChanged { value: raw })
                    }
                };
                if let Some(action) = action {
                    this.dispatch_input(&node_id, action, cx);
                }
            }
        });
        self.plugin_inputs.insert(node_id.to_owned(), state.clone());
        self.plugin_state_subscriptions
            .entry(node_id.to_owned())
            .or_default()
            .push(change_subscription);
        state
    }

    /// Retain (or create) one stable-ID select state for a plugin node.
    fn plugin_select(
        &mut self,
        node_id: &str,
        options: Vec<SharedString>,
        selected: Option<IndexPath>,
        searchable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SelectState<Vec<SharedString>>> {
        self.visited_input_ids.insert(node_id.to_owned());
        if let Some(state) = self.plugin_selects.get(node_id) {
            return state.clone();
        }
        let state =
            cx.new(|cx| SelectState::new(options, selected, window, cx).searchable(searchable));
        let confirm_subscription = cx.subscribe_in(&state, window, {
            let node_id = node_id.to_owned();
            move |this, _, event: &SelectEvent<Vec<SharedString>>, _, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                this.dispatch_input(
                    &node_id,
                    InputAction::SelectionChanged {
                        value: value.to_string(),
                    },
                    cx,
                );
            }
        });
        self.plugin_selects
            .insert(node_id.to_owned(), state.clone());
        self.plugin_state_subscriptions
            .entry(node_id.to_owned())
            .or_default()
            .push(confirm_subscription);
        state
    }

    /// Retain (or create) one stable-ID slider state for a plugin node. A single-value slider
    /// passes `value_range: None` and its protocol `value` via `single`.
    #[allow(
        clippy::too_many_arguments,
        reason = "closed schema mirrors every slider property"
    )]
    fn plugin_slider(
        &mut self,
        node_id: &str,
        min: f32,
        max: f32,
        step: f32,
        single: f32,
        value_range: Option<(f32, f32)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SliderState> {
        self.visited_input_ids.insert(node_id.to_owned());
        if let Some(state) = self.plugin_sliders.get(node_id) {
            return state.clone();
        }
        let state = cx.new(|_| {
            let mut state = SliderState::new().min(min).max(max);
            state = if step > 0.0 { state.step(step) } else { state };
            match value_range {
                Some((start, end)) => state.default_value((start, end)),
                None => state.default_value(single),
            }
        });
        let change_subscription = cx.subscribe_in(&state, window, {
            let node_id = node_id.to_owned();
            move |this, _, event: &SliderEvent, _, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                this.dispatch_input(
                    &node_id,
                    InputAction::SliderDrag {
                        value: f64::from(value.end()),
                    },
                    cx,
                );
            }
        });
        self.plugin_sliders
            .insert(node_id.to_owned(), state.clone());
        self.plugin_state_subscriptions
            .entry(node_id.to_owned())
            .or_default()
            .push(change_subscription);
        state
    }

    /// Retain (or create) one stable-ID OTP state for a plugin node.
    fn plugin_otp(
        &mut self,
        node_id: &str,
        length: usize,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<OtpState> {
        self.visited_input_ids.insert(node_id.to_owned());
        if let Some(state) = self.plugin_otps.get(node_id) {
            return state.clone();
        }
        let value = value.to_owned();
        let state = cx.new(|cx| OtpState::new(length, window, cx).default_value(value));
        let change_subscription = cx.subscribe_in(&state, window, {
            let node_id = node_id.to_owned();
            let state = state.clone();
            move |this, _, event: &InputEvent, _, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                this.dispatch_input(
                    &node_id,
                    InputAction::TextChanged {
                        value: state.read(cx).value().to_string(),
                    },
                    cx,
                );
            }
        });
        self.plugin_otps.insert(node_id.to_owned(), state.clone());
        self.plugin_state_subscriptions
            .entry(node_id.to_owned())
            .or_default()
            .push(change_subscription);
        state
    }

    fn prune_retired_widget_states(&mut self) {
        let live = std::mem::take(&mut self.visited_input_ids);
        self.plugin_inputs.retain(|id, _| live.contains(id));
        self.plugin_selects.retain(|id, _| live.contains(id));
        self.plugin_sliders.retain(|id, _| live.contains(id));
        self.plugin_otps.retain(|id, _| live.contains(id));
        self.plugin_state_subscriptions
            .retain(|id, _| live.contains(id));
    }

    /// Host-owned overlay gating: returns the stacking depth for a visible overlay, or `None`
    /// when the overlay is closed or host-dismissed. Dismissal state resets whenever the
    /// protocol reports the overlay closed so reopening works without remounts.
    fn overlay_gate(&mut self, node_id: &str, open: bool, cx: &mut Context<Self>) -> Option<usize> {
        if !open {
            self.dismissed_overlays.remove(node_id);
            return None;
        }
        if self.dismissed_overlays.contains(node_id) {
            return None;
        }
        let depth = self.overlay_depth;
        self.overlay_depth += 1;
        self.overlay_focus
            .entry(node_id.to_owned())
            .or_insert_with(|| cx.focus_handle());
        Some(depth)
    }

    fn dismiss_overlay(&mut self, node_id: &str, cx: &mut Context<Self>) {
        self.dismissed_overlays.insert(node_id.to_owned());
        cx.notify();
    }

    /// Shared empty-state placeholder used by data-display kinds when a declared collection
    /// (items/columns/children) is absent. Loading/error states are not expressible under the
    /// closed schema, so only empty and populated states exist.
    #[allow(
        clippy::unused_self,
        reason = "keeps helper grouped with gallery renderers"
    )]
    fn empty_state_element(&self, label: &str) -> AnyElement {
        div()
            .id(format!("empty:{label}"))
            .w_full()
            .p_4()
            .rounded_md()
            .border_1()
            .border_color(rgb(COLOR_BORDER_SUBTLE))
            .bg(rgb(COLOR_SURFACE_VARIANT))
            .flex()
            .flex_col()
            .items_center()
            .text_sm()
            .text_color(rgb(COLOR_MUTED))
            .child(label.to_owned())
            .into_any_element()
    }

    /// Full-screen overlay root with host-owned Escape dismissal. This gpui build has no
    /// z-index; stacking follows tree paint order, so the gate depth only disambiguates IDs.
    #[allow(
        clippy::unused_self,
        reason = "keeps helper grouped with gallery renderers"
    )]
    fn overlay_root(
        &self,
        node_id: &str,
        depth: usize,
        dimmed: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let dismiss_id = node_id.to_owned();
        div()
            .id(format!("{node_id}:overlay:{depth}"))
            .absolute()
            .inset_0()
            .when(dimmed, |element| element.bg(gpui::hsla(0.0, 0.0, 0.0, 0.5)))
            .flex()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" {
                    // Host-owned dismissal: Escape hides the overlay locally; the dismissal
                    // resets when the protocol reports `open=false` for the same node.
                    this.dismiss_overlay(&dismiss_id, cx);
                }
            }))
    }

    fn overlay_panel(
        title: String,
        message: Option<String>,
        width: f32,
        children: Vec<AnyElement>,
    ) -> gpui::Div {
        div()
            .w(px(width))
            .max_w(px(560.0))
            .p_6()
            .rounded_xl()
            .bg(rgb(COLOR_SURFACE))
            .border_1()
            .border_color(rgb(COLOR_BORDER))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .when_some(message, |element, message| {
                element.child(div().text_sm().text_color(rgb(COLOR_MUTED)).child(message))
            })
            .children(children)
    }

    /// Render a Select or Combobox node from its closed schema (label/value/options/enabled).
    fn select_like_element(
        &mut self,
        node: &PluginRenderNode,
        opacity: f32,
        accessibility_label: Option<String>,
        searchable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = prop_str(&node.props, "label")
            .unwrap_or("Select")
            .to_owned();
        let value = prop_str(&node.props, "value")
            .unwrap_or_default()
            .to_owned();
        let enabled = prop_bool(&node.props, "enabled", true);
        let options = node
            .props
            .get("options")
            .and_then(serde_json::Value::as_array)
            .map(|options| {
                options
                    .iter()
                    .filter_map(|option| option.as_str())
                    .map(SharedString::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected = options
            .iter()
            .position(|option| option.as_str() == value.as_str())
            .map(IndexPath::new);
        let state = self.plugin_select(&node.id, options, selected, searchable, window, cx);
        div()
            .id(node.id.clone())
            .opacity(opacity)
            .when_some(accessibility_label, |element, aria| {
                element.aria_label(aria)
            })
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_sm().text_color(rgb(COLOR_MUTED)).child(label))
            .child(Select::new(&state).placeholder(value).disabled(!enabled))
            .into_any_element()
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
            .clone()
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
                    .when(
                        horizontal,
                        gpui::StatefulInteractiveElement::overflow_x_scroll,
                    )
                    .when(
                        !horizontal,
                        gpui::StatefulInteractiveElement::overflow_y_scroll,
                    )
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
                    .when(
                        horizontal,
                        gpui::StatefulInteractiveElement::overflow_x_scroll,
                    )
                    .when(
                        !horizontal,
                        gpui::StatefulInteractiveElement::overflow_y_scroll,
                    )
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
                let background = semantic_background(
                    node.props
                        .get("background")
                        .and_then(serde_json::Value::as_str),
                );
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
                            gpui::StatefulInteractiveElement::aria_label,
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
                            gpui::StatefulInteractiveElement::aria_label,
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
                let requested_open = prop_bool(&node.props, "open", false);
                // Host-owned gating keeps Escape-dismissal consistent with other overlays;
                // the native popover still handles trigger-anchored presentation.
                let open = self.overlay_gate(&node.id, requested_open, cx).is_some();
                // AnyElement is not Clone and the content closure runs per frame, so the
                // declared children are attached eagerly; Popover renders `self.children`
                // inside its popup content panel.
                Popover::new(node.id)
                    .default_open(open)
                    .trigger(Button::new("popover-trigger").secondary().label("Open"))
                    .children(children)
                    .opacity(opacity)
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
                    .when_some(alt, gpui::StatefulInteractiveElement::aria_label)
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
                .role(Role::Navigation)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
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
                .role(Role::Banner)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
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
                .role(Role::GenericContainer)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .size_full()
                .flex()
                .flex_col()
                .min_h_0()
                .bg(rgb(COLOR_BACKGROUND))
                .children(children)
                .into_any_element(),
            NodeKind::Tabs => {
                let items = prop_strings(&node.props, "items");
                let id_prefix = node.id.clone();
                div()
                    .id(node.id)
                    .role(Role::TabList)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .p_1()
                            .rounded_lg()
                            .bg(rgb(COLOR_SURFACE_VARIANT))
                            // Tab selection is carried per-child via `selected`; this header
                            // renders only the declared item labels without inventing state.
                            .children(items.iter().map(|item| {
                                div()
                                    .id(format!("{id_prefix}:tab:{item}"))
                                    .role(Role::Tab)
                                    .text_sm()
                                    .child(item.clone())
                            })),
                    )
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Breadcrumb => div()
                .id(node.id)
                .role(Role::Navigation)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(rgb(COLOR_MUTED))
                .children(children)
                .into_any_element(),
            NodeKind::StatusBar => div()
                .id(node.id)
                .role(Role::Footer)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
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
            NodeKind::NavigationBar | NodeKind::NavigationRail => {
                let vertical = node.kind == NodeKind::NavigationRail;
                div()
                    .id(node.id)
                    .role(Role::Navigation)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .flex_shrink_0()
                    .flex()
                    .when(vertical, gpui::Styled::flex_col)
                    .when(!vertical, gpui::Styled::flex_row)
                    .when(vertical, gpui::Styled::h_full)
                    .items_center()
                    .gap_2()
                    .p_2()
                    .rounded_lg()
                    .bg(rgb(COLOR_SURFACE_VARIANT))
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Stepper => {
                let items = prop_strings(&node.props, "items");
                let step = prop_u64(&node.props, "step", 0) as usize;
                let id_prefix = node.id.clone();
                div()
                    .id(node.id)
                    .role(Role::List)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when(items.is_empty() && children.is_empty(), |element| {
                        element.child(self.empty_state_element("No steps"))
                    })
                    .children(items.iter().enumerate().map(|(index, label)| {
                        let current = index == step;
                        div()
                            .id(format!("{id_prefix}:step:{index}"))
                            .role(Role::ListItem)
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_sm()
                            .text_color(if current || index < step {
                                rgb(COLOR_TEXT)
                            } else {
                                rgb(COLOR_MUTED)
                            })
                            .child(format!("{}. {label}", index + 1))
                    }))
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Pagination => {
                let page = prop_u64(&node.props, "page", 1).max(1);
                let pages = prop_u64(&node.props, "pages", 1).max(1);
                div()
                    .id(node.id)
                    .role(Role::Navigation)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER_SUBTLE))
                    .bg(rgb(COLOR_SURFACE))
                    .text_sm()
                    .child("‹")
                    .child(div().child(format!("Page {page} of {pages}")))
                    .child("›")
                    .into_any_element()
            }
            NodeKind::ListTile => div()
                .id(node.id)
                .role(Role::ListItem)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_2()
                .border_b_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .child(
                    div().min_w_0().overflow_hidden().child(
                        prop_str(&node.props, "label")
                            .unwrap_or_default()
                            .to_owned(),
                    ),
                )
                .children(children)
                .into_any_element(),
            NodeKind::SearchableList | NodeKind::VirtualList => div()
                .id(node.id)
                .role(Role::List)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .w_full()
                .min_w_0()
                .max_h(px(320.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .when(children.is_empty(), |element| {
                    element.child(self.empty_state_element("No entries"))
                })
                .children(children)
                .into_any_element(),
            NodeKind::DataTable => {
                let columns = prop_strings(&node.props, "columns");
                let populated = !columns.is_empty() || !children.is_empty();
                let id_prefix = node.id.clone();
                div()
                    .id(node.id)
                    .role(Role::Table)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .w_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(!populated, |element| {
                        element.child(self.empty_state_element("No rows"))
                    })
                    .when(!columns.is_empty(), |element| {
                        element.child(
                            div()
                                .id(format!("{id_prefix}:columns-header"))
                                .role(Role::Row)
                                .flex()
                                .gap_4()
                                .pb_1()
                                .border_b_1()
                                .border_color(rgb(COLOR_BORDER))
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(rgb(COLOR_MUTED))
                                .children(columns.iter().map(|column| {
                                    div()
                                        .id(format!("{id_prefix}:col:{column}"))
                                        .child(column.clone())
                                })),
                        )
                    })
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Tree => div()
                .id(node.id)
                .role(Role::Tree)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .pl_3()
                .border_l_1()
                .border_color(rgb(COLOR_BORDER_SUBTLE))
                .when(children.is_empty(), |element| {
                    element.child(self.empty_state_element("No nodes"))
                })
                .children(children)
                .into_any_element(),
            NodeKind::DescriptionList => div()
                .id(node.id)
                .role(Role::DescriptionList)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .w_full()
                .min_w_0()
                .grid()
                .grid_cols(2)
                .gap_2()
                .when(children.is_empty(), |element| {
                    element.child(self.empty_state_element("No details"))
                })
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
                let open = prop_bool(&node.props, "open", false);
                let title = prop_str(&node.props, "title")
                    .unwrap_or("Dialog")
                    .to_owned();
                let Some(depth) = self.overlay_gate(&node.id, open, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                let panel = Self::overlay_panel(title, None, 480.0, children)
                    .id(format!("{}:panel", node.id))
                    .role(Role::Dialog)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    });
                // UNVERIFIED: focus is tracked host-side (overlay_focus handles + tab order);
                // full Tab cycling inside the overlay must be confirmed by the runner pass.
                let root = self
                    .overlay_root(&node.id, depth, true, cx)
                    .items_center()
                    .justify_center()
                    .child(panel);
                if self.model.reduced_motion() {
                    root.into_any_element()
                } else {
                    root.with_animation(
                        format!("{}:fade", node.id),
                        Animation::new(Duration::from_millis(150)),
                        gpui::Styled::opacity,
                    )
                    .into_any_element()
                }
            }
            NodeKind::AlertDialog => {
                let open = prop_bool(&node.props, "open", false);
                let title = prop_str(&node.props, "title")
                    .unwrap_or("Confirm")
                    .to_owned();
                let message = prop_str(&node.props, "message").map(ToOwned::to_owned);
                let Some(depth) = self.overlay_gate(&node.id, open, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                let panel = Self::overlay_panel(title, message, 420.0, children)
                    .id(format!("{}:panel", node.id))
                    .role(Role::AlertDialog)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    });
                let root = self
                    .overlay_root(&node.id, depth, true, cx)
                    .items_center()
                    .justify_center()
                    .child(panel);
                if self.model.reduced_motion() {
                    root.into_any_element()
                } else {
                    root.with_animation(
                        format!("{}:fade", node.id),
                        Animation::new(Duration::from_millis(150)),
                        gpui::Styled::opacity,
                    )
                    .into_any_element()
                }
            }
            NodeKind::Sheet | NodeKind::BottomSheet | NodeKind::Drawer => {
                let open = prop_bool(&node.props, "open", false);
                let title = prop_str(&node.props, "title").unwrap_or("").to_owned();
                let Some(depth) = self.overlay_gate(&node.id, open, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                let panel_id = format!("{}:panel", node.id);
                let panel = div()
                    .id(panel_id)
                    .role(Role::Dialog)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .bg(rgb(COLOR_SURFACE))
                    .border_color(rgb(COLOR_BORDER))
                    .shadow_lg()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(!title.is_empty(), |element| {
                        element.child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(title),
                        )
                    })
                    .children(children);
                // Sheets anchor to their declared edge; drawers dock left like sheets.
                let panel = match node.kind {
                    NodeKind::BottomSheet => {
                        panel.w_full().max_h(px(320.0)).rounded_t_xl().border_t_1()
                    }
                    NodeKind::Drawer => panel.w(px(280.0)).h_full().rounded_r_xl().border_r_1(),
                    _ => panel.w(px(360.0)).h_full().rounded_l_xl().border_l_1(),
                };
                let root = self.overlay_root(&node.id, depth, true, cx);
                let root = if node.kind == NodeKind::BottomSheet {
                    root.items_end().justify_center()
                } else if node.kind == NodeKind::Drawer {
                    root.items_stretch().justify_start()
                } else {
                    root.items_stretch().justify_end()
                };
                let root = root.child(panel);
                if self.model.reduced_motion() {
                    root.into_any_element()
                } else {
                    root.with_animation(
                        format!("{}:fade", node.id),
                        Animation::new(Duration::from_millis(150)),
                        gpui::Styled::opacity,
                    )
                    .into_any_element()
                }
            }
            NodeKind::Toast => {
                let message = prop_str(&node.props, "message").map(ToOwned::to_owned);
                if message.is_none() {
                    // No open property exists for toasts; a missing message means closed and
                    // clears any host-owned dismissal so the next message shows again.
                    self.dismissed_overlays.remove(&node.id);
                    return div().id(node.id).hidden().into_any_element();
                }
                let Some(depth) = self.overlay_gate(&node.id, true, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                let dismiss_id = node.id.clone();
                self.overlay_root(&node.id, depth, false, cx)
                    .items_start()
                    .justify_center()
                    .pt_4()
                    .child(
                        div()
                            .id(format!("{}:toast", node.id))
                            .role(Role::Alert)
                            .when_some(accessibility_label.clone(), |element, aria| {
                                element.aria_label(aria)
                            })
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(COLOR_TEXT))
                            .text_color(rgb(COLOR_SURFACE))
                            .text_sm()
                            .shadow_lg()
                            .child(message.unwrap_or_default())
                            // Host-owned dismissal: clicking a toast dismisses it locally.
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_overlay(&dismiss_id, cx);
                            })),
                    )
                    .into_any_element()
            }
            NodeKind::Notification => {
                let message = prop_str(&node.props, "message").map(ToOwned::to_owned);
                if message.is_none() {
                    self.dismissed_overlays.remove(&node.id);
                    return div().id(node.id).hidden().into_any_element();
                }
                let Some(depth) = self.overlay_gate(&node.id, true, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                let dismiss_id = node.id.clone();
                self.overlay_root(&node.id, depth, false, cx)
                    .items_start()
                    .justify_end()
                    .p_4()
                    .child(
                        div()
                            .id(format!("{}:notification", node.id))
                            .role(Role::Alert)
                            .when_some(accessibility_label.clone(), |element, aria| {
                                element.aria_label(aria)
                            })
                            .w(px(320.0))
                            .p_3()
                            .rounded_md()
                            .bg(rgb(COLOR_CARD))
                            .border_1()
                            .border_color(rgb(COLOR_BORDER))
                            .shadow_lg()
                            .text_sm()
                            .child(message.unwrap_or_default())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_overlay(&dismiss_id, cx);
                            })),
                    )
                    .into_any_element()
            }
            NodeKind::Banner => div()
                .id(node.id)
                .role(Role::Alert)
                .opacity(opacity)
                .when_some(accessibility_label.clone(), |element, aria| {
                    element.aria_label(aria)
                })
                .w_full()
                .px_4()
                .py_2()
                .rounded_md()
                .bg(rgb(COLOR_SURFACE_VARIANT))
                .border_1()
                .border_color(rgb(COLOR_BORDER))
                .text_sm()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    prop_str(&node.props, "message")
                        .unwrap_or_default()
                        .to_owned(),
                )
                .children(children)
                .into_any_element(),
            NodeKind::ContextMenu => {
                let open = prop_bool(&node.props, "open", false);
                let Some(depth) = self.overlay_gate(&node.id, open, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                // No position property exists in the closed schema, so the menu surfaces
                // centered until the protocol grows placement semantics.
                let menu = div()
                    .id(format!("{}:menu", node.id))
                    .role(Role::Menu)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .min_w(px(200.0))
                    .p_2()
                    .rounded_md()
                    .bg(rgb(COLOR_CARD))
                    .border_1()
                    .border_color(rgb(COLOR_BORDER))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when_some(prop_str(&node.props, "message"), |element, header| {
                        element.child(
                            div()
                                .text_xs()
                                .text_color(rgb(COLOR_MUTED))
                                .child(header.to_owned()),
                        )
                    })
                    .children(children);
                self.overlay_root(&node.id, depth, false, cx)
                    .items_center()
                    .justify_center()
                    .child(menu)
                    .into_any_element()
            }
            NodeKind::CommandPalette => {
                let open = prop_bool(&node.props, "open", false);
                let placeholder = prop_str(&node.props, "placeholder")
                    .unwrap_or("Type a command")
                    .to_owned();
                let commands = prop_strings(&node.props, "commands");
                let Some(depth) = self.overlay_gate(&node.id, open, cx) else {
                    return div().id(node.id).hidden().into_any_element();
                };
                let palette = div()
                    .id(format!("{}:palette", node.id))
                    .role(Role::Dialog)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .w(px(520.0))
                    .max_h(px(400.0))
                    .overflow_y_scroll()
                    .p_2()
                    .rounded_lg()
                    .bg(rgb(COLOR_CARD))
                    .border_1()
                    .border_color(rgb(COLOR_BORDER))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .text_color(rgb(COLOR_MUTED))
                            .child(placeholder),
                    )
                    .when(commands.is_empty(), |element| {
                        element.child(self.empty_state_element("No commands"))
                    })
                    .children(commands.iter().map(|command| {
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .hover(|style| style.bg(rgb(COLOR_SURFACE_VARIANT)))
                            .child(command.clone())
                    }))
                    .children(children);
                self.overlay_root(&node.id, depth, true, cx)
                    .items_start()
                    .justify_center()
                    .pt_16()
                    .child(palette)
                    .into_any_element()
            }
            NodeKind::Tooltip => {
                let tip = prop_str(&node.props, "message")
                    .unwrap_or_default()
                    .to_owned();
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .min_w_0()
                    .flex()
                    .children(children)
                    .when(!tip.is_empty(), |element| {
                        element.tooltip(move |window, cx| {
                            use gpui_component::tooltip;
                            tooltip::Tooltip::new(tip.clone()).build(window, cx)
                        })
                    })
                    .into_any_element()
            }
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
                let label = prop_str(&node.props, "label")
                    .unwrap_or("Button")
                    .to_owned();
                let variant = prop_str(&node.props, "variant").unwrap_or("primary");
                let enabled = prop_bool(&node.props, "enabled", true);
                // UNVERIFIED: the closed protocol declares a "selected" button variant but
                // gpui-component's Button has no selected style; it renders as primary until the
                // runner/fixer pass confirms host styling policy.
                let is_card_action = node_id.starts_with("add-");
                // gpui-component's Button exposes no aria_label; a declared custom
                // accessibility label cannot attach here (semantic finding).
                let button = Button::new(node_id)
                    .label(label)
                    .disabled(!enabled)
                    .opacity(opacity)
                    .when(is_card_action, gpui::Styled::w_full);
                let button = match variant {
                    "secondary" => button.secondary(),
                    _ => button.primary(),
                };
                button
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch_input(&click_id, InputAction::PointerClick, cx);
                    }))
                    .into_any_element()
            }
            NodeKind::IconButton if node.control == Some(RuntimeControl::Button) => {
                let icon = prop_str(&node.props, "icon").unwrap_or("").to_owned();
                let enabled = prop_bool(&node.props, "enabled", true);
                let click_id = node.id.clone();
                let key_id = click_id.clone();
                div()
                    .id(node.id)
                    .role(Role::Button)
                    .aria_label(accessibility_label.unwrap_or_else(|| icon.clone()))
                    .opacity(opacity)
                    .size(px(36.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER))
                    .bg(rgb(COLOR_SURFACE_VARIANT))
                    .text_color(if enabled {
                        rgb(COLOR_TEXT)
                    } else {
                        rgb(COLOR_MUTED)
                    })
                    .text_xs()
                    .when(!enabled, |element| element.opacity(0.5 * opacity))
                    .child(icon)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.dispatch_input(&click_id, InputAction::PointerClick, cx);
                    }))
                    // UNVERIFIED: keyboard activation relies on GPUI focusable div key handling;
                    // touch is covered by pointer synthesis in the Wayland input path.
                    .focusable()
                    .tab_stop(true)
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            this.dispatch_input(&key_id, InputAction::KeyboardActivate, cx);
                        }
                    }))
                    .into_any_element()
            }
            NodeKind::Checkbox | NodeKind::Radio | NodeKind::Switch | NodeKind::Toggle => {
                let label = prop_str(&node.props, "label")
                    .unwrap_or_default()
                    .to_owned();
                let checked = prop_bool(&node.props, "value", false);
                let enabled = prop_bool(&node.props, "enabled", true);
                let change_id = node.id.clone();
                let on_change = cx.listener(move |this, checked: &bool, _, cx| {
                    this.dispatch_input(
                        &change_id,
                        InputAction::SelectionChanged {
                            value: checked.to_string(),
                        },
                        cx,
                    );
                });
                let inner: AnyElement = match node.kind {
                    NodeKind::Radio => Radio::new(node.id.clone())
                        .label(label)
                        .checked(checked)
                        .disabled(!enabled)
                        .on_click(on_change)
                        .into_any_element(),
                    NodeKind::Switch | NodeKind::Toggle => Switch::new(node.id.clone())
                        .label(label)
                        .checked(checked)
                        .disabled(!enabled)
                        .on_click(on_change)
                        .into_any_element(),
                    _ => Checkbox::new(node.id.clone())
                        .label(label)
                        .checked(checked)
                        .disabled(!enabled)
                        .on_click(on_change)
                        .into_any_element(),
                };
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label, |element, aria| {
                        element.aria_label(aria)
                    })
                    .max_w_full()
                    .child(inner)
                    .into_any_element()
            }
            NodeKind::ButtonGroup => {
                let vertical =
                    prop_str(&node.props, "orientation").unwrap_or("horizontal") == "vertical";
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .when(vertical, gpui::Styled::flex_col)
                    .when(!vertical, gpui::Styled::flex_row)
                    .gap(px(gap))
                    .children(children)
                    .into_any_element()
            }
            NodeKind::Slider if node.control == Some(RuntimeControl::Slider) => {
                let label = prop_str(&node.props, "label")
                    .unwrap_or("Slider")
                    .to_owned();
                let min = prop_f64(&node.props, "min", 0.0) as f32;
                let max = prop_f64(&node.props, "max", 1.0) as f32;
                let value =
                    prop_f64(&node.props, "value", min.into()).clamp(min.into(), max.into());
                let enabled = prop_bool(&node.props, "enabled", true);
                let state =
                    self.plugin_slider(&node.id, min, max, 0.0, value as f32, None, window, cx);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(COLOR_MUTED))
                            .child(format!("{label}: {value:.2}")),
                    )
                    .child(Slider::new(&state).horizontal().disabled(!enabled))
                    .into_any_element()
            }
            NodeKind::RangeSlider => {
                let label = prop_str(&node.props, "label").unwrap_or("Range").to_owned();
                let min = prop_f64(&node.props, "min", 0.0) as f32;
                let max = prop_f64(&node.props, "max", 1.0) as f32;
                let start =
                    prop_f64(&node.props, "start", min.into()).clamp(min.into(), max.into());
                let end = prop_f64(&node.props, "end", max.into()).clamp(start, max.into());
                let enabled = prop_bool(&node.props, "enabled", true);
                let state = self.plugin_slider(
                    &node.id,
                    min,
                    max,
                    0.0,
                    start as f32,
                    Some((start as f32, end as f32)),
                    window,
                    cx,
                );
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(COLOR_MUTED))
                            .child(format!("{label}: {start:.2} – {end:.2}")),
                    )
                    .child(Slider::new(&state).horizontal().disabled(!enabled))
                    .into_any_element()
            }
            NodeKind::Select if node.control == Some(RuntimeControl::Select) => {
                self.select_like_element(&node, opacity, accessibility_label, false, window, cx)
            }
            NodeKind::Combobox => {
                // A combobox is rendered as the same closed select contract with native search;
                // options/value/on_changed semantics stay exactly as declared.
                self.select_like_element(&node, opacity, accessibility_label, true, window, cx)
            }
            NodeKind::TextInput if node.control == Some(RuntimeControl::Input) => {
                let placeholder = prop_str(&node.props, "placeholder")
                    .unwrap_or_default()
                    .to_owned();
                let initial_value = prop_str(&node.props, "value")
                    .unwrap_or_default()
                    .to_owned();
                let enabled = prop_bool(&node.props, "enabled", true);
                let state = self.plugin_input(
                    &node.id,
                    &placeholder,
                    &initial_value,
                    InputBinding::Text,
                    window,
                    cx,
                );
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .flex_grow_1()
                    .child(Input::new(&state).disabled(!enabled))
                    .into_any_element()
            }
            NodeKind::TextArea => {
                let placeholder = prop_str(&node.props, "placeholder")
                    .unwrap_or_default()
                    .to_owned();
                let initial_value = prop_str(&node.props, "value")
                    .unwrap_or_default()
                    .to_owned();
                let enabled = prop_bool(&node.props, "enabled", true);
                let state = self.plugin_input(
                    &node.id,
                    &placeholder,
                    &initial_value,
                    InputBinding::Multiline,
                    window,
                    cx,
                );
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .w_full()
                    .child(Input::new(&state).disabled(!enabled))
                    .into_any_element()
            }
            NodeKind::NumberInput => {
                let placeholder = prop_str(&node.props, "label")
                    .unwrap_or_default()
                    .to_owned();
                let initial_value = format!("{}", prop_f64(&node.props, "value", 0.0));
                let enabled = prop_bool(&node.props, "enabled", true);
                let state = self.plugin_input(
                    &node.id,
                    &placeholder,
                    &initial_value,
                    InputBinding::Number,
                    window,
                    cx,
                );
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, label| {
                        element.aria_label(label)
                    })
                    .min_w_0()
                    .w(px(160.0))
                    .child(NumberInput::new(&state).disabled(!enabled))
                    .into_any_element()
            }
            NodeKind::SecretInput if node.control == Some(RuntimeControl::Input) => {
                let label = prop_str(&node.props, "label")
                    .unwrap_or("Trusted input")
                    .to_owned();
                let enabled = prop_bool(&node.props, "enabled", true);
                let state = self.plugin_input(&node.id, "", "", InputBinding::Secret, window, cx);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().text_color(rgb(COLOR_MUTED)).child(label))
                    .child(Input::new(&state).disabled(!enabled))
                    .into_any_element()
            }
            NodeKind::Field | NodeKind::InputGroup => {
                let label = prop_str(&node.props, "label").map(ToOwned::to_owned);
                let description = prop_str(&node.props, "description").map(ToOwned::to_owned);
                let error = prop_str(&node.props, "error").map(ToOwned::to_owned);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .min_w_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when_some(label, |element, label| {
                        element.child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(label.clone()),
                        )
                    })
                    .children(children)
                    .when_some(description, |element, description| {
                        element.child(
                            div()
                                .text_xs()
                                .text_color(rgb(COLOR_MUTED))
                                .child(description.clone()),
                        )
                    })
                    .when_some(error, |element, error| {
                        element.child(
                            div()
                                .id("error")
                                .text_xs()
                                .text_color(rgb(COLOR_ERROR))
                                .role(Role::Alert)
                                .child(error.clone()),
                        )
                    })
                    .into_any_element()
            }
            NodeKind::OtpInput => {
                let label = prop_str(&node.props, "label").unwrap_or("Code").to_owned();
                let length = node
                    .props
                    .get("length")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(6)
                    .clamp(1, 12) as usize;
                let value = prop_str(&node.props, "value")
                    .unwrap_or_default()
                    .to_owned();
                let enabled = prop_bool(&node.props, "enabled", true);
                let state = self.plugin_otp(&node.id, length, &value, window, cx);
                div()
                    .id(node.id)
                    .opacity(opacity)
                    .when_some(accessibility_label.clone(), |element, aria| {
                        element.aria_label(aria)
                    })
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(!enabled, |element| element.opacity(0.5 * opacity))
                    .child(div().text_sm().text_color(rgb(COLOR_MUTED)).child(label))
                    .child(OtpInput::new(&state))
                    .into_any_element()
            }
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
                        gpui::Styled::opacity,
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
            self.visited_input_ids.clear();
            self.overlay_depth = 0;
            let plugin = self.plugin_node(root, window, cx);
            // Retained form-widget states are keyed by stable node ID; states whose nodes left
            // the render tree are pruned so removals do not leak native widgets or buffers.
            self.prune_retired_widget_states();
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
    use super::{
        ImageFormat, PluginRenderNode, image_format, parse_number_input, prop_strings, prop_u64,
    };
    use studio_protocol::NodeKind;

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

    #[test]
    fn parses_numeric_input_buffers_for_number_dispatch() {
        assert_eq!(parse_number_input("42"), Some(42.0));
        assert_eq!(parse_number_input(" 3.5 "), Some(3.5));
        assert_eq!(parse_number_input("-0.25"), Some(-0.25));
        // Non-numeric and non-finite buffers are dropped rather than dispatched.
        assert_eq!(parse_number_input(""), None);
        assert_eq!(parse_number_input("abc"), None);
        assert_eq!(parse_number_input("NaN"), None);
        assert_eq!(parse_number_input("inf"), None);
    }

    #[test]
    fn reads_declared_string_list_properties_for_data_display() {
        use std::collections::BTreeMap;
        let props: BTreeMap<String, serde_json::Value> = BTreeMap::from([
            ("columns".to_owned(), serde_json::json!(["Name", "Price"])),
            ("items".to_owned(), serde_json::json!([])),
        ]);
        let node = PluginRenderNode {
            id: "table".to_owned(),
            kind: NodeKind::DataTable,
            control: None,
            props,
            children: Vec::new(),
        };
        assert_eq!(
            super::prop_strings(&node.props, "columns"),
            vec!["Name".to_owned(), "Price".to_owned()]
        );
        assert!(super::prop_strings(&node.props, "items").is_empty());
        assert!(super::prop_strings(&node.props, "missing").is_empty());
        assert_eq!(super::prop_u64(&node.props, "pages", 1), 1);
    }
}
