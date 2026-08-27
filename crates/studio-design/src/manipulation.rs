//! Host-independent canvas manipulation primitives.
//!
//! This module deliberately stops at the Designer command seam.  It knows
//! about stable node identities and geometry, but it does not know about a
//! renderer, GPUI events, or a particular projection.  A canvas or hierarchy
//! presentation can therefore turn the same intent into one command batch.
#![allow(
    missing_docs,
    reason = "the manipulation algebra is documented as a whole"
)]
#![allow(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    reason = "canvas math is intentionally f64 and command builders own payloads"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    command::{Command, CommandBatch, CommandPrecondition, ParentPlacement},
    model::{
        Actor, NodeId, NodeParent, OperationId, ProjectId, PropertyValue, RevisionId,
        STUDIO_DESIGN_SCHEMA_VERSION, SelectionSnapshot, StudioDesign, StudioDesignSnapshot,
        UndoGroupId,
    },
};

/// Reserved source property used by the command engine for canvas geometry.
///
/// The command payload remains typed (`SetCanvasRect`).  Persisting the value
/// in the existing property map keeps snapshots from ticket 37 forward and
/// lets ticket 39's projection consume geometry without a parallel document.
pub const CANVAS_RECT_PROPERTY: &str = "__studio_canvas_rect";

/// A two-dimensional canvas point.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasPoint {
    pub x: f64,
    pub y: f64,
}

impl CanvasPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A non-negative canvas size.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasSize {
    pub width: f64,
    pub height: f64,
}

impl CanvasSize {
    #[must_use]
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }
}

/// A stable, axis-aligned canvas rectangle.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasRect {
    pub origin: CanvasPoint,
    pub size: CanvasSize,
}

impl CanvasRect {
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: CanvasPoint::new(x, y),
            size: CanvasSize::new(width, height),
        }
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.origin.x.is_finite() && self.origin.y.is_finite() && self.size.is_valid()
    }

    #[must_use]
    pub fn left(self) -> f64 {
        self.origin.x
    }

    #[must_use]
    pub fn right(self) -> f64 {
        self.origin.x + self.size.width
    }

    #[must_use]
    pub fn top(self) -> f64 {
        self.origin.y
    }

    #[must_use]
    pub fn bottom(self) -> f64 {
        self.origin.y + self.size.height
    }

    #[must_use]
    pub fn center_x(self) -> f64 {
        self.left() + self.size.width / 2.0
    }

    #[must_use]
    pub fn center_y(self) -> f64 {
        self.top() + self.size.height / 2.0
    }

    #[must_use]
    pub fn center(self) -> CanvasPoint {
        CanvasPoint::new(self.center_x(), self.center_y())
    }

    #[must_use]
    pub fn contains(self, point: CanvasPoint, tolerance: f64) -> bool {
        let tolerance = tolerance.max(0.0);
        point.x >= self.left() - tolerance
            && point.x <= self.right() + tolerance
            && point.y >= self.top() - tolerance
            && point.y <= self.bottom() + tolerance
    }

    #[must_use]
    pub fn translated(self, delta: CanvasPoint) -> Self {
        Self::new(
            self.left() + delta.x,
            self.top() + delta.y,
            self.size.width,
            self.size.height,
        )
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(left, top, right - left, bottom - top)
    }

    /// Encode the typed rectangle in the stable property representation.
    #[must_use]
    pub fn to_property_value(self) -> Option<PropertyValue> {
        self.is_valid().then(|| {
            PropertyValue::List(
                [self.left(), self.top(), self.size.width, self.size.height]
                    .into_iter()
                    .map(|value| PropertyValue::Decimal(canonical_decimal(value)))
                    .collect(),
            )
        })
    }

    /// Decode the reserved property representation, rejecting malformed data.
    #[must_use]
    pub fn from_property_value(value: &PropertyValue) -> Option<Self> {
        let PropertyValue::List(values) = value else {
            return None;
        };
        if values.len() != 4 {
            return None;
        }
        let values = values
            .iter()
            .map(|value| match value {
                PropertyValue::Decimal(value) => value.parse::<f64>().ok(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let rect = Self::new(values[0], values[1], values[2], values[3]);
        rect.is_valid().then_some(rect)
    }
}

fn canonical_decimal(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        value.to_string()
    }
}

/// Geometry and deterministic paint order used by hit testing and gestures.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CanvasGeometry {
    pub frames: BTreeMap<NodeId, CanvasRect>,
    /// Earlier entries paint first; later entries win ties in hit testing.
    pub paint_order: Vec<NodeId>,
}

