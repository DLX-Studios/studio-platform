//! Isolated, deterministic prototype execution for Studio Design interactions.
//!
//! A [`PrototypeSession`] owns only a clone of the design plus ephemeral
//! navigation, property, visibility, and emitted-event state.  Dispatch never
//! submits a [`crate::CommandBatch`] and therefore cannot create or alter a
//! durable design revision.
#![allow(
    missing_docs,
    reason = "closed prototype effect fields mirror the domain schema"
)]

use std::{borrow::Borrow, collections::BTreeMap};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    InteractionAction, InteractionEvent, InteractionGraph, InteractionId, NavigationGraph,
    NavigationMode, NodeId, PropertyValue, ScreenId, StudioDesign,
    model::{DesignerDiagnostic, DiagnosticSeverity},
};

const MAX_PROTOTYPE_STACK_DEPTH: usize = 32;

/// A typed event delivered to an ephemeral prototype.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrototypeEvent {
    /// Stable node receiving the event.
    pub node_id: NodeId,
    /// Closed event type declared by the interaction model.
    pub event: InteractionEvent,
}

/// A source-linked prototype execution effect.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrototypeEffect {
    /// A navigation stack operation was applied.
    Navigate {
        from_screen_id: ScreenId,
        to_screen_id: ScreenId,
        mode: NavigationMode,
    },
    /// A local ephemeral property changed.
    SetProperty {
        node_id: NodeId,
        property: String,
        value: PropertyValue,
    },
    /// A local ephemeral visibility value changed.
    ToggleVisibility { node_id: NodeId, visible: bool },
    /// A declared event was emitted into the trace only.
    Emit { event: String },
    /// A sequence dispatched its nested interactions.
    Sequence { interaction_ids: Vec<InteractionId> },
}

/// One deterministic trace entry produced by a prototype dispatch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrototypeTraceEntry {
    /// Monotonic position within the prototype session trace.
    pub index: usize,
    /// Interaction that produced the effect.
    pub interaction_id: InteractionId,
    /// Source node of that interaction.
    pub source_node_id: NodeId,
    /// Typed trigger of that interaction.
    pub event: InteractionEvent,
    /// Effect applied to ephemeral state.
    pub effect: PrototypeEffect,
}

/// Immutable view of all ephemeral prototype state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrototypeStateSnapshot {
    /// Screen stack from root to active screen.
    pub screen_stack: Vec<ScreenId>,
    /// The active screen, if one exists.
    pub active_screen_id: Option<ScreenId>,
    /// Ephemeral property overrides keyed by node and property.
    pub properties: BTreeMap<NodeId, BTreeMap<String, PropertyValue>>,
    /// Ephemeral visibility values keyed by node.
    pub visibility: BTreeMap<NodeId, bool>,
    /// Events emitted by the prototype so far.
    pub emitted_events: Vec<String>,
}

/// The result of one event dispatch, including a stable trace delta.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrototypeDispatch {
    /// Event that was delivered.
    pub event: PrototypeEvent,
    /// Interactions reached in execution order.
    pub interaction_ids: Vec<InteractionId>,
    /// Effects produced by this dispatch in execution order.
    pub trace: Vec<PrototypeTraceEntry>,
    /// State after this dispatch.
    pub state: PrototypeStateSnapshot,
    /// Non-fatal execution diagnostics, if any.
    pub diagnostics: Vec<DesignerDiagnostic>,
}

/// Construction failures for a prototype session.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PrototypeError {
    /// The design contains invalid routes or interaction references.
    #[error("prototype design is invalid: {0:?}")]
    InvalidDesign(Vec<DesignerDiagnostic>),
    /// The requested start screen is not declared by the design.
    #[error("prototype start screen does not exist: {0}")]
    StartScreenMissing(ScreenId),
    /// A prototype cannot start without a screen.
    #[error("prototype design has no screens")]
    NoScreens,
}

/// Isolated interaction runner that never mutates its source design.
#[derive(Clone, Debug)]
pub struct PrototypeSession {
    design: StudioDesign,
    navigation: NavigationGraph,
    interactions: InteractionGraph,
    screen_stack: Vec<ScreenId>,
    properties: BTreeMap<NodeId, BTreeMap<String, PropertyValue>>,
    visibility: BTreeMap<NodeId, bool>,
    emitted_events: Vec<String>,
    trace: Vec<PrototypeTraceEntry>,
}

