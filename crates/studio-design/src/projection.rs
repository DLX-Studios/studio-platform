//! Deterministic projection of Studio Design source into protocol-v1 UI.
//!
//! The projection is deliberately host-independent.  It consumes an immutable
//! design revision and emits the same protocol tree for the same inputs.  The
//! source node identity is copied verbatim into [`studio_protocol::UiNode::id`]
//! so a renderer can preserve retained state and selection across revisions.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use studio_protocol::{
    GuestMessage, MountTree, ProtocolError, ProtocolLimits, UiNode, validate_guest_message,
};

use crate::model::{
    ColorValue, DesignNode, DesignNodeSource, LayoutProperties, Length, LengthUnit, LibraryAssetId,
    NodeId, Paint, PropertyValue, ResponsiveVariantId, RevisionId, StudioDesign,
    StudioDesignSnapshot, TokenValue,
};

/// Stable code used when a design construct is outside Runtime Projection v0.
pub const CODE_UNSUPPORTED: &str = "RUNTIME_PROJECTION_UNSUPPORTED";
/// Stable code used when a source node cannot be projected as a protocol node.
pub const CODE_NODE_INVALID: &str = "RUNTIME_PROJECTION_NODE_INVALID";
/// Stable code used when a source property cannot be represented on the wire.
pub const CODE_PROPERTY_INVALID: &str = "RUNTIME_PROJECTION_PROPERTY_INVALID";
/// Stable code used when a referenced Library asset is unavailable.
pub const CODE_ASSET_MISSING: &str = "RUNTIME_PROJECTION_ASSET_MISSING";
/// Stable code used when the generated tree fails the protocol contract.
pub const CODE_PROTOCOL_INVALID: &str = "RUNTIME_PROJECTION_PROTOCOL_INVALID";
/// Stable code used when a source screen cannot be selected.
pub const CODE_SCREEN_INVALID: &str = "RUNTIME_PROJECTION_SCREEN_INVALID";

/// An admitted Library asset reference used by the projection boundary.
///
/// Asset bytes intentionally do not cross this domain seam.  The runtime only
/// needs a stable, normalized package path to put in an `Image`/`Avatar`
/// property; the package/host layer owns loading and integrity checks.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryAsset {
    /// Stable source identity.
    pub id: LibraryAssetId,
    /// Normalized package path, normally under `assets/`.
    pub path: String,
}

/// Minimal immutable Library snapshot accepted by Runtime Projection v0.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LibrarySnapshot {
    /// Assets keyed by their source identity for deterministic lookup.
    pub assets: BTreeMap<LibraryAssetId, LibraryAsset>,
}

impl LibrarySnapshot {
    /// Construct a snapshot from an ordered asset map.
    #[must_use]
    pub fn new(assets: BTreeMap<LibraryAssetId, LibraryAsset>) -> Self {
        Self { assets }
    }

    /// Add or replace one asset in the snapshot.
    pub fn insert(&mut self, asset: LibraryAsset) {
        self.assets.insert(asset.id.clone(), asset);
    }

    /// Resolve an asset identity to its package path.
    #[must_use]
    pub fn path(&self, id: &LibraryAssetId) -> Option<&str> {
        self.assets.get(id).map(|asset| asset.path.as_str())
    }
}

/// Optional projection controls.  The default selects the first ordered
/// screen and applies only base values.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectionOptions {
    /// Screen to project; `None` selects `screen_order[0]`.
    pub screen_id: Option<crate::ScreenId>,
    /// Responsive variant to apply before converting properties.
    pub responsive_variant_id: Option<ResponsiveVariantId>,
    /// Protocol ceilings used for the final output validation.
    pub limits: ProtocolLimits,
}

/// A diagnostic tied to the source node/property that caused it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionDiagnostic {
    /// Stable machine-readable code.
    pub code: String,
    /// Safe developer-facing explanation.
    pub message: String,
    /// Source node, when the issue is node-local.
    pub node_id: Option<NodeId>,
    /// Source property, when the issue is property-local.
    pub property: Option<String>,
    /// Source revision that produced this diagnostic.
    pub revision_id: RevisionId,
}

impl ProjectionDiagnostic {
    fn new(
        code: &'static str,
        message: impl Into<String>,
        node_id: Option<NodeId>,
        property: Option<String>,
        revision_id: RevisionId,
    ) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            node_id,
            property,
            revision_id,
        }
    }
}