impl CanvasGeometry {
    #[must_use]
    pub fn from_design(design: &StudioDesign) -> Self {
        let frames = design
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                node.properties
                    .get(CANVAS_RECT_PROPERTY)
                    .and_then(CanvasRect::from_property_value)
                    .map(|rect| (node_id.clone(), rect))
            })
            .collect();
        let paint_order = paint_order(design);
        Self {
            frames,
            paint_order,
        }
    }

    pub fn set_frame(
        &mut self,
        node_id: NodeId,
        frame: CanvasRect,
    ) -> Result<(), ManipulationError> {
        if !frame.is_valid() {
            return Err(ManipulationError::InvalidGeometry(node_id));
        }
        if !self.paint_order.contains(&node_id) {
            self.paint_order.push(node_id.clone());
        }
        self.frames.insert(node_id, frame);
        Ok(())
    }

    #[must_use]
    pub fn frame(&self, node_id: &NodeId) -> Option<CanvasRect> {
        self.frames.get(node_id).copied()
    }

    #[must_use]
    pub fn hit_test(&self, point: CanvasPoint, tolerance: f64) -> Option<NodeId> {
        self.hit_test_index().hit_test(point, tolerance)
    }

    #[must_use]
    pub fn hit_test_index(&self) -> HitTestIndex {
        let mut entries = Vec::with_capacity(self.frames.len());
        for (paint_index, node_id) in self.paint_order.iter().enumerate() {
            let Some(rect) = self.frames.get(node_id).copied() else {
                continue;
            };
            entries.push(HitTestEntry {
                node_id: node_id.clone(),
                rect,
                depth: 0,
                paint_index,
            });
        }
        HitTestIndex { entries }
    }
}

fn paint_order(design: &StudioDesign) -> Vec<NodeId> {
    fn visit(design: &StudioDesign, node_id: &NodeId, output: &mut Vec<NodeId>, depth: usize) {
        let Some(node) = design.nodes.get(node_id) else {
            return;
        };
        if output.iter().any(|candidate| candidate == node_id) {
            return;
        }
        output.push(node_id.clone());
        for child in &node.children {
            visit(design, child, output, depth.saturating_add(1));
        }
        let _ = depth;
    }
    let mut output = Vec::new();
    for screen_id in &design.screen_order {
        if let Some(screen) = design.screens.get(screen_id) {
            visit(design, &screen.root_node_id, &mut output, 0);
        }
    }
    for node_id in design.nodes.keys() {
        if !output.contains(node_id) {
            visit(design, node_id, &mut output, 0);
        }
    }
    output
}

/// One candidate in a stable-ID hit-test index.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HitTestEntry {
    pub node_id: NodeId,
    pub rect: CanvasRect,
    pub depth: usize,
    pub paint_index: usize,
}

/// Deterministic hit-test index. Children or later-painted entries win ties.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct HitTestIndex {
    pub entries: Vec<HitTestEntry>,
}

impl HitTestIndex {
    #[must_use]
    pub fn hit_test(&self, point: CanvasPoint, tolerance: f64) -> Option<NodeId> {
        self.entries
            .iter()
            .filter(|entry| entry.rect.contains(point, tolerance))
            .max_by(|left, right| {
                left.depth
                    .cmp(&right.depth)
                    .then(left.paint_index.cmp(&right.paint_index))
                    .then(left.node_id.cmp(&right.node_id))
            })
            .map(|entry| entry.node_id.clone())
    }
}

/// A pointer location axis.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GuideAxis {
    Horizontal,
    Vertical,
}

/// Why a guide was selected.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GuideKind {
    Grid,
    Edge,
    Center,
}

/// A visible snapping guide.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SnapGuide {
    pub axis: GuideAxis,
    pub position: f64,
    pub kind: GuideKind,
    pub source: Option<NodeId>,
}

/// User-configurable snapping tolerance and guide sources.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct SnapConfig {
    pub tolerance: f64,
    pub grid_size: Option<f64>,
    pub snap_to_edges: bool,
    pub snap_to_centers: bool,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            tolerance: 4.0,
            grid_size: None,
            snap_to_edges: true,
            snap_to_centers: true,
        }
    }
}

/// The result of applying snapping to one rectangle.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SnapResult {
    pub rect: CanvasRect,
    pub delta: CanvasPoint,
    pub guides: Vec<SnapGuide>,
}