impl PrototypeSession {
    /// Build an isolated prototype rooted at the first declared screen.
    ///
    /// # Errors
    ///
    /// Returns [`PrototypeError::NoScreens`] for an empty design and
    /// [`PrototypeError::InvalidDesign`] when graph validation fails.
    pub fn new(design: impl Borrow<StudioDesign>) -> Result<Self, PrototypeError> {
        let design = design.borrow().clone();
        let start = design
            .screen_order
            .first()
            .cloned()
            .ok_or(PrototypeError::NoScreens)?;
        Self::new_at(design, start)
    }

    /// Build an isolated prototype rooted at a chosen screen.
    ///
    /// # Errors
    ///
    /// Returns [`PrototypeError::InvalidDesign`] when graph validation fails
    /// and [`PrototypeError::StartScreenMissing`] when the requested screen is
    /// not declared.
    pub fn new_at(
        design: impl Borrow<StudioDesign>,
        start_screen_id: ScreenId,
    ) -> Result<Self, PrototypeError> {
        let design = design.borrow().clone();
        let navigation = NavigationGraph::from_design(&design);
        if !navigation.is_valid() {
            return Err(PrototypeError::InvalidDesign(
                navigation.diagnostics().to_vec(),
            ));
        }
        let interactions = InteractionGraph::from_design(&design);
        if !interactions.is_valid() {
            return Err(PrototypeError::InvalidDesign(
                interactions.diagnostics().to_vec(),
            ));
        }
        if !navigation.screens().contains_key(&start_screen_id) {
            return Err(PrototypeError::StartScreenMissing(start_screen_id));
        }
        Ok(Self {
            design,
            navigation,
            interactions,
            screen_stack: vec![start_screen_id],
            properties: BTreeMap::new(),
            visibility: BTreeMap::new(),
            emitted_events: Vec::new(),
            trace: Vec::new(),
        })
    }

    /// Return the immutable design clone used by this preview.
    #[must_use]
    pub fn design(&self) -> &StudioDesign {
        &self.design
    }

    /// Return the domain-owned navigation graph.
    #[must_use]
    pub fn navigation_graph(&self) -> &NavigationGraph {
        &self.navigation
    }

    /// Return the domain-owned interaction graph.
    #[must_use]
    pub fn interaction_graph(&self) -> &InteractionGraph {
        &self.interactions
    }

    /// Return the active screen identity.
    #[must_use]
    pub fn active_screen_id(&self) -> Option<&ScreenId> {
        self.screen_stack.last()
    }

    /// Return the active route without exposing host router state.
    #[must_use]
    pub fn active_route(&self) -> Option<&str> {
        self.active_screen_id()
            .and_then(|screen_id| self.navigation.route_for_screen(screen_id))
    }

    /// Return an immutable snapshot of ephemeral state.
    #[must_use]
    pub fn state(&self) -> PrototypeStateSnapshot {
        PrototypeStateSnapshot {
            screen_stack: self.screen_stack.clone(),
            active_screen_id: self.screen_stack.last().cloned(),
            properties: self.properties.clone(),
            visibility: self.visibility.clone(),
            emitted_events: self.emitted_events.clone(),
        }
    }

    /// Return all trace entries accumulated by this preview.
    #[must_use]
    pub fn trace(&self) -> &[PrototypeTraceEntry] {
        &self.trace
    }

    /// Read an ephemeral property, falling back to its authored value.
    #[must_use]
    pub fn property(&self, node_id: &NodeId, property: &str) -> Option<PropertyValue> {
        self.properties
            .get(node_id)
            .and_then(|properties| properties.get(property))
            .cloned()
            .or_else(|| {
                self.design
                    .nodes
                    .get(node_id)
                    .and_then(|node| node.properties.get(property))
                    .cloned()
            })
    }

    /// Read an ephemeral visibility value, falling back to authored metadata.
    #[must_use]
    pub fn is_visible(&self, node_id: &NodeId) -> Option<bool> {
        self.visibility.get(node_id).copied().or_else(|| {
            self.design
                .nodes
                .get(node_id)
                .map(|node| !node.accessibility.hidden)
        })
    }

