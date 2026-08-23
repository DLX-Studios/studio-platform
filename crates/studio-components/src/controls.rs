//! First-class interactive controls admitted by the Studio native runtime.

use studio_protocol::NodeKind;

/// The four interactive controls with a stable runtime implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeControl {
    /// An activatable action control.
    Button,
    /// A text-editing control; secret input uses the host-owned variant.
    Input,
    /// A bounded option selector.
    Select,
    /// A bounded scalar/range control.
    Slider,
}

impl RuntimeControl {
    /// Resolve a protocol node kind to its first-class runtime control.
    #[must_use]
    pub const fn from_node_kind(kind: NodeKind) -> Option<Self> {
        match kind {
            NodeKind::Button
            | NodeKind::IconButton
            | NodeKind::Checkbox
            | NodeKind::Radio
            | NodeKind::Switch
            | NodeKind::Toggle
            | NodeKind::ButtonGroup => Some(Self::Button),
            NodeKind::TextInput
            | NodeKind::TextArea
            | NodeKind::Field
            | NodeKind::InputGroup
            | NodeKind::OtpInput
            | NodeKind::SecretInput => Some(Self::Input),
            NodeKind::Select | NodeKind::Combobox => Some(Self::Select),
            NodeKind::Slider | NodeKind::RangeSlider | NodeKind::NumberInput => Some(Self::Slider),
            _ => None,
        }
    }

    /// Whether the control owns sensitive value material in the host.
    #[must_use]
    pub const fn host_owned_value(self, kind: NodeKind) -> bool {
        matches!((self, kind), (Self::Input, NodeKind::SecretInput))
    }
}