impl CanvasGeometry {
    #[must_use]
    pub fn snap_rect(
        &self,
        rect: CanvasRect,
        moving_ids: &BTreeSet<NodeId>,
        config: SnapConfig,
    ) -> SnapResult {
        let tolerance = config.tolerance.max(0.0);
        let mut x_candidates = Vec::new();
        let mut y_candidates = Vec::new();
        if let Some(grid_size) = config
            .grid_size
            .filter(|size| size.is_finite() && *size > 0.0)
        {
            for value in [rect.left(), rect.center_x(), rect.right()] {
                x_candidates.push((round_to_grid(value, grid_size), GuideKind::Grid, None));
            }
            for value in [rect.top(), rect.center_y(), rect.bottom()] {
                y_candidates.push((round_to_grid(value, grid_size), GuideKind::Grid, None));
            }
        }
        for (node_id, other) in &self.frames {
            if moving_ids.contains(node_id) {
                continue;
            }
            if config.snap_to_edges {
                x_candidates.extend([
                    (other.left(), GuideKind::Edge, Some(node_id.clone())),
                    (other.right(), GuideKind::Edge, Some(node_id.clone())),
                ]);
                y_candidates.extend([
                    (other.top(), GuideKind::Edge, Some(node_id.clone())),
                    (other.bottom(), GuideKind::Edge, Some(node_id.clone())),
                ]);
            }
            if config.snap_to_centers {
                x_candidates.push((other.center_x(), GuideKind::Center, Some(node_id.clone())));
                y_candidates.push((other.center_y(), GuideKind::Center, Some(node_id.clone())));
            }
        }
        let x = best_snap(
            [
                (rect.left(), GuideKind::Edge),
                (rect.center_x(), GuideKind::Center),
                (rect.right(), GuideKind::Edge),
            ],
            x_candidates,
            tolerance,
            GuideAxis::Vertical,
        );
        let y = best_snap(
            [
                (rect.top(), GuideKind::Edge),
                (rect.center_y(), GuideKind::Center),
                (rect.bottom(), GuideKind::Edge),
            ],
            y_candidates,
            tolerance,
            GuideAxis::Horizontal,
        );
        let delta = CanvasPoint::new(
            x.as_ref().map_or(0.0, |snap| snap.0),
            y.as_ref().map_or(0.0, |snap| snap.0),
        );
        let mut guides = Vec::new();
        if let Some((_, guide)) = x {
            guides.push(guide);
        }
        if let Some((_, guide)) = y {
            guides.push(guide);
        }
        SnapResult {
            rect: rect.translated(delta),
            delta,
            guides,
        }
    }
}

fn round_to_grid(value: f64, grid: f64) -> f64 {
    (value / grid).round() * grid
}

fn best_snap<const N: usize>(
    anchors: [(f64, GuideKind); N],
    candidates: Vec<(f64, GuideKind, Option<NodeId>)>,
    tolerance: f64,
    axis: GuideAxis,
) -> Option<(f64, SnapGuide)> {
    let mut best: Option<(f64, f64, u8, SnapGuide)> = None;
    for (anchor, anchor_kind) in anchors {
        for (candidate, kind, source) in &candidates {
            let delta = *candidate - anchor;
            let distance = delta.abs();
            if distance > tolerance {
                continue;
            }
            let guide = SnapGuide {
                axis,
                position: *candidate,
                kind: *kind,
                source: source.clone(),
            };
            let priority = match (*kind, anchor_kind) {
                (GuideKind::Center, GuideKind::Center) => 0,
                (GuideKind::Edge, GuideKind::Edge) => 1,
                (GuideKind::Grid, _) => 2,
                _ => 3,
            };
            if best
                .as_ref()
                .is_none_or(|(_, best_distance, best_priority, best_guide)| {
                    distance < *best_distance
                        || (distance == *best_distance
                            && (priority, guide.position, &guide.source)
                                < (*best_priority, best_guide.position, &best_guide.source))
                })
            {
                best = Some((delta, distance, priority, guide));
            }
        }
    }
    best.map(|(delta, _, _, guide)| (delta, guide))
}

/// Which edge or center should be aligned.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CanvasAlignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

/// Which axis should be distributed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CanvasDistribution {
    Horizontal,
    Vertical,
}

fn selected_frames(
    geometry: &CanvasGeometry,
    selection: &[NodeId],
) -> Result<BTreeMap<NodeId, CanvasRect>, ManipulationError> {
    if selection.is_empty() {
        return Err(ManipulationError::EmptySelection);
    }
    selection
        .iter()
        .map(|node_id| {
            geometry
                .frame(node_id)
                .map(|frame| (node_id.clone(), frame))
                .ok_or_else(|| ManipulationError::MissingFrame(node_id.clone()))
        })
        .collect()
}

