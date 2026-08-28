//! Deterministic responsive value resolution and device preview profiles.
#![allow(missing_docs, clippy::struct_excessive_bools)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::{
    DesignNode, InputEnvironment, LayoutProperties, Paint, PropertyValue, ResponsiveVariant,
    ResponsiveVariantId, StudioDesign, StyleProperties,
};
use crate::{DeviceProfileId, NodeId};

/// A typed base value with sparse breakpoint overrides.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsiveValue<T> {
    pub base: T,
    pub overrides: BTreeMap<ResponsiveVariantId, T>,
}

impl<T> ResponsiveValue<T> {
    /// Create a value with no breakpoint overrides.
    #[must_use]
    pub fn new(base: T) -> Self {
        Self {
            base,
            overrides: BTreeMap::new(),
        }
    }

    /// Set an override and return the previous value, if any.
    pub fn set_override(&mut self, variant: ResponsiveVariantId, value: T) -> Option<T> {
        self.overrides.insert(variant, value)
    }

    /// Remove an override and return it, if present.
    pub fn clear_override(&mut self, variant: &ResponsiveVariantId) -> Option<T> {
        self.overrides.remove(variant)
    }

    /// Resolve a value by the selected variant, falling back to the base.
    #[must_use]
    pub fn resolve(&self, variant: Option<&ResponsiveVariantId>) -> ResolvedValue<&T> {
        variant
            .and_then(|id| {
                self.overrides.get(id).map(|value| ResolvedValue {
                    value,
                    provenance: BreakpointProvenance::Breakpoint(id.clone()),
                })
            })
            .unwrap_or(ResolvedValue {
                value: &self.base,
                provenance: BreakpointProvenance::Base,
            })
    }
}

/// The source of an inspector value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BreakpointProvenance {
    Base,
    Breakpoint(ResponsiveVariantId),
}

/// A value paired with its source provenance.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedValue<T> {
    pub value: T,
    pub provenance: BreakpointProvenance,
}

/// Logical property path shown in the inspector and comparison report.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PropertyPath {
    Property(String),
    Layout(String),
    Style(String),
}

/// One resolved property and its breakpoint provenance.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyProvenance {
    pub node_id: NodeId,
    pub path: PropertyPath,
    pub value: Option<PropertyValue>,
    pub provenance: BreakpointProvenance,
}

/// Device viewport dimensions in logical pixels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// Device cutout/inset dimensions in logical pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Insets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

/// Orientation used by a profile preview.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Input capabilities and focus semantics for a device profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceInput {
    pub pointer: bool,
    pub touch: bool,
    pub keyboard: bool,
    pub remote_focus: bool,
}

/// One preview target, including all metadata that affects responsive layout.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceProfile {
    pub schema_version: u16,
    pub id: DeviceProfileId,
    pub name: String,
    pub viewport: Viewport,
    pub orientation: Orientation,
    pub safe_area: Insets,
    /// Pixel ratio is a decimal string to keep persisted values deterministic.
    pub pixel_ratio: String,
    pub input: DeviceInput,
}

impl DeviceProfile {
    /// Return the effective content viewport after safe-area insets.
    #[must_use]
    pub fn content_viewport(&self) -> Viewport {
        Viewport {
            width: self
                .viewport
                .width
                .saturating_sub(self.safe_area.left.saturating_add(self.safe_area.right)),
            height: self
                .viewport
                .height
                .saturating_sub(self.safe_area.top.saturating_add(self.safe_area.bottom)),
        }
    }

    fn input_environment(&self) -> InputEnvironment {
        if self.input.touch && !self.input.pointer {
            InputEnvironment::Touch
        } else if self.input.pointer {
            InputEnvironment::Pointer
        } else {
            InputEnvironment::Any
        }
    }
}

/// Ordered, deterministic device-profile catalog used by the Designer.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceProfileMatrix {
    pub profiles: BTreeMap<DeviceProfileId, DeviceProfile>,
}