/// An ordered, diagnostic-rich projection attempt.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionReport {
    /// Immutable source revision used for this attempt.
    pub revision_id: RevisionId,
    /// Selected source screen, if one could be selected.
    pub screen_id: Option<crate::ScreenId>,
    /// Selected screen route, if one could be selected.
    pub route: Option<String>,
    /// Projected protocol root when no errors prevented final validation.
    pub root: Option<UiNode>,
    /// Diagnostics in deterministic source traversal order.
    pub diagnostics: Vec<ProjectionDiagnostic>,
}

impl ProjectionReport {
    /// Whether the report contains a protocol-valid root and no diagnostics.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.root.is_some() && self.diagnostics.is_empty()
    }

    /// Convert a valid report to the protocol mount envelope.
    #[must_use]
    pub fn mount_tree(&self) -> Option<MountTree> {
        Some(MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route: self.route.clone()?,
            root: self.root.clone()?,
        })
    }
}

/// Strict result of Runtime Projection v0.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProjection {
    /// Immutable source revision used for this projection.
    pub revision_id: RevisionId,
    /// Source screen identity.
    pub screen_id: crate::ScreenId,
    /// Runtime route copied from the source screen.
    pub route: String,
    /// Protocol-compatible retained root.
    pub root: UiNode,
    /// IDs visited while creating the tree, useful to prove source identity preservation.
    pub source_ids: BTreeSet<NodeId>,
}

/// A failed strict projection.  All failures retain the complete ordered diagnostics.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionError {
    /// Diagnostics collected during the failed attempt.
    pub diagnostics: Vec<ProjectionDiagnostic>,
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "runtime projection failed with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for ProjectionError {}

impl RuntimeProjection {
    /// Project one immutable revision with default options.
    pub fn project(
        snapshot: &StudioDesignSnapshot,
        library: Option<&LibrarySnapshot>,
    ) -> Result<Self, ProjectionError> {
        Self::project_with_options(snapshot, library, ProjectionOptions::default())
    }

    /// Project one immutable revision with explicit screen, variant, and protocol options.
    pub fn project_with_options(
        snapshot: &StudioDesignSnapshot,
        library: Option<&LibrarySnapshot>,
        options: ProjectionOptions,
    ) -> Result<Self, ProjectionError> {
        let report = project_report(snapshot, library, options);
        if !report.is_valid() {
            return Err(ProjectionError {
                diagnostics: report.diagnostics,
            });
        }
        Ok(Self {
            revision_id: report.revision_id,
            screen_id: report.screen_id.expect("valid projection has a screen"),
            route: report.route.expect("valid projection has a route"),
            root: report.root.clone().expect("valid projection has a root"),
            source_ids: collect_source_ids(report.root.as_ref().expect("root was checked")),
        })
    }

    /// Return the protocol mount envelope.
    #[must_use]
    pub fn mount_tree(&self) -> MountTree {
        MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route: self.route.clone(),
            root: self.root.clone(),
        }
    }
}

/// Run a diagnostic-preserving projection attempt without rejecting partial failures.
#[must_use]
pub fn project_report(
    snapshot: &StudioDesignSnapshot,
    library: Option<&LibrarySnapshot>,
    options: ProjectionOptions,
) -> ProjectionReport {
    let revision_id = snapshot.revision.id;
    let (screen_id, route) = match select_screen(&snapshot.design, options.screen_id.as_ref()) {
        Ok(screen) => (Some(screen.id.clone()), Some(screen.route.clone())),
        Err(diagnostic) => {
            return ProjectionReport {
                revision_id,
                screen_id: None,
                route: None,
                root: None,
                diagnostics: vec![diagnostic_with_revision(diagnostic, revision_id)],
            };
        }
    };
    let screen_id_ref = screen_id.as_ref().expect("selected screen exists");
    let screen = snapshot
        .design
        .screens
        .get(screen_id_ref)
        .expect("selection returned an existing screen");
    let mut diagnostics = Vec::new();
    validate_source_id_index(&snapshot.design, revision_id, &mut diagnostics);
    let mut source_ids = BTreeSet::new();
    let root = project_node(
        &snapshot.design,
        &screen.root_node_id,
        library,
        options.responsive_variant_id.as_ref(),
        revision_id,
        &mut diagnostics,
        &mut source_ids,
    );

    let root = root.filter(|root| {
        let mount = GuestMessage::Mount(MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route: route.clone().expect("selected screen has route"),
            root: root.clone(),
        });
        match validate_guest_message(&mount, options.limits) {
            Ok(()) => true,
            Err(error) => {
                diagnostics.push(protocol_diagnostic(error, revision_id));
                false
            }
        }
    });
    if !diagnostics.is_empty() {
        return ProjectionReport {
            revision_id,
            screen_id,
            route,
            root: None,
            diagnostics,
        };
    }
    ProjectionReport {
        revision_id,
        screen_id,
        route,
        root,
        diagnostics,
    }
}