pub fn alignment_targets(
    geometry: &CanvasGeometry,
    selection: &[NodeId],
    alignment: CanvasAlignment,
) -> Result<BTreeMap<NodeId, CanvasRect>, ManipulationError> {
    let frames = selected_frames(geometry, selection)?;
    let bounds = frames
        .values()
        .copied()
        .reduce(CanvasRect::union)
        .expect("selected_frames rejects empty selections");
    Ok(frames
        .into_iter()
        .map(|(node_id, frame)| {
            let (x, y) = match alignment {
                CanvasAlignment::Left => (bounds.left(), frame.top()),
                CanvasAlignment::HorizontalCenter => {
                    (bounds.center_x() - frame.size.width / 2.0, frame.top())
                }
                CanvasAlignment::Right => (bounds.right() - frame.size.width, frame.top()),
                CanvasAlignment::Top => (frame.left(), bounds.top()),
                CanvasAlignment::VerticalCenter => {
                    (frame.left(), bounds.center_y() - frame.size.height / 2.0)
                }
                CanvasAlignment::Bottom => (frame.left(), bounds.bottom() - frame.size.height),
            };
            (
                node_id,
                CanvasRect::new(x, y, frame.size.width, frame.size.height),
            )
        })
        .collect())
}

pub fn distribution_targets(
    geometry: &CanvasGeometry,
    selection: &[NodeId],
    distribution: CanvasDistribution,
) -> Result<BTreeMap<NodeId, CanvasRect>, ManipulationError> {
    let frames = selected_frames(geometry, selection)?;
    if frames.len() < 3 {
        return Ok(frames);
    }
    let mut ordered = frames
        .iter()
        .map(|(id, frame)| (id.clone(), *frame))
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        let left_position = match distribution {
            CanvasDistribution::Horizontal => left.1.left(),
            CanvasDistribution::Vertical => left.1.top(),
        };
        let right_position = match distribution {
            CanvasDistribution::Horizontal => right.1.left(),
            CanvasDistribution::Vertical => right.1.top(),
        };
        left_position
            .total_cmp(&right_position)
            .then(left.0.cmp(&right.0))
    });
    let first = ordered.first().expect("at least three frames").1;
    let last = ordered.last().expect("at least three frames").1;
    let total_size = ordered
        .iter()
        .map(|(_, frame)| match distribution {
            CanvasDistribution::Horizontal => frame.size.width,
            CanvasDistribution::Vertical => frame.size.height,
        })
        .sum::<f64>();
    let span = match distribution {
        CanvasDistribution::Horizontal => last.right() - first.left(),
        CanvasDistribution::Vertical => last.bottom() - first.top(),
    };
    let gap = (span - total_size) / (ordered.len() - 1) as f64;
    let mut cursor = match distribution {
        CanvasDistribution::Horizontal => first.left(),
        CanvasDistribution::Vertical => first.top(),
    };
    let mut output = BTreeMap::new();
    for (node_id, frame) in ordered {
        let (x, y) = match distribution {
            CanvasDistribution::Horizontal => (cursor, frame.top()),
            CanvasDistribution::Vertical => (frame.left(), cursor),
        };
        output.insert(
            node_id,
            CanvasRect::new(x, y, frame.size.width, frame.size.height),
        );
        cursor += match distribution {
            CanvasDistribution::Horizontal => frame.size.width,
            CanvasDistribution::Vertical => frame.size.height,
        } + gap;
    }
    Ok(output)
}

/// Resize direction. Corner and edge handles are all supported.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

#[must_use]
pub fn resize_rect(
    rect: CanvasRect,
    handle: ResizeHandle,
    delta: CanvasPoint,
    minimum_size: CanvasSize,
) -> CanvasRect {
    let min_width = minimum_size.width.max(0.0);
    let min_height = minimum_size.height.max(0.0);
    let move_left = matches!(
        handle,
        ResizeHandle::NorthWest | ResizeHandle::West | ResizeHandle::SouthWest
    );
    let move_right = matches!(
        handle,
        ResizeHandle::NorthEast | ResizeHandle::East | ResizeHandle::SouthEast
    );
    let move_top = matches!(
        handle,
        ResizeHandle::NorthWest | ResizeHandle::North | ResizeHandle::NorthEast
    );
    let move_bottom = matches!(
        handle,
        ResizeHandle::SouthWest | ResizeHandle::South | ResizeHandle::SouthEast
    );
    let left = if move_left {
        rect.left() + delta.x
    } else {
        rect.left()
    };
    let right = if move_right {
        rect.right() + delta.x
    } else {
        rect.right()
    };
    let top = if move_top {
        rect.top() + delta.y
    } else {
        rect.top()
    };
    let bottom = if move_bottom {
        rect.bottom() + delta.y
    } else {
        rect.bottom()
    };
    let (left, right) = if right - left < min_width {
        if move_left {
            (right - min_width, right)
        } else {
            (left, left + min_width)
        }
    } else {
        (left, right)
    };
    let (top, bottom) = if bottom - top < min_height {
        if move_top {
            (bottom - min_height, bottom)
        } else {
            (top, top + min_height)
        }
    } else {
        (top, bottom)
    };
    CanvasRect::new(left, top, right - left, bottom - top)
}