impl DeviceProfileMatrix {
    /// Build the canonical phone-through-4K profile matrix.
    ///
    /// # Panics
    ///
    /// This cannot panic unless a static profile identity violates the identity contract.
    #[must_use]
    pub fn standard() -> Self {
        let mut matrix = Self::default();
        let definitions = [
            ("phone", "Phone", 390, 844, 3, true, false),
            ("foldable", "Foldable", 673, 841, 2, true, false),
            ("tablet", "Tablet", 834, 1194, 2, true, false),
            ("laptop", "Laptop", 1440, 900, 1, true, true),
            ("desktop", "Desktop", 1920, 1080, 1, true, true),
            ("ultrawide", "Ultrawide", 2560, 1080, 1, true, true),
            ("television", "Television", 3840, 2160, 1, false, false),
            ("4k", "4K", 3840, 2160, 1, true, true),
        ];
        for (slug, name, width, height, ratio, touch, pointer) in definitions {
            for (orientation, (profile_width, profile_height)) in [
                (
                    Orientation::Portrait,
                    (width.min(height), width.max(height)),
                ),
                (
                    Orientation::Landscape,
                    (width.max(height), width.min(height)),
                ),
            ] {
                let orientation_slug = match orientation {
                    Orientation::Portrait => "portrait",
                    Orientation::Landscape => "landscape",
                };
                let id = DeviceProfileId::new(format!("{slug}-{orientation_slug}"))
                    .expect("static profile identity is valid");
                matrix.profiles.insert(
                    id.clone(),
                    DeviceProfile {
                        schema_version: crate::STUDIO_DESIGN_SCHEMA_VERSION,
                        id,
                        name: format!("{name} {orientation_slug}"),
                        viewport: Viewport {
                            width: profile_width,
                            height: profile_height,
                        },
                        orientation,
                        safe_area: if touch {
                            Insets {
                                top: 24,
                                bottom: 16,
                                ..Insets::default()
                            }
                        } else {
                            Insets::default()
                        },
                        pixel_ratio: format!("{ratio}.0"),
                        input: DeviceInput {
                            pointer,
                            touch,
                            keyboard: pointer,
                            remote_focus: !pointer && !touch,
                        },
                    },
                );
            }
        }
        matrix
    }
}

/// A node with base-plus-breakpoint values resolved for a profile.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedNode {
    pub node_id: NodeId,
    pub variant: Option<ResponsiveVariantId>,
    pub properties: BTreeMap<String, PropertyProvenance>,
    pub layout: LayoutProperties,
    pub style: StyleProperties,
}

/// Resolve one node's sparse breakpoint override for a device profile.
#[must_use]
///
/// # Panics
///
/// This cannot panic for a valid design because a stored override is always keyed by its
/// selected variant.
pub fn resolve_node(
    design: &StudioDesign,
    node: &DesignNode,
    profile: &DeviceProfile,
) -> ResolvedNode {
    let variant = select_variant(design, profile);
    let override_value = variant
        .as_ref()
        .and_then(|id| node.responsive_overrides.get(id));
    let mut properties = BTreeMap::new();
    for (name, value) in &node.properties {
        properties.insert(
            name.clone(),
            PropertyProvenance {
                node_id: node.id.clone(),
                path: PropertyPath::Property(name.clone()),
                value: override_value
                    .and_then(|item| item.properties.get(name))
                    .cloned()
                    .or_else(|| Some(value.clone())),
                provenance: if override_value.is_some_and(|item| item.properties.contains_key(name))
                {
                    BreakpointProvenance::Breakpoint(variant.clone().expect("override has variant"))
                } else {
                    BreakpointProvenance::Base
                },
            },
        );
    }
    if let Some(item) = override_value {
        for (name, value) in &item.properties {
            properties
                .entry(name.clone())
                .or_insert_with(|| PropertyProvenance {
                    node_id: node.id.clone(),
                    path: PropertyPath::Property(name.clone()),
                    value: Some(value.clone()),
                    provenance: BreakpointProvenance::Breakpoint(
                        variant.clone().expect("override has variant"),
                    ),
                });
        }
    }
    let (layout, style) =
        override_value.map_or((node.layout.clone(), node.style.clone()), |item| {
            (
                merge_layout(&node.layout, &item.layout),
                merge_style(&node.style, &item.style),
            )
        });
    ResolvedNode {
        node_id: node.id.clone(),
        variant,
        properties,
        layout,
        style,
    }
}

