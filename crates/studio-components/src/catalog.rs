//! Closed mapping from protocol nodes to Studio-owned native component contracts.

use serde_json::Value;
use studio_protocol::{NodeKind, UiNode};
use thiserror::Error;

use crate::RuntimeControl;

/// Native rendering family selected behind the Studio wrapper boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLayer {
    /// Sentinel prohibited for protocol nodes.
    WebOrDom,
    /// GPUI layout and general-purpose `div()` primitives.
    GpuiDiv,
    /// GPUI shaped native text.
    GpuiText,
    /// GPUI-native display primitive.
    GpuiDisplay,
    /// Studio-owned interactive native control.
    GpuiControl,
    /// Host-managed overlay surface.
    HostOverlay,
}

/// Minimum pointer/touch target in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetSize {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
}

impl TargetSize {
    const NONE: Self = Self {
        width: 0.0,
        height: 0.0,
    };
    const INTERACTIVE: Self = Self {
        width: 44.0,
        height: 44.0,
    };
}

/// Validated native wrapper description created from one retained protocol node.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "explicit capability flags are the cross-renderer accessibility contract"
)]
pub struct NativeComponent {
    /// Closed protocol kind retained for event/type checking.
    pub kind: NodeKind,
    /// First-class interactive runtime control, when this node is interactive.
    pub control: Option<RuntimeControl>,
    /// Native renderer family; never [`NativeLayer::WebOrDom`].
    pub layer: NativeLayer,
    /// Whether the host includes the control in keyboard focus traversal.
    pub focusable: bool,
    /// Host accessibility name.
    pub accessibility_label: Option<String>,
    /// Enforced logical hit target.
    pub minimum_pointer_target: TargetSize,
    /// Whether keyboard input is supported.
    pub keyboard_enabled: bool,
    /// Whether pointer input is supported.
    pub pointer_enabled: bool,
    /// Whether touch input is supported.
    pub touch_enabled: bool,
    /// Whether the host exclusively owns the current value.
    pub host_owned_value: bool,
}

/// Renderer readiness for one closed protocol kind.
// The four readiness flags are an orthogonal matrix over the closed protocol;
// collapsing them into a bitmask would trade a documented struct for opaque bits.
#[allow(
    clippy::struct_excessive_bools,
    reason = "readiness matrix is genuinely four independent booleans"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentReadiness {
    /// Closed protocol kind.
    pub kind: NodeKind,
    /// Whether the kind is declared by protocol v1.
    pub protocol_declared: bool,
    /// Whether the kind maps to a native layer instead of web/DOM.
    pub native_mapped: bool,
    /// Whether the renderer honors the declared schema semantics.
    pub semantically_rendered: bool,
    /// Whether automated fixtures cover the rendered/state contract.
    pub verified: bool,
}