/// Metadata shared by one gesture's command batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GestureContext {
    pub operation_id: OperationId,
    pub actor: Actor,
    pub project_id: ProjectId,
    pub base_revision: RevisionId,
    pub undo_group_id: UndoGroupId,
}

impl GestureContext {
    #[must_use]
    pub fn new(
        operation_id: OperationId,
        actor: Actor,
        project_id: ProjectId,
        base_revision: RevisionId,
        undo_group_id: UndoGroupId,
    ) -> Self {
        Self {
            operation_id,
            actor,
            project_id,
            base_revision,
            undo_group_id,
        }
    }

    fn batch(
        &self,
        name: &str,
        commands: Vec<Command>,
        preconditions: Vec<CommandPrecondition>,
    ) -> CommandBatch {
        CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id: self.operation_id.clone(),
            actor: self.actor.clone(),
            project_id: self.project_id.clone(),
            base_revision: self.base_revision,
            undo_group_id: self.undo_group_id.clone(),
            undo_group_name: name.to_owned(),
            preconditions,
            commands,
        }
    }
}

/// Errors detected while constructing a manipulation intent.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ManipulationError {
    #[error("selection is empty")]
    EmptySelection,
    #[error("node {0} is missing from the design")]
    MissingNode(NodeId),
    #[error("node {0} has no canvas frame")]
    MissingFrame(NodeId),
    #[error("node {0} has invalid canvas geometry")]
    InvalidGeometry(NodeId),
    #[error("node {0} cannot be manipulated because it is a root")]
    RootNode(NodeId),
    #[error("destination parent is invalid")]
    InvalidDestination,
    #[error("no deletion tombstone exists for node {0}")]
    MissingTombstone(NodeId),
    #[error("duplicate identity map is missing or collides")]
    InvalidIdentityMap,
}

fn existing_preconditions(ids: impl IntoIterator<Item = NodeId>) -> Vec<CommandPrecondition> {
    ids.into_iter()
        .map(|node_id| CommandPrecondition::NodeExists { node_id })
        .collect()
}

fn set_frame_commands(targets: BTreeMap<NodeId, CanvasRect>) -> Vec<Command> {
    targets
        .into_iter()
        .map(|(node_id, rect)| Command::SetCanvasRect { node_id, rect })
        .collect()
}

fn selection_set(selection: &[NodeId]) -> Vec<NodeId> {
    let mut seen = BTreeSet::new();
    selection
        .iter()
        .filter(|node_id| seen.insert((*node_id).clone()))
        .cloned()
        .collect()
}

/// Build one named undo group for a drag/move gesture.
pub fn drag_batch(
    context: &GestureContext,
    geometry: &CanvasGeometry,
    selection: &[NodeId],
    delta: CanvasPoint,
    snap: SnapConfig,
) -> Result<CommandBatch, ManipulationError> {
    let selection = selection_set(selection);
    let frames = selected_frames(geometry, &selection)?;
    let bounds = frames
        .values()
        .copied()
        .reduce(CanvasRect::union)
        .expect("non-empty selection");
    let moving = selection.iter().cloned().collect::<BTreeSet<_>>();
    let snapped = geometry.snap_rect(bounds.translated(delta), &moving, snap);
    let total_delta = CanvasPoint::new(delta.x + snapped.delta.x, delta.y + snapped.delta.y);
    let targets = frames
        .into_iter()
        .map(|(id, frame)| (id, frame.translated(total_delta)))
        .collect();
    Ok(context.batch(
        "Move selection",
        set_frame_commands(targets),
        existing_preconditions(selection),
    ))
}

/// Build a keyboard nudge using the same move and snapping semantics as drag.
pub fn nudge_batch(
    context: &GestureContext,
    geometry: &CanvasGeometry,
    selection: &[NodeId],
    direction: CanvasPoint,
    step: f64,
    snap: SnapConfig,
) -> Result<CommandBatch, ManipulationError> {
    drag_batch(
        context,
        geometry,
        selection,
        CanvasPoint::new(direction.x * step, direction.y * step),
        snap,
    )
}