/// Convenience strict function for callers that do not need the report shape.
pub fn project_runtime(
    snapshot: &StudioDesignSnapshot,
    library: Option<&LibrarySnapshot>,
) -> Result<RuntimeProjection, ProjectionError> {
    RuntimeProjection::project(snapshot, library)
}

fn select_screen<'a>(
    design: &'a StudioDesign,
    requested: Option<&crate::ScreenId>,
) -> Result<&'a crate::Screen, ProjectionDiagnostic> {
    let id = requested
        .or_else(|| design.screen_order.first())
        .ok_or_else(|| {
            ProjectionDiagnostic::new(
                CODE_SCREEN_INVALID,
                "the design has no ordered screen to project",
                None,
                None,
                RevisionId::INITIAL,
            )
        })?;
    design.screens.get(id).ok_or_else(|| {
        ProjectionDiagnostic::new(
            CODE_SCREEN_INVALID,
            format!("screen `{id}` is not present in the source screen map"),
            None,
            None,
            RevisionId::INITIAL,
        )
    })
}

fn diagnostic_with_revision(
    mut diagnostic: ProjectionDiagnostic,
    revision_id: RevisionId,
) -> ProjectionDiagnostic {
    diagnostic.revision_id = revision_id;
    diagnostic
}

fn validate_source_id_index(
    design: &StudioDesign,
    revision_id: RevisionId,
    diagnostics: &mut Vec<ProjectionDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for (map_id, node) in &design.nodes {
        if map_id != &node.id {
            diagnostics.push(ProjectionDiagnostic::new(
                CODE_NODE_INVALID,
                "source node map key does not match the node's declared identity",
                Some(node.id.clone()),
                None,
                revision_id,
            ));
        }
        if !seen.insert(node.id.clone()) {
            diagnostics.push(ProjectionDiagnostic::new(
                CODE_NODE_INVALID,
                "source node identity is duplicated in the source node map",
                Some(node.id.clone()),
                None,
                revision_id,
            ));
        }
    }
}