    /// Deliver a typed node event and return its deterministic effects.
    pub fn dispatch(&mut self, event: PrototypeEvent) -> PrototypeDispatch {
        let mut trace = Vec::new();
        let mut interaction_ids = Vec::new();
        let mut diagnostics = Vec::new();
        if self.design.nodes.contains_key(&event.node_id) {
            let ids = self
                .interactions
                .inspect(&event.node_id, event.event)
                .into_iter()
                .map(|entry| entry.interaction_id.clone())
                .collect::<Vec<_>>();
            for interaction_id in ids {
                self.execute(
                    &interaction_id,
                    &mut Vec::new(),
                    &mut interaction_ids,
                    &mut trace,
                    &mut diagnostics,
                );
            }
        } else {
            diagnostics.push(prototype_diagnostic(
                "PROTOTYPE_SOURCE_NODE_MISSING",
                format!("prototype event names missing node {}", event.node_id),
                None,
                Some(event.node_id.clone()),
            ));
        }
        self.trace.extend(trace.iter().cloned());
        PrototypeDispatch {
            event,
            interaction_ids,
            trace,
            state: self.state(),
            diagnostics,
        }
    }

    /// Convenience overload accepting node and event separately.
    pub fn dispatch_event(
        &mut self,
        node_id: NodeId,
        event: InteractionEvent,
    ) -> PrototypeDispatch {
        self.dispatch(PrototypeEvent { node_id, event })
    }