/// Build one named undo group for a resize gesture.
pub fn resize_batch(
    context: &GestureContext,
    geometry: &CanvasGeometry,
    node_id: NodeId,
    handle: ResizeHandle,
    delta: CanvasPoint,
    minimum_size: CanvasSize,
    snap: SnapConfig,
) -> Result<CommandBatch, ManipulationError> {
    let frame = geometry
        .frame(&node_id)
        .ok_or_else(|| ManipulationError::MissingFrame(node_id.clone()))?;
    let moving = BTreeSet::from([node_id.clone()]);
    let resized = resize_rect(frame, handle, delta, minimum_size);
    let snapped = geometry.snap_rect(resized, &moving, snap);
    let target = snapped.rect;
    Ok(context.batch(
        "Resize selection",
        vec![Command::SetCanvasRect {
            node_id: node_id.clone(),
            rect: target,
        }],
        vec![CommandPrecondition::NodeExists { node_id }],
    ))
}

/// Build a keyboard resize using the pointer resize operation.
pub fn keyboard_resize_batch(
    context: &GestureContext,
    geometry: &CanvasGeometry,
    node_id: NodeId,
    handle: ResizeHandle,
    step: CanvasPoint,
    minimum_size: CanvasSize,
    snap: SnapConfig,
) -> Result<CommandBatch, ManipulationError> {
    resize_batch(context, geometry, node_id, handle, step, minimum_size, snap)
}

/// Build a reorder operation shared by canvas and hierarchy drag/drop.
pub fn reorder_batch(
    context: &GestureContext,
    design: &StudioDesign,
    node_id: NodeId,
    index: usize,
) -> Result<CommandBatch, ManipulationError> {
    let parent = design
        .parents
        .get(&node_id)
        .cloned()
        .ok_or_else(|| ManipulationError::MissingNode(node_id.clone()))?;
    let NodeParent::Node { .. } = parent.clone() else {
        return Err(ManipulationError::RootNode(node_id));
    };
    let old_index = child_index(design, &node_id)
        .ok_or_else(|| ManipulationError::MissingNode(node_id.clone()))?;
    Ok(context.batch(
        "Reorder layer",
        vec![Command::ReorderNode {
            node_id: node_id.clone(),
            index,
        }],
        vec![
            CommandPrecondition::NodeExists {
                node_id: node_id.clone(),
            },
            CommandPrecondition::ParentEquals {
                node_id: node_id.clone(),
                parent,
            },
            CommandPrecondition::ChildIndexEquals {
                node_id,
                index: old_index,
            },
        ],
    ))
}

/// Build a reparent operation that keeps the node's canvas position unchanged.
pub fn reparent_batch(
    context: &GestureContext,
    design: &StudioDesign,
    geometry: &CanvasGeometry,
    node_id: NodeId,
    destination: ParentPlacement,
) -> Result<CommandBatch, ManipulationError> {
    let old_parent = design
        .parents
        .get(&node_id)
        .cloned()
        .ok_or_else(|| ManipulationError::MissingNode(node_id.clone()))?;
    if !matches!(old_parent, NodeParent::Node { .. }) {
        return Err(ManipulationError::RootNode(node_id));
    }
    if !matches!(destination.parent, NodeParent::Node { .. }) {
        return Err(ManipulationError::InvalidDestination);
    }
    let frame = geometry
        .frame(&node_id)
        .ok_or_else(|| ManipulationError::MissingFrame(node_id.clone()))?;
    let old_index = child_index(design, &node_id)
        .ok_or_else(|| ManipulationError::MissingNode(node_id.clone()))?;
    Ok(context.batch(
        "Reparent layer",
        vec![
            Command::MoveNode {
                node_id: node_id.clone(),
                destination,
            },
            Command::SetCanvasRect {
                node_id: node_id.clone(),
                rect: frame,
            },
        ],
        vec![
            CommandPrecondition::NodeExists {
                node_id: node_id.clone(),
            },
            CommandPrecondition::ParentEquals {
                node_id: node_id.clone(),
                parent: old_parent,
            },
            CommandPrecondition::ChildIndexEquals {
                node_id,
                index: old_index,
            },
        ],
    ))
}