fn project_node(
    design: &StudioDesign,
    node_id: &NodeId,
    library: Option<&LibrarySnapshot>,
    responsive_variant_id: Option<&ResponsiveVariantId>,
    revision_id: RevisionId,
    diagnostics: &mut Vec<ProjectionDiagnostic>,
    source_ids: &mut BTreeSet<NodeId>,
) -> Option<UiNode> {
    let Some(node) = design.nodes.get(node_id) else {
        diagnostics.push(ProjectionDiagnostic::new(
            CODE_NODE_INVALID,
            "the screen root or child references an unknown source node",
            Some(node_id.clone()),
            None,
            revision_id,
        ));
        return None;
    };
    source_ids.insert(node.id.clone());
    let kind = match &node.source {
        DesignNodeSource::Primitive { kind } => *kind,
        DesignNodeSource::CompositionInstance { .. } => {
            diagnostics.push(ProjectionDiagnostic::new(
                CODE_UNSUPPORTED,
                "composition instances are not expanded by Runtime Projection v0",
                Some(node.id.clone()),
                None,
                revision_id,
            ));
            return None;
        }
    };
    let mut props = BTreeMap::new();
    for (property, value) in &node.properties {
        match property_value(design, node, property, value, library, revision_id) {
            Ok(value) => {
                props.insert(property.clone(), value);
            }
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    apply_layout(
        node,
        &node.layout,
        &mut props,
        revision_id,
        diagnostics,
        false,
    );
    apply_style(
        design,
        node,
        &node.style,
        &mut props,
        revision_id,
        diagnostics,
        false,
    );
    if let Some(variant_id) = responsive_variant_id
        && let Some(override_value) = node.responsive_overrides.get(variant_id)
    {
        for (property, value) in &override_value.properties {
            match property_value(design, node, property, value, library, revision_id) {
                Ok(value) => {
                    props.insert(property.clone(), value);
                }
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        apply_layout(
            node,
            &override_value.layout,
            &mut props,
            revision_id,
            diagnostics,
            true,
        );
        apply_style(
            design,
            node,
            &override_value.style,
            &mut props,
            revision_id,
            diagnostics,
            true,
        );
    }
    apply_accessibility(node, &mut props);

    let children = node
        .children
        .iter()
        .filter_map(|child_id| {
            let child = project_node(
                design,
                child_id,
                library,
                responsive_variant_id,
                revision_id,
                diagnostics,
                source_ids,
            );
            if child.is_none() {
                diagnostics.push(ProjectionDiagnostic::new(
                    CODE_NODE_INVALID,
                    "a source child could not be represented in the projected tree",
                    Some(node.id.clone()),
                    None,
                    revision_id,
                ));
            }
            child
        })
        .collect();
    Some(UiNode {
        id: node.id.as_str().to_owned(),
        kind,
        props,
        children,
    })
}

fn property_value(
    design: &StudioDesign,
    node: &DesignNode,
    property: &str,
    value: &PropertyValue,
    library: Option<&LibrarySnapshot>,
    revision_id: RevisionId,
) -> Result<Value, ProjectionDiagnostic> {
    match value {
        PropertyValue::String(value) => Ok(Value::String(value.clone())),
        PropertyValue::Boolean(value) => Ok(Value::Bool(*value)),
        PropertyValue::Integer(value) => Ok(Value::Number((*value).into())),
        PropertyValue::Decimal(value) => decimal_value(value).map(Value::Number).ok_or_else(|| {
            property_diagnostic(
                node,
                property,
                "decimal value is not a finite JSON number",
                revision_id,
            )
        }),
        PropertyValue::Length(value) => length_value(value).ok_or_else(|| {
            property_diagnostic(
                node,
                property,
                "only finite pixel lengths are supported by Runtime Projection v0",
                revision_id,
            )
        }),
        PropertyValue::Color(value) => color_value(value).ok_or_else(|| {
            property_diagnostic(
                node,
                property,
                "only approved semantic colors are supported by Runtime Projection v0",
                revision_id,
            )
        }),
        PropertyValue::Token(token_id) => {
            let Some(token) = design.tokens.get(token_id) else {
                return Err(property_diagnostic(
                    node,
                    property,
                    "property references an unknown design token",
                    revision_id,
                ));
            };
            match &token.value {
                TokenValue::Color(value) => color_value(value),
                TokenValue::Length(value) => length_value(value),
                TokenValue::Number(value) => decimal_value(value).map(Value::Number),
                TokenValue::String(value) => Some(Value::String(value.clone())),
                TokenValue::Typography(_) => None,
            }
            .ok_or_else(|| {
                property_diagnostic(
                    node,
                    property,
                    "design token value cannot be represented by protocol-v1",
                    revision_id,
                )
            })
        }
        PropertyValue::Asset(asset_id) => {
            let Some(path) = library.and_then(|library| library.path(asset_id)) else {
                return Err(ProjectionDiagnostic::new(
                    CODE_ASSET_MISSING,
                    "property references an asset absent from the supplied Library snapshot",
                    Some(node.id.clone()),
                    Some(property.to_owned()),
                    revision_id,
                ));
            };
            if valid_asset_path(path) {
                Ok(Value::String(path.to_owned()))
            } else {
                Err(property_diagnostic(
                    node,
                    property,
                    "Library asset path is not a safe assets/ path",
                    revision_id,
                ))
            }
        }
        PropertyValue::List(values) => values
            .iter()
            .map(|value| property_value(design, node, property, value, library, revision_id))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        PropertyValue::Binding(_) | PropertyValue::Node(_) => Err(ProjectionDiagnostic::new(
            CODE_UNSUPPORTED,
            "bindings and node references require guest state and are not static protocol properties",
            Some(node.id.clone()),
            Some(property.to_owned()),
            revision_id,
        )),
    }
}

fn apply_layout(
    node: &DesignNode,
    layout: &LayoutProperties,
    props: &mut BTreeMap<String, Value>,
    revision_id: RevisionId,
    diagnostics: &mut Vec<ProjectionDiagnostic>,
    override_values: bool,
) {
    if let Some(value) = &layout.padding {
        if let Some(value) = length_value(value) {
            if override_values {
                props.insert("padding".to_owned(), value);
            } else {
                props.entry("padding".to_owned()).or_insert(value);
            }
        } else {
            diagnostics.push(property_diagnostic(
                node,
                "padding",
                "only finite pixel padding is supported by Runtime Projection v0",
                revision_id,
            ));
        }
    }
    if let Some(value) = &layout.gap {
        if let Some(value) = length_value(value) {
            if override_values {
                props.insert("gap".to_owned(), value);
            } else {
                props.entry("gap".to_owned()).or_insert(value);
            }
        } else {
            diagnostics.push(property_diagnostic(
                node,
                "gap",
                "only finite pixel gap is supported by Runtime Projection v0",
                revision_id,
            ));
        }
    }
    if let Some(alignment) = layout.alignment {
        let value = Value::String(
            match alignment {
                crate::model::Alignment::Start => "start",
                crate::model::Alignment::Center => "center",
                crate::model::Alignment::End => "end",
                crate::model::Alignment::Stretch => "stretch",
                crate::model::Alignment::SpaceBetween => "space_between",
            }
            .to_owned(),
        );
        if override_values {
            props.insert("alignment".to_owned(), value);
        } else {
            props.entry("alignment".to_owned()).or_insert(value);
        }
    }
    if layout.width.is_some() || layout.height.is_some() || layout.placement.is_some() {
        diagnostics.push(ProjectionDiagnostic::new(
            CODE_UNSUPPORTED,
            "width, height, and placement are not represented by protocol-v1 Runtime Projection v0",
            Some(node.id.clone()),
            None,
            revision_id,
        ));
    }
}

fn apply_style(
    design: &StudioDesign,
    node: &DesignNode,
    style: &crate::StyleProperties,
    props: &mut BTreeMap<String, Value>,
    revision_id: RevisionId,
    diagnostics: &mut Vec<ProjectionDiagnostic>,
    override_values: bool,
) {
    if let Some(background) = style
        .background
        .as_ref()
        .and_then(|paint| paint_value(design, paint))
    {
        if override_values {
            props.insert("background".to_owned(), background);
        } else {
            props.entry("background".to_owned()).or_insert(background);
        }
    } else if style.background.is_some() {
        diagnostics.push(property_diagnostic(
            node,
            "background",
            "style background cannot be represented by protocol-v1",
            revision_id,
        ));
    }
    if style.opacity.is_some() {
        if let Some(opacity) = style.opacity.as_deref().and_then(decimal_value) {
            let valid = opacity
                .as_f64()
                .is_some_and(|value| (0.0..=1.0).contains(&value));
            if valid {
                if override_values {
                    props.insert("opacity".to_owned(), Value::Number(opacity));
                } else {
                    props
                        .entry("opacity".to_owned())
                        .or_insert(Value::Number(opacity));
                }
            } else {
                diagnostics.push(property_diagnostic(
                    node,
                    "opacity",
                    "opacity must be between 0 and 1",
                    revision_id,
                ));
            }
        } else {
            diagnostics.push(property_diagnostic(
                node,
                "opacity",
                "opacity is not a finite number",
                revision_id,
            ));
        }
    }
    if style.foreground.is_some()
        || style.corner_radius.is_some()
        || style.border_width.is_some()
        || style.border_color.is_some()
    {
        diagnostics.push(ProjectionDiagnostic::new(
            CODE_UNSUPPORTED,
            "foreground, corner radius, and border style require a later protocol catalog revision",
            Some(node.id.clone()),
            None,
            revision_id,
        ));
    }
}

fn apply_accessibility(node: &DesignNode, props: &mut BTreeMap<String, Value>) {
    if let Some(label) = node.accessibility.label.as_ref() {
        props
            .entry("accessibility_label".to_owned())
            .or_insert_with(|| Value::String(label.clone()));
    }
    if node.accessibility.hidden {
        props
            .entry("visible".to_owned())
            .or_insert(Value::Bool(false));
    }
}

fn paint_value(design: &StudioDesign, paint: &Paint) -> Option<Value> {
    match paint {
        Paint::Color(value) => color_value(value),
        Paint::Token(token_id) => {
            design
                .tokens
                .get(token_id)
                .and_then(|token| match &token.value {
                    TokenValue::Color(value) => color_value(value),
                    _ => None,
                })
        }
        Paint::None => None,
    }
}

fn color_value(value: &ColorValue) -> Option<Value> {
    let ColorValue::Semantic(value) = value else {
        return None;
    };
    matches!(
        value.as_str(),
        "surface"
            | "surface_variant"
            | "primary"
            | "secondary"
            | "error"
            | "success"
            | "warning"
            | "transparent"
    )
    .then(|| Value::String(value.clone()))
}

fn length_value(value: &Length) -> Option<Value> {
    (value.unit == LengthUnit::Pixels)
        .then(|| decimal_value(&value.value))
        .flatten()
        .map(Value::Number)
}

fn decimal_value(value: &str) -> Option<Number> {
    let number = value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())?;
    Number::from_f64(number)
}

fn valid_asset_path(path: &str) -> bool {
    path.starts_with("assets/")
        && !path.contains(['\\', '\0'])
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn property_diagnostic(
    node: &DesignNode,
    property: &str,
    message: &str,
    revision_id: RevisionId,
) -> ProjectionDiagnostic {
    ProjectionDiagnostic::new(
        CODE_PROPERTY_INVALID,
        message,
        Some(node.id.clone()),
        Some(property.to_owned()),
        revision_id,
    )
}

fn protocol_diagnostic(error: ProtocolError, revision_id: RevisionId) -> ProjectionDiagnostic {
    let (node_id, property) = match &error {
        ProtocolError::InvalidNodeProperty { node_id, property } => {
            (NodeId::new(node_id.clone()).ok(), Some(property.clone()))
        }
        ProtocolError::InvalidChildCount { node_id, .. }
        | ProtocolError::InvalidNodeId(node_id)
        | ProtocolError::DuplicateNodeId(node_id) => (NodeId::new(node_id.clone()).ok(), None),
        _ => (None, None),
    };
    ProjectionDiagnostic::new(
        CODE_PROTOCOL_INVALID,
        format!("projected tree failed protocol validation: {error}"),
        node_id,
        property,
        revision_id,
    )
}

fn collect_source_ids(root: &UiNode) -> BTreeSet<NodeId> {
    let mut ids = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Ok(id) = NodeId::new(node.id.clone()) {
            ids.insert(id);
        }
        stack.extend(node.children.iter());
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::{
        LibraryAsset, LibrarySnapshot, ProjectionOptions, RuntimeProjection, project_report,
    };
    use crate::{
        Actor, ActorId, ActorKind, DesignNode, OperationId, ProjectId, RevisionId,
        RevisionMetadata, RevisionReason, Screen, ScreenId, StudioDesign, StudioDesignSnapshot,
        UndoGroupId,
    };
    use std::collections::BTreeMap;
    use studio_protocol::NodeKind;

    fn snapshot(root: DesignNode) -> StudioDesignSnapshot {
        let project_id = ProjectId::new("projection-project").unwrap();
        let screen_id = ScreenId::new("home").unwrap();
        let mut design = StudioDesign::empty(project_id.clone(), "Projection");
        design.nodes.insert(root.id.clone(), root.clone());
        design.parents.insert(
            root.id.clone(),
            crate::NodeParent::Screen {
                screen_id: screen_id.clone(),
            },
        );
        design.screens.insert(
            screen_id.clone(),
            Screen {
                schema_version: crate::STUDIO_DESIGN_SCHEMA_VERSION,
                id: screen_id.clone(),
                name: "Home".to_owned(),
                route: "/home".to_owned(),
                root_node_id: root.id,
            },
        );
        design.screen_order.push(screen_id);
        StudioDesignSnapshot {
            revision: RevisionMetadata {
                id: RevisionId::new(4),
                parent_id: Some(RevisionId::new(3)),
                operation_id: OperationId::new("op").unwrap(),
                actor: Actor {
                    id: ActorId::new("designer").unwrap(),
                    kind: ActorKind::Human,
                    display_name: "Designer".to_owned(),
                },
                undo_group_id: UndoGroupId::new("group").unwrap(),
                undo_group_name: "Edit".to_owned(),
                reason: RevisionReason::Command,
            },
            design,
            tombstones: BTreeMap::new(),
        }
    }

    #[test]
    fn projection_is_deterministic_and_preserves_source_ids() {
        let mut root = DesignNode::primitive(
            crate::NodeId::new("root-box").unwrap(),
            "Root",
            NodeKind::Box,
        );
        let mut text = DesignNode::primitive(
            crate::NodeId::new("title-node").unwrap(),
            "Title",
            NodeKind::Text,
        );
        text.properties.insert(
            "text".to_owned(),
            crate::PropertyValue::String("Hello".to_owned()),
        );
        root.children.push(text.id.clone());
        let mut one = snapshot(root);
        let child = one
            .design
            .nodes
            .get(&crate::NodeId::new("title-node").unwrap());
        assert!(child.is_none(), "fixture child is inserted below");
        let root_id = crate::NodeId::new("root-box").unwrap();
        let child = DesignNode::primitive(
            crate::NodeId::new("title-node").unwrap(),
            "Title",
            NodeKind::Text,
        );
        one.design.nodes.get_mut(&root_id).unwrap().children = vec![child.id.clone()];
        one.design.nodes.insert(child.id.clone(), child);
        one.design.parents.insert(
            crate::NodeId::new("title-node").unwrap(),
            crate::NodeParent::Node {
                node_id: crate::NodeId::new("root-box").unwrap(),
            },
        );
        let first = RuntimeProjection::project(&one, None).unwrap();
        let second = RuntimeProjection::project(&one, None).unwrap();
        assert_eq!(first.root, second.root);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert_eq!(first.root.id, "root-box");
        assert_eq!(first.root.children[0].id, "title-node");
        assert_eq!(first.source_ids.len(), 2);
    }

    #[test]
    fn unsupported_asset_and_composition_are_source_linked() {
        let mut root =
            DesignNode::primitive(crate::NodeId::new("root").unwrap(), "Root", NodeKind::Image);
        root.properties.insert(
            "asset".to_owned(),
            crate::PropertyValue::Asset(crate::LibraryAssetId::new("missing").unwrap()),
        );
        let report = project_report(&snapshot(root), None, ProjectionOptions::default());
        assert_eq!(report.root, None);
        assert_eq!(report.diagnostics[0].code, super::CODE_ASSET_MISSING);
        assert_eq!(
            report.diagnostics[0].node_id.as_ref().unwrap().as_str(),
            "root"
        );
        let mut composition = DesignNode::primitive(
            crate::NodeId::new("composition").unwrap(),
            "Composition",
            NodeKind::Box,
        );
        composition.source = crate::DesignNodeSource::CompositionInstance {
            composition_id: crate::CompositionId::new("missing-composition").unwrap(),
            definition_version: 1,
            inputs: BTreeMap::new(),
            admitted_overrides: BTreeMap::new(),
        };
        let report = project_report(&snapshot(composition), None, ProjectionOptions::default());
        assert_eq!(report.diagnostics[0].code, super::CODE_UNSUPPORTED);
    }

    #[test]
    fn library_asset_path_is_emitted_without_asset_bytes() {
        let mut root = DesignNode::primitive(
            crate::NodeId::new("image").unwrap(),
            "Image",
            NodeKind::Image,
        );
        let asset_id = crate::LibraryAssetId::new("logo").unwrap();
        root.properties.insert(
            "asset".to_owned(),
            crate::PropertyValue::Asset(asset_id.clone()),
        );
        let mut library = LibrarySnapshot::default();
        library.insert(LibraryAsset {
            id: asset_id,
            path: "assets/logo.png".to_owned(),
        });
        let projection = RuntimeProjection::project(&snapshot(root), Some(&library)).unwrap();
        assert_eq!(projection.root.props["asset"], "assets/logo.png");
    }

    #[test]
    fn duplicate_source_ids_are_rejected_with_revision_diagnostics() {
        let root =
            DesignNode::primitive(crate::NodeId::new("root").unwrap(), "Root", NodeKind::Box);
        let mut source = snapshot(root);
        let map_key = crate::NodeId::new("other-key").unwrap();
        let mut duplicate = DesignNode::primitive(
            crate::NodeId::new("root").unwrap(),
            "Duplicate",
            NodeKind::Text,
        );
        duplicate.id = crate::NodeId::new("root").unwrap();
        source.design.nodes.insert(map_key, duplicate);
        let report = project_report(&source, None, ProjectionOptions::default());
        assert!(report.root.is_none());
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == super::CODE_NODE_INVALID
                && diagnostic
                    .node_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == "root")
                && diagnostic.revision_id == RevisionId::new(4)
        }));
    }
}