    /// Dispatch a declared interaction directly, useful for deterministic tests.
    ///
    /// # Panics
    ///
    /// The fallback identity is a compile-time-safe literal and cannot fail
    /// identity validation.
    #[allow(clippy::missing_panics_doc)]
    pub fn dispatch_interaction(&mut self, interaction_id: &InteractionId) -> PrototypeDispatch {
        let event = self
            .interactions
            .entry(interaction_id)
            .map_or_else(|| PrototypeEvent {
                node_id: NodeId::new(format!("missing:{interaction_id}")).expect("safe identity"),
                event: InteractionEvent::Pressed,
            }, |entry| PrototypeEvent {
                node_id: entry.source_node_id.clone(),
                event: entry.event,
            });
        let mut trace = Vec::new();
        let mut interaction_ids = Vec::new();
        let mut diagnostics = Vec::new();
        self.execute(
            interaction_id,
            &mut Vec::new(),
            &mut interaction_ids,
            &mut trace,
            &mut diagnostics,
        );
        self.trace.extend(trace.iter().cloned());
        PrototypeDispatch {
            event,
            interaction_ids,
            trace,
            state: self.state(),
            diagnostics,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn execute(
        &mut self,
        interaction_id: &InteractionId,
        call_stack: &mut Vec<InteractionId>,
        interaction_ids: &mut Vec<InteractionId>,
        trace: &mut Vec<PrototypeTraceEntry>,
        diagnostics: &mut Vec<DesignerDiagnostic>,
    ) {
        if call_stack.contains(interaction_id) {
            diagnostics.push(prototype_diagnostic(
                "PROTOTYPE_INTERACTION_CYCLE",
                format!("prototype stopped recursive interaction {interaction_id}"),
                Some(interaction_id.clone()),
                self.interactions
                    .entry(interaction_id)
                    .map(|entry| entry.source_node_id.clone()),
            ));
            return;
        }
        let Some(entry) = self.interactions.entry(interaction_id).cloned() else {
            diagnostics.push(prototype_diagnostic(
                "PROTOTYPE_INTERACTION_MISSING",
                format!("prototype sequence names missing interaction {interaction_id}"),
                Some(interaction_id.clone()),
                None,
            ));
            return;
        };
        call_stack.push(interaction_id.clone());
        interaction_ids.push(interaction_id.clone());
        let effect = match &entry.action {
            InteractionAction::Navigate { screen_id, mode } => {
                if let Some((from_screen_id, to_screen_id)) = self.navigate(
                    &entry.source_node_id,
                    interaction_id,
                    screen_id,
                    *mode,
                    diagnostics,
                ) {
                    PrototypeEffect::Navigate {
                        from_screen_id,
                        to_screen_id,
                        mode: *mode,
                    }
                } else {
                    call_stack.pop();
                    return;
                }
            }
            InteractionAction::SetProperty {
                node_id,
                property,
                value,
            } => {
                if !self.design.nodes.contains_key(node_id) {
                    diagnostics.push(prototype_diagnostic(
                        "PROTOTYPE_TARGET_MISSING",
                        format!("prototype property target {node_id} is missing"),
                        Some(interaction_id.clone()),
                        Some(entry.source_node_id.clone()),
                    ));
                    call_stack.pop();
                    return;
                }
                self.properties
                    .entry(node_id.clone())
                    .or_default()
                    .insert(property.clone(), value.clone());
                PrototypeEffect::SetProperty {
                    node_id: node_id.clone(),
                    property: property.clone(),
                    value: value.clone(),
                }
            }
            InteractionAction::ToggleVisibility { node_id } => {
                if !self.design.nodes.contains_key(node_id) {
                    diagnostics.push(prototype_diagnostic(
                        "PROTOTYPE_TARGET_MISSING",
                        format!("prototype visibility target {node_id} is missing"),
                        Some(interaction_id.clone()),
                        Some(entry.source_node_id.clone()),
                    ));
                    call_stack.pop();
                    return;
                }
                let visible = !self.is_visible(node_id).unwrap_or(true);
                self.visibility.insert(node_id.clone(), visible);
                PrototypeEffect::ToggleVisibility {
                    node_id: node_id.clone(),
                    visible,
                }
            }
            InteractionAction::Emit { event } => {
                self.emitted_events.push(event.clone());
                PrototypeEffect::Emit {
                    event: event.clone(),
                }
            }
            InteractionAction::Sequence {
                interaction_ids: nested,
            } => {
                let nested = nested.clone();
                for nested_id in &nested {
                    self.execute(nested_id, call_stack, interaction_ids, trace, diagnostics);
                }
                PrototypeEffect::Sequence {
                    interaction_ids: nested,
                }
            }
        };
        trace.push(PrototypeTraceEntry {
            index: self.trace.len() + trace.len(),
            interaction_id: interaction_id.clone(),
            source_node_id: entry.source_node_id,
            event: entry.event,
            effect,
        });
        call_stack.pop();
    }

    fn navigate(
        &mut self,
        source_node_id: &NodeId,
        interaction_id: &InteractionId,
        target: &ScreenId,
        mode: NavigationMode,
        diagnostics: &mut Vec<DesignerDiagnostic>,
    ) -> Option<(ScreenId, ScreenId)> {
        if !self.navigation.screens().contains_key(target) {
            diagnostics.push(prototype_diagnostic(
                "PROTOTYPE_NAVIGATION_TARGET_MISSING",
                format!("prototype navigation target {target} is missing"),
                Some(interaction_id.clone()),
                Some(source_node_id.clone()),
            ));
            return None;
        }
        let from = self.screen_stack.last().cloned()?;
        match mode {
            NavigationMode::Push => {
                if self.screen_stack.len() >= MAX_PROTOTYPE_STACK_DEPTH {
                    diagnostics.push(prototype_diagnostic(
                        "PROTOTYPE_STACK_OVERFLOW",
                        "prototype navigation stack depth exceeded".to_owned(),
                        Some(interaction_id.clone()),
                        Some(source_node_id.clone()),
                    ));
                    return None;
                }
                self.screen_stack.push(target.clone());
            }
            NavigationMode::Replace => {
                if let Some(active) = self.screen_stack.last_mut() {
                    *active = target.clone();
                }
            }
            NavigationMode::Reset => self.screen_stack = vec![target.clone()],
            NavigationMode::PopTo => {
                let Some(index) = self
                    .screen_stack
                    .iter()
                    .rposition(|screen| screen == target)
                else {
                    diagnostics.push(prototype_diagnostic(
                        "PROTOTYPE_NAVIGATION_TARGET_NOT_IN_STACK",
                        format!("prototype pop-to target {target} is not in the stack"),
                        Some(interaction_id.clone()),
                        Some(source_node_id.clone()),
                    ));
                    return None;
                };
                self.screen_stack.truncate(index + 1);
            }
        }
        Some((from, target.clone()))
    }
}

fn prototype_diagnostic(
    code: &str,
    message: String,
    interaction_id: Option<InteractionId>,
    node_id: Option<NodeId>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message,
        node_id,
        interaction_id,
    }
}
