//! Closed protocol-v1 node property and child-cardinality contracts.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::{NodeKind, ProtocolError, ProtocolLimits, UiNode, validate_bounded_string};

/// Validate the complete semantic contract of one node, excluding descendants.
///
/// # Errors
///
/// Returns [`ProtocolError::InvalidNodeProperty`] for unknown names, invalid values, or invalid
/// cross-property state, and [`ProtocolError::InvalidChildCount`] for kind/cardinality mismatch.
pub fn validate_node_contract(node: &UiNode, limits: ProtocolLimits) -> Result<(), ProtocolError> {
    validate_child_count(node)?;
    for (property, value) in &node.props {
        validate_bounded_string(property, 128)?;
        if !validate_common(property, value, limits)?
            && !validate_specific(node.kind, property, value, limits)?
        {
            return invalid(node, property);
        }
    }
    validate_cross_properties(node)
}

fn validate_common(
    property: &str,
    value: &Value,
    limits: ProtocolLimits,
) -> Result<bool, ProtocolError> {
    match property {
        "visible" => Ok(value.is_boolean()),
        "opacity" => Ok(number_in(value, 0.0, 1.0)),
        "accessibility_label" => Ok(nonempty_string(value, limits)?),
        "transition" => Ok(valid_transition(value)),
        _ => Ok(false),
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::match_same_arms,
    reason = "the closed exhaustive catalog is clearest as one auditable kind dispatch"
)]
fn validate_specific(
    kind: NodeKind,
    property: &str,
    value: &Value,
    limits: ProtocolLimits,
) -> Result<bool, ProtocolError> {
    let valid = match kind {
        NodeKind::Box => match property {
            "padding" | "width" | "height" => nonnegative_number(value),
            "background" => semantic_color(value),
            "flex" => value
                .as_f64()
                .is_some_and(|value| value.is_finite() && (0.0..=10.0).contains(&value)),
            "shrink" => value.is_boolean(),
            _ => false,
        },
        NodeKind::Column | NodeKind::Row => match property {
            "gap" => nonnegative_number(value),
            "alignment" => alignment(value),
            "flex" => value
                .as_f64()
                .is_some_and(|value| value.is_finite() && (0.0..=10.0).contains(&value)),
            _ => false,
        },
        NodeKind::Stack => property == "alignment" && alignment(value),
        NodeKind::Grid => match property {
            "columns" => value
                .as_u64()
                .is_some_and(|value| (1..=64).contains(&value)),
            "gap" => nonnegative_number(value),
            _ => false,
        },
        NodeKind::ScrollView => property == "axis" && axis(value),
        NodeKind::ListView => match property {
            "axis" => axis(value),
            "gap" => nonnegative_number(value),
            _ => false,
        },
        NodeKind::Spacer => property == "size" && nonnegative_number(value),
        NodeKind::Divider => {
            property == "thickness" && value.as_f64().is_some_and(|value| value > 0.0)
        }
        NodeKind::Text => match property {
            "text" => string(value, limits)?,
            "typography_role" => matches_string(
                value,
                &["body", "label", "caption", "title", "headline", "display"],
            ),
            _ => false,
        },
        NodeKind::Icon => property == "name" && nonempty_string(value, limits)?,
        NodeKind::Image => match property {
            "asset" => asset_path(value),
            "alt" => nonempty_string(value, limits)?,
            "width" | "height" => nonnegative_number(value),
            _ => false,
        },
        NodeKind::Card => property == "padding" && nonnegative_number(value),
        NodeKind::Badge => property == "label" && nonempty_string(value, limits)?,
        NodeKind::Tag => match property {
            "label" => nonempty_string(value, limits)?,
            "variant" => matches_string(value, &["default", "success", "warning", "destructive"]),
            _ => false,
        },
        NodeKind::Avatar => match property {
            "asset" => asset_path(value),
            "alt" => nonempty_string(value, limits)?,
            "fallback" => nonempty_string(value, limits)?,
            _ => false,
        },
        NodeKind::Empty => match property {
            "title" | "description" => nonempty_string(value, limits)?,
            _ => false,
        },
        NodeKind::Skeleton => match property {
            "width" | "height" => nonnegative_number(value),
            _ => false,
        },
        NodeKind::ProgressIndicator => property == "value" && number_in(value, 0.0, 1.0),
        NodeKind::ProgressCircle => property == "value" && number_in(value, 0.0, 1.0),
        NodeKind::Spinner => property == "label" && nonempty_string(value, limits)?,
        NodeKind::Button => match property {
            "label" => nonempty_string(value, limits)?,
            "enabled" => value.is_boolean(),
            "on_pressed" => event_name(value),
            "variant" => matches_string(value, &["primary", "secondary", "selected"]),
            "width" => matches_string(value, &["intrinsic", "full"]),
            _ => false,
        },
        NodeKind::IconButton => match property {
            "icon" => nonempty_string(value, limits)?,
            "enabled" => value.is_boolean(),
            "on_pressed" => event_name(value),
            _ => false,
        },
        NodeKind::Checkbox | NodeKind::Radio | NodeKind::Switch | NodeKind::Toggle => {
            match property {
                "label" => nonempty_string(value, limits)?,
                "value" | "enabled" => value.is_boolean(),
                "on_changed" => event_name(value),
                _ => false,
            }
        }
        NodeKind::ButtonGroup => {
            property == "orientation" && matches_string(value, &["horizontal", "vertical"])
        }
        NodeKind::Slider => match property {
            "label" => nonempty_string(value, limits)?,
            "min" | "max" | "value" => value.as_f64().is_some_and(f64::is_finite),
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::RangeSlider => match property {
            "label" => nonempty_string(value, limits)?,
            "min" | "max" | "start" | "end" => value.as_f64().is_some_and(f64::is_finite),
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::Select => match property {
            "label" | "value" => nonempty_string(value, limits)?,
            "options" => string_options(value, limits)?,
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::Combobox => match property {
            "label" | "value" => nonempty_string(value, limits)?,
            "options" => string_options(value, limits)?,
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::NumberInput => match property {
            "label" => nonempty_string(value, limits)?,
            "value" | "min" | "max" | "step" => value.as_f64().is_some_and(f64::is_finite),
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::TextInput => match property {
            "label" => nonempty_string(value, limits)?,
            "value" | "placeholder" => string(value, limits)?,
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::TextArea => match property {
            "label" => nonempty_string(value, limits)?,
            "value" | "placeholder" => string(value, limits)?,
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::Field | NodeKind::InputGroup => match property {
            "label" | "description" | "error" => nonempty_string(value, limits)?,
            _ => false,
        },
        NodeKind::OtpInput => match property {
            "label" | "value" => string(value, limits)?,
            "length" => value.as_u64().is_some_and(|v| (1..=12).contains(&v)),
            "enabled" => value.is_boolean(),
            "on_changed" => event_name(value),
            _ => false,
        },
        NodeKind::SecretInput => match property {
            "label" => nonempty_string(value, limits)?,
            "ready" | "enabled" => value.is_boolean(),
            "on_ready" => event_name(value),
            _ => false,
        },
        NodeKind::Dialog => match property {
            "title" => nonempty_string(value, limits)?,
            "open" => value.is_boolean(),
            _ => false,
        },
        NodeKind::AlertDialog => match property {
            "title" | "message" => nonempty_string(value, limits)?,
            "open" => value.is_boolean(),
            _ => false,
        },
        NodeKind::Popover | NodeKind::Sheet => property == "open" && value.is_boolean(),
        NodeKind::BottomSheet => property == "open" && value.is_boolean(),
        NodeKind::Toast | NodeKind::Notification | NodeKind::Banner | NodeKind::ContextMenu => {
            property == "message" && nonempty_string(value, limits)?
        }
        NodeKind::CommandPalette => match property {
            "placeholder" => nonempty_string(value, limits)?,
            "open" => value.is_boolean(),
            "commands" => string_options(value, limits)?,
            _ => false,
        },
        NodeKind::Tooltip => property == "message" && nonempty_string(value, limits)?,
        NodeKind::Scaffold
        | NodeKind::AppBar
        | NodeKind::Sidebar
        | NodeKind::NavigationBar
        | NodeKind::NavigationRail
        | NodeKind::Drawer
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
        | NodeKind::TimePicker => match property {
            "label" | "title" | "value" | "route" => nonempty_string(value, limits)?,
            "selected" | "open" | "enabled" => value.is_boolean(),
            "items" | "options" | "columns" => string_options(value, limits)?,
            "page" | "pages" | "step" => value.as_u64().is_some(),
            "on_changed" | "on_select" | "on_navigate" => event_name(value),
            _ => false,
        },
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
        | NodeKind::Theme
        | NodeKind::AspectRatio
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
        | NodeKind::Sonner => match property {
            "label" | "title" | "content" | "variant" => string(value, limits)?,
            "value" => string(value, limits)? || value.as_f64().is_some_and(f64::is_finite),
            "open" | "enabled" | "interactive" => value.is_boolean(),
            "items" | "options" | "keys" | "series" => string_options(value, limits)?,
            "min" | "max" | "step" => value.as_f64().is_some_and(f64::is_finite),
            "on_changed" | "on_select" | "on_drop" => event_name(value),
            _ => false,
        },
    };
    Ok(valid)
}

fn validate_cross_properties(node: &UiNode) -> Result<(), ProtocolError> {
    if node.kind == NodeKind::Slider {
        let minimum = node.props.get("min").and_then(Value::as_f64);
        let maximum = node.props.get("max").and_then(Value::as_f64);
        let value = node.props.get("value").and_then(Value::as_f64);
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && (minimum >= maximum || value.is_some_and(|value| value < minimum || value > maximum))
        {
            return invalid(node, "value");
        }
    }
    if node.kind == NodeKind::Select
        && let (Some(options), Some(value)) = (
            node.props.get("options").and_then(Value::as_array),
            node.props.get("value").and_then(Value::as_str),
        )
        && !options.iter().any(|option| option.as_str() == Some(value))
    {
        return invalid(node, "value");
    }
    Ok(())
}

#[allow(clippy::too_many_lines, clippy::match_same_arms)]
fn validate_child_count(node: &UiNode) -> Result<(), ProtocolError> {
    let count = node.children.len();
    let valid = match node.kind {
        NodeKind::Box
        | NodeKind::Column
        | NodeKind::Row
        | NodeKind::Stack
        | NodeKind::Grid
        | NodeKind::ListView
        | NodeKind::ButtonGroup
        | NodeKind::Scaffold
        | NodeKind::AppBar
        | NodeKind::Sidebar
        | NodeKind::NavigationBar
        | NodeKind::NavigationRail
        | NodeKind::Tabs
        | NodeKind::Breadcrumb
        | NodeKind::Stepper
        | NodeKind::Pagination
        | NodeKind::SearchableList
        | NodeKind::VirtualList
        | NodeKind::DataTable
        | NodeKind::Tree
        | NodeKind::DescriptionList
        | NodeKind::Calendar
        | NodeKind::DatePicker
        | NodeKind::TimePicker
        | NodeKind::Accordion
        | NodeKind::Collapsible
        | NodeKind::HoverCard
        | NodeKind::MenuBar
        | NodeKind::StatusBar
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
        | NodeKind::ToggleGroup => true,
        NodeKind::Separator
        | NodeKind::Kbd
        | NodeKind::ColorPicker
        | NodeKind::Rating
        | NodeKind::Alert
        | NodeKind::Attachment
        | NodeKind::Command
        | NodeKind::NativeSelect
        | NodeKind::Message
        | NodeKind::Sonner
        | NodeKind::Spacer
        | NodeKind::Divider
        | NodeKind::Text
        | NodeKind::Icon
        | NodeKind::Image
        | NodeKind::Badge
        | NodeKind::Tag
        | NodeKind::Avatar
        | NodeKind::Empty
        | NodeKind::Skeleton
        | NodeKind::ProgressIndicator
        | NodeKind::ProgressCircle
        | NodeKind::Spinner
        | NodeKind::Button
        | NodeKind::IconButton
        | NodeKind::Checkbox
        | NodeKind::Radio
        | NodeKind::Switch
        | NodeKind::Toggle
        | NodeKind::Slider
        | NodeKind::RangeSlider
        | NodeKind::Select
        | NodeKind::Combobox
        | NodeKind::NumberInput
        | NodeKind::TextInput
        | NodeKind::TextArea
        | NodeKind::OtpInput
        | NodeKind::SecretInput
        | NodeKind::Toast
        | NodeKind::Notification
        | NodeKind::Banner => count == 0,
        NodeKind::ListTile
        | NodeKind::ScrollView
        | NodeKind::Card
        | NodeKind::Dialog
        | NodeKind::AlertDialog
        | NodeKind::Popover
        | NodeKind::Sheet
        | NodeKind::BottomSheet
        | NodeKind::Drawer
        | NodeKind::Field
        | NodeKind::InputGroup => count <= 1,
        NodeKind::Tooltip | NodeKind::ContextMenu | NodeKind::CommandPalette => count == 1,
    };
    if !valid {
        return Err(ProtocolError::InvalidChildCount {
            node_id: node.id.clone(),
            actual: count,
        });
    }
    Ok(())
}

fn invalid<T>(node: &UiNode, property: &str) -> Result<T, ProtocolError> {
    Err(ProtocolError::InvalidNodeProperty {
        node_id: node.id.clone(),
        property: property.to_owned(),
    })
}

fn string(value: &Value, limits: ProtocolLimits) -> Result<bool, ProtocolError> {
    let Some(value) = value.as_str() else {
        return Ok(false);
    };
    validate_bounded_string(value, limits.max_string_bytes)?;
    Ok(true)
}

fn nonempty_string(value: &Value, limits: ProtocolLimits) -> Result<bool, ProtocolError> {
    Ok(string(value, limits)? && value.as_str().is_some_and(|value| !value.is_empty()))
}

fn string_options(value: &Value, limits: ProtocolLimits) -> Result<bool, ProtocolError> {
    let Some(options) = value.as_array() else {
        return Ok(false);
    };
    if options.is_empty() || options.len() > 256 {
        return Ok(false);
    }
    let mut unique = BTreeSet::new();
    for option in options {
        if !nonempty_string(option, limits)? || !unique.insert(option.as_str().unwrap()) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn nonnegative_number(value: &Value) -> bool {
    value
        .as_f64()
        .is_some_and(|value| value.is_finite() && (0.0..=10_000.0).contains(&value))
}

fn number_in(value: &Value, minimum: f64, maximum: f64) -> bool {
    value
        .as_f64()
        .is_some_and(|value| value.is_finite() && (minimum..=maximum).contains(&value))
}

fn matches_string(value: &Value, allowed: &[&str]) -> bool {
    value.as_str().is_some_and(|value| allowed.contains(&value))
}

fn semantic_color(value: &Value) -> bool {
    matches_string(
        value,
        &[
            "surface",
            "surface_variant",
            "primary",
            "secondary",
            "error",
            "success",
            "warning",
            "transparent",
        ],
    )
}

fn alignment(value: &Value) -> bool {
    matches_string(
        value,
        &["start", "center", "end", "stretch", "space_between"],
    )
}

fn axis(value: &Value) -> bool {
    matches_string(value, &["horizontal", "vertical"])
}

fn event_name(value: &Value) -> bool {
    value.as_str().is_some_and(|value| {
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
            })
    })
}

fn asset_path(value: &Value) -> bool {
    value.as_str().is_some_and(|value| {
        value.starts_with("assets/")
            && !value.contains(['\\', '\0'])
            && value
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
    })
}

fn valid_transition(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    exact_keys(object, &["curve", "duration_ms"])
        && object
            .get("duration_ms")
            .and_then(Value::as_u64)
            .is_some_and(|duration| duration <= 60_000)
        && object.get("curve").is_some_and(|curve| {
            matches_string(curve, &["linear", "ease_in", "ease_out", "ease_in_out"])
        })
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> bool {
    object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
}