/// Return the resolved inspector entries in deterministic path order.
#[must_use]
///
/// # Panics
///
/// This cannot panic for a valid design because a stored override is always keyed by its
/// selected variant.
pub fn inspect_node(
    design: &StudioDesign,
    node: &DesignNode,
    profile: &DeviceProfile,
) -> Vec<PropertyProvenance> {
    let resolved = resolve_node(design, node, profile);
    let variant = resolved.variant.clone();
    let override_value = variant
        .as_ref()
        .and_then(|id| node.responsive_overrides.get(id));
    let mut entries = resolved.properties.into_values().collect::<Vec<_>>();
    let mut add = |path: PropertyPath,
                   base: Option<PropertyValue>,
                   override_value: Option<PropertyValue>,
                   overridden: bool| {
        if base.is_some() || override_value.is_some() {
            entries.push(PropertyProvenance {
                node_id: node.id.clone(),
                path,
                value: override_value.clone().or(base),
                provenance: if overridden {
                    BreakpointProvenance::Breakpoint(variant.clone().expect("override has variant"))
                } else {
                    BreakpointProvenance::Base
                },
            });
        }
    };
    add_layout_length(
        &mut add,
        "width",
        &node.layout,
        override_value.map(|item| &item.layout),
    );
    add_layout_length(
        &mut add,
        "height",
        &node.layout,
        override_value.map(|item| &item.layout),
    );
    add_layout_length(
        &mut add,
        "gap",
        &node.layout,
        override_value.map(|item| &item.layout),
    );
    add_layout_length(
        &mut add,
        "padding",
        &node.layout,
        override_value.map(|item| &item.layout),
    );
    add_style_length(
        &mut add,
        "corner_radius",
        &node.style,
        override_value.map(|item| &item.style),
    );
    add_style_length(
        &mut add,
        "border_width",
        &node.style,
        override_value.map(|item| &item.style),
    );
    add_paint(
        &mut add,
        "background",
        &node.style,
        override_value.map(|item| &item.style),
        PropertyPath::Style,
    );
    add_paint(
        &mut add,
        "foreground",
        &node.style,
        override_value.map(|item| &item.style),
        PropertyPath::Style,
    );
    add_paint(
        &mut add,
        "border_color",
        &node.style,
        override_value.map(|item| &item.style),
        PropertyPath::Style,
    );
    add(
        PropertyPath::Style("opacity".to_owned()),
        node.style.opacity.clone().map(PropertyValue::String),
        override_value
            .and_then(|item| item.style.opacity.clone())
            .map(PropertyValue::String),
        override_value.is_some_and(|item| item.style.opacity.is_some()),
    );
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    entries
}

fn add_layout_length(
    add: &mut impl FnMut(PropertyPath, Option<PropertyValue>, Option<PropertyValue>, bool),
    name: &str,
    base: &LayoutProperties,
    override_value: Option<&LayoutProperties>,
) {
    let (base, override_value) = match name {
        "width" => (
            &base.width,
            override_value.and_then(|item| item.width.as_ref()),
        ),
        "height" => (
            &base.height,
            override_value.and_then(|item| item.height.as_ref()),
        ),
        "gap" => (&base.gap, override_value.and_then(|item| item.gap.as_ref())),
        "padding" => (
            &base.padding,
            override_value.and_then(|item| item.padding.as_ref()),
        ),
        _ => return,
    };
    add(
        PropertyPath::Layout(name.to_owned()),
        base.clone().map(PropertyValue::Length),
        override_value.cloned().map(PropertyValue::Length),
        override_value.is_some(),
    );
}

fn add_style_length(
    add: &mut impl FnMut(PropertyPath, Option<PropertyValue>, Option<PropertyValue>, bool),
    name: &str,
    base: &StyleProperties,
    override_value: Option<&StyleProperties>,
) {
    let (base, override_value) = match name {
        "corner_radius" => (
            &base.corner_radius,
            override_value.and_then(|item| item.corner_radius.as_ref()),
        ),
        "border_width" => (
            &base.border_width,
            override_value.and_then(|item| item.border_width.as_ref()),
        ),
        _ => return,
    };
    add(
        PropertyPath::Style(name.to_owned()),
        base.clone().map(PropertyValue::Length),
        override_value.cloned().map(PropertyValue::Length),
        override_value.is_some(),
    );
}

fn add_paint(
    add: &mut impl FnMut(PropertyPath, Option<PropertyValue>, Option<PropertyValue>, bool),
    name: &str,
    base: &StyleProperties,
    override_value: Option<&StyleProperties>,
    path: fn(String) -> PropertyPath,
) {
    let (base, override_value) = match name {
        "background" => (
            &base.background,
            override_value.and_then(|item| item.background.as_ref()),
        ),
        "foreground" => (
            &base.foreground,
            override_value.and_then(|item| item.foreground.as_ref()),
        ),
        "border_color" => (
            &base.border_color,
            override_value.and_then(|item| item.border_color.as_ref()),
        ),
        _ => return,
    };
    add(
        path(name.to_owned()),
        base.as_ref().and_then(paint_value),
        override_value.and_then(paint_value),
        override_value.is_some(),
    );
}