/// Build a duplicate operation and copy all known descendant frames.
pub fn duplicate_batch(
    context: &GestureContext,
    design: &StudioDesign,
    geometry: &CanvasGeometry,
    source_node_id: NodeId,
    destination: ParentPlacement,
    id_map: BTreeMap<NodeId, NodeId>,
) -> Result<CommandBatch, ManipulationError> {
    let source_ids = subtree_ids(design, &source_node_id)
        .ok_or_else(|| ManipulationError::MissingNode(source_node_id.clone()))?;
    if source_ids.iter().any(|id| !id_map.contains_key(id))
        || id_map.len() != source_ids.len()
        || id_map.values().collect::<BTreeSet<_>>().len() != id_map.len()
    {
        return Err(ManipulationError::InvalidIdentityMap);
    }
    let mut commands = vec![Command::DuplicateNode {
        source_node_id: source_node_id.clone(),
        destination,
        id_map: id_map.clone(),
    }];
    for source_id in source_ids {
        if let (Some(target_id), Some(frame)) = (id_map.get(&source_id), geometry.frame(&source_id))
        {
            commands.push(Command::SetCanvasRect {
                node_id: target_id.clone(),
                rect: frame,
            });
        }
    }
    Ok(context.batch(
        "Duplicate selection",
        commands,
        existing_preconditions([source_node_id]),
    ))
}

/// Build one delete batch for the selected top-level subtrees.
pub fn delete_batch(
    context: &GestureContext,
    design: &StudioDesign,
    selection: &[NodeId],
) -> Result<CommandBatch, ManipulationError> {
    let selection = selection_set(selection);
    if selection.is_empty() {
        return Err(ManipulationError::EmptySelection);
    }
    for node_id in &selection {
        let Some(parent) = design.parents.get(node_id) else {
            return Err(ManipulationError::MissingNode(node_id.clone()));
        };
        if !matches!(parent, NodeParent::Node { .. }) {
            return Err(ManipulationError::RootNode(node_id.clone()));
        }
    }
    let mut roots = selection
        .iter()
        .filter(|candidate| !has_selected_ancestor(design, candidate, &selection))
        .cloned()
        .collect::<Vec<_>>();
    roots.sort_by(|left, right| {
        child_index(design, right)
            .cmp(&child_index(design, left))
            .then(right.cmp(left))
    });
    let commands = roots
        .iter()
        .cloned()
        .map(|node_id| Command::DeleteNode { node_id })
        .collect::<Vec<_>>();
    Ok(context.batch("Delete selection", commands, existing_preconditions(roots)))
}

/// Build a restore batch from exact current tombstones.
pub fn restore_batch(
    context: &GestureContext,
    snapshot: &StudioDesignSnapshot,
    selection: &[NodeId],
) -> Result<CommandBatch, ManipulationError> {
    let mut ids = selection_set(selection);
    if ids.is_empty() {
        return Err(ManipulationError::EmptySelection);
    }
    ids.sort_by(|left, right| {
        tombstone_index(snapshot, left)
            .cmp(&tombstone_index(snapshot, right))
            .then(left.cmp(right))
    });
    let tombstones = ids
        .into_iter()
        .map(|node_id| {
            snapshot
                .tombstones
                .get(&node_id)
                .cloned()
                .map(Box::new)
                .ok_or(ManipulationError::MissingTombstone(node_id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let commands = tombstones
        .into_iter()
        .map(|tombstone| Command::RestoreNode { tombstone })
        .collect();
    Ok(context.batch("Restore selection", commands, Vec::new()))
}

fn tombstone_index(snapshot: &StudioDesignSnapshot, node_id: &NodeId) -> usize {
    snapshot
        .tombstones
        .get(node_id)
        .map_or(usize::MAX, |tombstone| tombstone.detached_index)
}

/// Build an alignment command batch.
pub fn align_batch(
    context: &GestureContext,
    geometry: &CanvasGeometry,
    selection: &[NodeId],
    alignment: CanvasAlignment,
) -> Result<CommandBatch, ManipulationError> {
    let selection = selection_set(selection);
    let targets = alignment_targets(geometry, &selection, alignment)?;
    Ok(context.batch(
        "Align selection",
        set_frame_commands(targets),
        existing_preconditions(selection),
    ))
}

/// Build a distribution command batch.
pub fn distribute_batch(
    context: &GestureContext,
    geometry: &CanvasGeometry,
    selection: &[NodeId],
    distribution: CanvasDistribution,
) -> Result<CommandBatch, ManipulationError> {
    let selection = selection_set(selection);
    let targets = distribution_targets(geometry, &selection, distribution)?;
    Ok(context.batch(
        "Distribute selection",
        set_frame_commands(targets),
        existing_preconditions(selection),
    ))
}

/// A hierarchy tree node that retains its source identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HierarchyNode {
    pub node_id: NodeId,
    pub name: String,
    pub parent: NodeParent,
    pub index: usize,
    pub children: Vec<Self>,
}

/// A hierarchy projection over the same source tree used by the canvas.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct HierarchySnapshot {
    pub roots: Vec<HierarchyNode>,
}

impl HierarchySnapshot {
    #[must_use]
    pub fn from_design(design: &StudioDesign) -> Self {
        fn build(
            design: &StudioDesign,
            node_id: &NodeId,
            parent: NodeParent,
            index: usize,
            seen: &mut BTreeSet<NodeId>,
        ) -> Option<HierarchyNode> {
            if !seen.insert(node_id.clone()) {
                return None;
            }
            let node = design.nodes.get(node_id)?;
            let children = node
                .children
                .iter()
                .enumerate()
                .filter_map(|(index, child)| {
                    let parent = NodeParent::Node {
                        node_id: node_id.clone(),
                    };
                    build(design, child, parent, index, seen)
                })
                .collect();
            Some(HierarchyNode {
                node_id: node_id.clone(),
                name: node.name.clone(),
                parent,
                index,
                children,
            })
        }
        let mut roots = Vec::new();
        let mut seen = BTreeSet::new();
        for screen_id in &design.screen_order {
            if let Some(screen) = design.screens.get(screen_id)
                && let Some(root) = build(
                    design,
                    &screen.root_node_id,
                    NodeParent::Screen {
                        screen_id: screen_id.clone(),
                    },
                    0,
                    &mut seen,
                )
            {
                roots.push(root);
            }
        }
        for (node_id, parent) in &design.parents {
            if !matches!(
                parent,
                NodeParent::Screen { .. } | NodeParent::Composition { .. }
            ) || seen.contains(node_id)
            {
                continue;
            }
            if let Some(root) = build(design, node_id, parent.clone(), 0, &mut seen) {
                roots.push(root);
            }
        }
        Self { roots }
    }

    #[must_use]
    pub fn find(&self, node_id: &NodeId) -> Option<&HierarchyNode> {
        fn find<'a>(nodes: &'a [HierarchyNode], node_id: &NodeId) -> Option<&'a HierarchyNode> {
            nodes.iter().find_map(|node| {
                (node.node_id == *node_id)
                    .then_some(node)
                    .or_else(|| find(&node.children, node_id))
            })
        }
        find(&self.roots, node_id)
    }

    #[must_use]
    pub fn selection(&self, selected: &[NodeId]) -> SelectionSnapshot {
        let ids = selected
            .iter()
            .filter(|node_id| self.find(node_id).is_some())
            .cloned()
            .collect::<Vec<_>>();
        let primary = ids.first().cloned();
        SelectionSnapshot {
            node_ids: ids,
            primary,
        }
    }
}