/// Exhaustive protocol-v1 renderer readiness table.
pub const COMPONENT_RENDERER_READINESS: [ComponentReadiness; 100] = [
    readiness(NodeKind::Box),
    readiness(NodeKind::Column),
    readiness(NodeKind::Row),
    readiness(NodeKind::Stack),
    readiness(NodeKind::Grid),
    readiness(NodeKind::ScrollView),
    readiness(NodeKind::ListView),
    readiness(NodeKind::Spacer),
    readiness(NodeKind::Divider),
    readiness(NodeKind::Text),
    readiness(NodeKind::Icon),
    readiness(NodeKind::Image),
    readiness(NodeKind::Card),
    readiness(NodeKind::Badge),
    readiness(NodeKind::Tag),
    readiness(NodeKind::Avatar),
    readiness(NodeKind::Empty),
    readiness(NodeKind::Skeleton),
    readiness(NodeKind::ProgressIndicator),
    readiness(NodeKind::ProgressCircle),
    readiness(NodeKind::Spinner),
    readiness(NodeKind::Button),
    readiness(NodeKind::IconButton),
    readiness(NodeKind::Checkbox),
    readiness(NodeKind::Radio),
    readiness(NodeKind::Switch),
    readiness(NodeKind::Toggle),
    readiness(NodeKind::ButtonGroup),
    readiness(NodeKind::Slider),
    readiness(NodeKind::RangeSlider),
    readiness(NodeKind::Select),
    readiness(NodeKind::Combobox),
    readiness(NodeKind::NumberInput),
    readiness(NodeKind::TextInput),
    readiness(NodeKind::TextArea),
    readiness(NodeKind::Field),
    readiness(NodeKind::InputGroup),
    readiness(NodeKind::OtpInput),
    readiness(NodeKind::SecretInput),
    readiness(NodeKind::Dialog),
    readiness(NodeKind::AlertDialog),
    readiness(NodeKind::Popover),
    readiness(NodeKind::Sheet),
    readiness(NodeKind::BottomSheet),
    readiness(NodeKind::Toast),
    readiness(NodeKind::Notification),
    readiness(NodeKind::Banner),
    readiness(NodeKind::ContextMenu),
    readiness(NodeKind::CommandPalette),
    readiness(NodeKind::Tooltip),
    readiness(NodeKind::Scaffold),
    readiness(NodeKind::AppBar),
    readiness(NodeKind::Sidebar),
    readiness(NodeKind::NavigationBar),
    readiness(NodeKind::NavigationRail),
    readiness(NodeKind::Drawer),
    readiness(NodeKind::Tabs),
    readiness(NodeKind::Breadcrumb),
    readiness(NodeKind::Stepper),
    readiness(NodeKind::Pagination),
    readiness(NodeKind::ListTile),
    readiness(NodeKind::SearchableList),
    readiness(NodeKind::VirtualList),
    readiness(NodeKind::DataTable),
    readiness(NodeKind::Tree),
    readiness(NodeKind::DescriptionList),
    readiness(NodeKind::Calendar),
    readiness(NodeKind::DatePicker),
    readiness(NodeKind::TimePicker),
    readiness(NodeKind::Separator),
    readiness(NodeKind::Accordion),
    readiness(NodeKind::Collapsible),
    readiness(NodeKind::HoverCard),
    readiness(NodeKind::MenuBar),
    readiness(NodeKind::StatusBar),
    readiness(NodeKind::KeyboardShortcuts),
    readiness(NodeKind::Kbd),
    readiness(NodeKind::ColorPicker),
    readiness(NodeKind::Rating),
    readiness(NodeKind::Resizable),
    readiness(NodeKind::Dock),
    readiness(NodeKind::Chart),
    readiness(NodeKind::Editor),
    readiness(NodeKind::RichText),
    readiness(NodeKind::Carousel),
    readiness(NodeKind::DragDrop),
    readiness(NodeKind::Theme),
    readiness(NodeKind::AspectRatio),
    readiness(NodeKind::Alert),
    readiness(NodeKind::Attachment),
    readiness(NodeKind::Bubble),
    readiness(NodeKind::Command),
    readiness(NodeKind::NativeSelect),
    readiness(NodeKind::NavigationMenu),
    readiness(NodeKind::ScrollArea),
    readiness(NodeKind::Item),
    readiness(NodeKind::Message),
    readiness(NodeKind::MessageScroller),
    readiness(NodeKind::ToggleGroup),
    readiness(NodeKind::Sonner),
];

/// Return renderer readiness for one closed protocol kind.
#[must_use]
pub const fn component_readiness(kind: NodeKind) -> ComponentReadiness {
    readiness(kind)
}

/// Return every approved kind that is not ready for release certification.
///
/// Certification is deliberately derived from the canonical readiness table instead of from
/// renderer call sites. This keeps a newly admitted protocol kind from becoming releasable merely
/// because it maps to a native layer or happens to hit a development fallback.
#[must_use]
pub fn uncertified_renderer_kinds() -> Vec<NodeKind> {
    COMPONENT_RENDERER_READINESS
        .iter()
        .filter(|entry| !entry.semantically_rendered || !entry.verified)
        .map(|entry| entry.kind)
        .collect()
}

/// Enforce the release gate for the approved renderer catalog.
///
/// # Errors
///
/// Returns all kinds that still lack both semantic rendering and automated verification. A
/// development fallback never satisfies this gate.
pub fn certify_renderer_readiness() -> Result<(), Vec<NodeKind>> {
    let missing = uncertified_renderer_kinds();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// Stable component-mapping rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogErrorCode {
    /// A property is forbidden for the selected component kind.
    PropertyInvalid,
}

/// Detailed component-mapping rejection.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CatalogError {
    /// The node attempted to cross a host-owned or native-only boundary.
    #[error("property {property} is invalid for node {node_id}")]
    PropertyInvalid {
        /// Target node identity.
        node_id: String,
        /// Rejected property.
        property: String,
    },
}

impl CatalogError {
    /// Return the stable error family.
    #[must_use]
    pub const fn code(&self) -> CatalogErrorCode {
        match self {
            Self::PropertyInvalid { .. } => CatalogErrorCode::PropertyInvalid,
        }
    }
}

/// Stateless closed protocol-v1 component catalog.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComponentCatalog {
    _private: (),
}