fn paint_value(paint: &Paint) -> Option<PropertyValue> {
    match paint {
        Paint::Color(color) => Some(PropertyValue::Color(color.clone())),
        Paint::Token(token) => Some(PropertyValue::Token(token.clone())),
        Paint::None => None,
    }
}

fn merge_layout(base: &LayoutProperties, override_value: &LayoutProperties) -> LayoutProperties {
    base.merged_with(override_value)
}

fn merge_style(base: &StyleProperties, override_value: &StyleProperties) -> StyleProperties {
    StyleProperties {
        schema_version: base.schema_version,
        background: override_value
            .background
            .clone()
            .or_else(|| base.background.clone()),
        foreground: override_value
            .foreground
            .clone()
            .or_else(|| base.foreground.clone()),
        opacity: override_value
            .opacity
            .clone()
            .or_else(|| base.opacity.clone()),
        corner_radius: override_value
            .corner_radius
            .clone()
            .or_else(|| base.corner_radius.clone()),
        border_width: override_value
            .border_width
            .clone()
            .or_else(|| base.border_width.clone()),
        border_color: override_value
            .border_color
            .clone()
            .or_else(|| base.border_color.clone()),
    }
}

/// Select the most-specific matching breakpoint deterministically.
#[must_use]
pub fn select_variant(
    design: &StudioDesign,
    profile: &DeviceProfile,
) -> Option<ResponsiveVariantId> {
    let width = profile.content_viewport().width;
    design
        .responsive_variants
        .values()
        .filter(|variant| variant_matches(variant, width, profile.input_environment()))
        .max_by(|left, right| {
            left.minimum_width
                .cmp(&right.minimum_width)
                .then_with(|| right.maximum_width.cmp(&left.maximum_width))
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|variant| variant.id.clone())
}

fn variant_matches(variant: &ResponsiveVariant, width: u32, input: InputEnvironment) -> bool {
    variant.minimum_width.is_none_or(|minimum| width >= minimum)
        && variant.maximum_width.is_none_or(|maximum| width <= maximum)
        && (variant.input == InputEnvironment::Any || variant.input == input)
}

/// One profile-to-profile difference for a selected node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileDifference {
    pub path: PropertyPath,
    pub left: Option<PropertyValue>,
    pub right: Option<PropertyValue>,
    pub left_provenance: BreakpointProvenance,
    pub right_provenance: BreakpointProvenance,
    pub unintended: bool,
}

/// Deterministic side-by-side comparison result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompareReport {
    pub node_id: NodeId,
    pub left_profile: DeviceProfileId,
    pub right_profile: DeviceProfileId,
    pub differences: Vec<ProfileDifference>,
}

/// Compare authored properties and report differences without mutating state.
#[must_use]
pub fn compare_profiles(
    design: &StudioDesign,
    node: &DesignNode,
    left: &DeviceProfile,
    right: &DeviceProfile,
) -> CompareReport {
    let mut differences = Vec::new();
    let left_entries = inspect_node(design, node, left)
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let right_entries = inspect_node(design, node, right)
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let paths = left_entries
        .keys()
        .chain(right_entries.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for path in paths {
        let left_value = left_entries.get(&path);
        let right_value = right_entries.get(&path);
        if left_value.map(|value| &value.value) == right_value.map(|value| &value.value) {
            continue;
        }
        differences.push(ProfileDifference {
            path,
            left: left_value.and_then(|value| value.value.clone()),
            right: right_value.and_then(|value| value.value.clone()),
            left_provenance: left_value
                .map_or(BreakpointProvenance::Base, |value| value.provenance.clone()),
            right_provenance: right_value
                .map_or(BreakpointProvenance::Base, |value| value.provenance.clone()),
            unintended: left_value
                .is_none_or(|value| matches!(value.provenance, BreakpointProvenance::Base))
                && right_value
                    .is_none_or(|value| matches!(value.provenance, BreakpointProvenance::Base)),
        });
    }
    differences.sort_by(|left, right| left.path.cmp(&right.path));
    CompareReport {
        node_id: node.id.clone(),
        left_profile: left.id.clone(),
        right_profile: right.id.clone(),
        differences,
    }
}