/// Edits emitted by hierarchy drag/drop and inline rename controls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HierarchyEdit {
    Rename {
        node_id: NodeId,
        name: String,
    },
    Reorder {
        node_id: NodeId,
        index: usize,
    },
    Reparent {
        node_id: NodeId,
        destination: ParentPlacement,
    },
}

/// Route hierarchy edits through the exact canvas command builders.
pub fn hierarchy_edit_batch(
    context: &GestureContext,
    design: &StudioDesign,
    geometry: &CanvasGeometry,
    edit: HierarchyEdit,
) -> Result<CommandBatch, ManipulationError> {
    match edit {
        HierarchyEdit::Rename { node_id, name } => Ok(context.batch(
            "Rename layer",
            vec![Command::RenameNode {
                node_id: node_id.clone(),
                name,
            }],
            vec![CommandPrecondition::NodeExists { node_id }],
        )),
        HierarchyEdit::Reorder { node_id, index } => reorder_batch(context, design, node_id, index),
        HierarchyEdit::Reparent {
            node_id,
            destination,
        } => reparent_batch(context, design, geometry, node_id, destination),
    }
}

fn child_index(design: &StudioDesign, node_id: &NodeId) -> Option<usize> {
    let NodeParent::Node { node_id: parent_id } = design.parents.get(node_id)? else {
        return None;
    };
    design
        .nodes
        .get(parent_id)?
        .children
        .iter()
        .position(|child| child == node_id)
}

fn subtree_ids(design: &StudioDesign, root: &NodeId) -> Option<Vec<NodeId>> {
    if !design.nodes.contains_key(root) {
        return None;
    }
    let mut result = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(node_id) = pending.pop() {
        result.push(node_id.clone());
        if let Some(node) = design.nodes.get(&node_id) {
            pending.extend(node.children.iter().rev().cloned());
        }
    }
    Some(result)
}

fn has_selected_ancestor(design: &StudioDesign, node_id: &NodeId, selected: &[NodeId]) -> bool {
    let selected = selected.iter().collect::<BTreeSet<_>>();
    let mut current = node_id;
    while let Some(NodeParent::Node { node_id: parent }) = design.parents.get(current) {
        if selected.contains(parent) {
            return true;
        }
        current = parent;
    }
    false
}