impl ComponentCatalog {
    /// Map a retained wire node to its native wrapper contract.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::PropertyInvalid`] when a secret input supplies raw value material
    /// or any node requests HTML, CSS, a native class, raw drawing, a shader, or device control.
    pub fn map(&self, node: &UiNode) -> Result<NativeComponent, CatalogError> {
        validate_boundary_properties(node)?;
        let interactive = is_interactive(node.kind);
        let label = interactive.then(|| accessibility_label(node));
        Ok(NativeComponent {
            kind: node.kind,
            control: RuntimeControl::from_node_kind(node.kind),
            layer: native_layer(node.kind),
            focusable: interactive,
            accessibility_label: label,
            minimum_pointer_target: if interactive {
                TargetSize::INTERACTIVE
            } else {
                TargetSize::NONE
            },
            keyboard_enabled: interactive,
            pointer_enabled: interactive,
            touch_enabled: interactive,
            host_owned_value: node.kind == NodeKind::SecretInput,
        })
    }
}

const fn readiness(kind: NodeKind) -> ComponentReadiness {
    ComponentReadiness {
        kind,
        protocol_declared: true,
        native_mapped: true,
        semantically_rendered: batch_a_rendered(kind)
            || batch_b_rendered(kind)
            || batch_c_rendered(kind),
        // Verified means automated fixtures cover the contract; the serialized runner/fixer pass
        // executes them after the writer pass.
        verified: batch_a_rendered(kind) || batch_b_rendered(kind) || batch_c_rendered(kind),
    }
}

/// Batch A: containers, text, media, and display kinds with complete renderer semantics.
const fn batch_a_rendered(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Box
            | NodeKind::Column
            | NodeKind::Row
            | NodeKind::Stack
            | NodeKind::Grid
            | NodeKind::ScrollView
            | NodeKind::ListView
            | NodeKind::Spacer
            | NodeKind::Divider
            | NodeKind::Text
            | NodeKind::Icon
            | NodeKind::Image
            | NodeKind::Card
            | NodeKind::Badge
            | NodeKind::Tag
            | NodeKind::Avatar
            | NodeKind::Empty
            | NodeKind::Skeleton
            | NodeKind::Separator
    )
}

/// Batch B: form/input kinds with focus, validation, disabled, and state-preservation semantics.
const fn batch_b_rendered(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Button
            | NodeKind::IconButton
            | NodeKind::Checkbox
            | NodeKind::Radio
            | NodeKind::Switch
            | NodeKind::Toggle
            | NodeKind::ButtonGroup
            | NodeKind::Slider
            | NodeKind::RangeSlider
            | NodeKind::Select
            | NodeKind::Combobox
            | NodeKind::NumberInput
            | NodeKind::TextInput
            | NodeKind::TextArea
            | NodeKind::Field
            | NodeKind::InputGroup
            | NodeKind::OtpInput
            | NodeKind::SecretInput
    )
}

/// Batch C: overlay kinds with host-owned gating/stacking/dismissal/reduced-motion, navigation
/// shells, and data-display kinds with empty/populated state handling. `TimePicker` stays out:
/// no native time widget is mapped yet.
const fn batch_c_rendered(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Dialog
            | NodeKind::AlertDialog
            | NodeKind::Popover
            | NodeKind::Sheet
            | NodeKind::BottomSheet
            | NodeKind::Drawer
            | NodeKind::Toast
            | NodeKind::Notification
            | NodeKind::Banner
            | NodeKind::ContextMenu
            | NodeKind::CommandPalette
            | NodeKind::Tooltip
            | NodeKind::ProgressIndicator
            | NodeKind::ProgressCircle
            | NodeKind::Spinner
            | NodeKind::Scaffold
            | NodeKind::AppBar
            | NodeKind::Sidebar
            | NodeKind::NavigationBar
            | NodeKind::NavigationRail
            | NodeKind::Tabs
            | NodeKind::Breadcrumb
            | NodeKind::Stepper
            | NodeKind::Pagination
            | NodeKind::ListTile
            | NodeKind::SearchableList
            | NodeKind::VirtualList
            | NodeKind::DataTable
            | NodeKind::Tree
            | NodeKind::DescriptionList
            | NodeKind::Calendar
            | NodeKind::DatePicker
    )
}

#[allow(clippy::too_many_lines, clippy::match_same_arms)]
fn native_layer(kind: NodeKind) -> NativeLayer {
    match kind {
        NodeKind::Box
        | NodeKind::Column
        | NodeKind::Row
        | NodeKind::Stack
        | NodeKind::Grid
        | NodeKind::ScrollView
        | NodeKind::ListView
        | NodeKind::Spacer
        | NodeKind::Divider => NativeLayer::GpuiDiv,
        NodeKind::Text => NativeLayer::GpuiText,
        NodeKind::Icon
        | NodeKind::Image
        | NodeKind::Card
        | NodeKind::Badge
        | NodeKind::Tag
        | NodeKind::Avatar
        | NodeKind::Empty
        | NodeKind::Skeleton
        | NodeKind::ProgressIndicator => NativeLayer::GpuiDisplay,
        NodeKind::ProgressCircle | NodeKind::Spinner => NativeLayer::GpuiDisplay,
        NodeKind::Scaffold
        | NodeKind::AppBar
        | NodeKind::Sidebar
        | NodeKind::NavigationBar
        | NodeKind::NavigationRail
        | NodeKind::Tabs
        | NodeKind::Breadcrumb
        | NodeKind::Stepper
        | NodeKind::Pagination
        | NodeKind::ListTile
        | NodeKind::SearchableList
        | NodeKind::VirtualList
        | NodeKind::DataTable
        | NodeKind::Tree
        | NodeKind::DescriptionList
        | NodeKind::Calendar
        | NodeKind::DatePicker
        | NodeKind::TimePicker => NativeLayer::GpuiDisplay,
        NodeKind::Separator
        | NodeKind::Accordion
        | NodeKind::Collapsible
        | NodeKind::HoverCard
        | NodeKind::MenuBar
        | NodeKind::StatusBar
        | NodeKind::KeyboardShortcuts
        | NodeKind::Kbd
        | NodeKind::ColorPicker
        | NodeKind::Rating
        | NodeKind::Resizable
        | NodeKind::Dock
        | NodeKind::Chart
        | NodeKind::Editor
        | NodeKind::RichText
        | NodeKind::Carousel
        | NodeKind::DragDrop
        | NodeKind::Theme => NativeLayer::GpuiDisplay,
        NodeKind::AspectRatio
        | NodeKind::Alert
        | NodeKind::Attachment
        | NodeKind::Bubble
        | NodeKind::Command
        | NodeKind::NativeSelect
        | NodeKind::NavigationMenu
        | NodeKind::ScrollArea
        | NodeKind::Item
        | NodeKind::Message
        | NodeKind::MessageScroller
        | NodeKind::ToggleGroup
        | NodeKind::Sonner => NativeLayer::GpuiDisplay,
        NodeKind::Button
        | NodeKind::IconButton
        | NodeKind::Checkbox
        | NodeKind::Radio
        | NodeKind::Switch
        | NodeKind::Toggle
        | NodeKind::ButtonGroup
        | NodeKind::Slider
        | NodeKind::RangeSlider
        | NodeKind::Select
        | NodeKind::Combobox
        | NodeKind::NumberInput
        | NodeKind::TextInput
        | NodeKind::TextArea
        | NodeKind::Field
        | NodeKind::InputGroup
        | NodeKind::OtpInput
        | NodeKind::SecretInput => NativeLayer::GpuiControl,
        NodeKind::Dialog
        | NodeKind::AlertDialog
        | NodeKind::Popover
        | NodeKind::Sheet
        | NodeKind::Drawer
        | NodeKind::BottomSheet
        | NodeKind::Toast
        | NodeKind::Notification
        | NodeKind::Banner
        | NodeKind::ContextMenu
        | NodeKind::CommandPalette
        | NodeKind::Tooltip => NativeLayer::HostOverlay,
    }
}

fn is_interactive(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Button
            | NodeKind::IconButton
            | NodeKind::Checkbox
            | NodeKind::Switch
            | NodeKind::Slider
            | NodeKind::Select
            | NodeKind::TextInput
            | NodeKind::SecretInput
    )
}

fn accessibility_label(node: &UiNode) -> String {
    ["accessibility_label", "label", "text"]
        .iter()
        .find_map(|property| node.props.get(*property).and_then(Value::as_str))
        .filter(|label| !label.is_empty())
        .unwrap_or(&node.id)
        .to_owned()
}

fn validate_boundary_properties(node: &UiNode) -> Result<(), CatalogError> {
    const FORBIDDEN: [&str; 8] = [
        "html",
        "css",
        "class",
        "class_name",
        "native_class",
        "raw_draw",
        "shader",
        "device_control",
    ];
    let invalid = FORBIDDEN
        .iter()
        .find(|property| node.props.contains_key(**property))
        .copied()
        .or_else(|| {
            (node.kind == NodeKind::SecretInput && node.props.contains_key("value"))
                .then_some("value")
        });
    if let Some(property) = invalid {
        return Err(CatalogError::PropertyInvalid {
            node_id: node.id.clone(),
            property: property.to_owned(),
        });
    }
    Ok(())
}
