//! Domain-owned navigation and interaction graphs.
//!
//! The graph is derived from [`StudioDesign`] and is deliberately independent
//! of any host router.  It is the common inspection surface for the Designer,
//! prototype runner, Runtime projection, and future Workbench UI.
#![allow(
    missing_docs,
    reason = "graph fields are documented at the public type level"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::model::{
    DesignerDiagnostic, DiagnosticSeverity, InteractionAction, InteractionEvent, InteractionId,
    NavigationMode, NodeId, NodeParent, Screen, ScreenId, StudioDesign,
};

/// Stable diagnostic for a route that cannot be used by a Designer screen.
pub const CODE_ROUTE_INVALID: &str = "DESIGN_ROUTE_INVALID";
/// Stable diagnostic for two screens declaring the same route.
pub const CODE_ROUTE_DUPLICATE: &str = "DESIGN_ROUTE_DUPLICATE";
/// Stable diagnostic for an interaction source that is not a node.
pub const CODE_INTERACTION_SOURCE_MISSING: &str = "DESIGN_INTERACTION_SOURCE_MISSING";
/// Stable diagnostic for a missing screen, node, or interaction target.
pub const CODE_INTERACTION_TARGET_MISSING: &str = "DESIGN_INTERACTION_TARGET_MISSING";
/// Stable diagnostic for a recursive interaction sequence.
pub const CODE_INTERACTION_CYCLE: &str = "DESIGN_INTERACTION_CYCLE";

/// A screen and its typed route as exposed by the domain graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NavigationScreen {
    /// Stable screen identity.
    pub id: ScreenId,
    /// Human-readable screen name.
    pub name: String,
    /// Canonical route.
    pub route: String,
    /// Stable visual root identity.
    pub root_node_id: NodeId,
}

impl From<&Screen> for NavigationScreen {
    fn from(screen: &Screen) -> Self {
        Self {
            id: screen.id.clone(),
            name: screen.name.clone(),
            route: screen.route.clone(),
            root_node_id: screen.root_node_id.clone(),
        }
    }
}

/// One navigation edge originating at a declared interaction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NavigationEdge {
    /// The interaction that declares this edge.
    pub interaction_id: InteractionId,
    /// The node receiving the event.
    pub source_node_id: NodeId,
    /// The screen containing the source node, when it can be resolved.
    pub source_screen_id: Option<ScreenId>,
    /// The declared target screen.
    pub target_screen_id: ScreenId,
    /// How the prototype stack changes.
    pub mode: NavigationMode,
}

/// A deterministic route and navigation graph derived from one design.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NavigationGraph {
    screens: BTreeMap<ScreenId, NavigationScreen>,
    routes: BTreeMap<String, ScreenId>,
    edges: BTreeMap<InteractionId, NavigationEdge>,
    diagnostics: Vec<DesignerDiagnostic>,
}

impl NavigationGraph {
    /// Derive a graph without constructing or mutating any runtime/UI object.
    ///
    /// Invalid references remain represented in [`Self::diagnostics`] so the
    /// editor can display source-linked problems rather than losing the graph.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_design(design: &StudioDesign) -> Self {
        let mut diagnostics = Vec::new();
        let mut screens = BTreeMap::new();
        let mut routes = BTreeMap::new();
        for (screen_id, screen) in &design.screens {
            screens.insert(screen_id.clone(), NavigationScreen::from(screen));
            if !valid_route(&screen.route) {
                diagnostics.push(DesignerDiagnostic {
                    code: CODE_ROUTE_INVALID.to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("screen {screen_id} declares an invalid route"),
                    node_id: Some(screen.root_node_id.clone()),
                    interaction_id: None,
                    collection_id: None,
                    binding_id: None,
                    form_id: None,
                    record_id: None,
                });
            } else if let Some(previous) = routes.insert(screen.route.clone(), screen_id.clone()) {
                diagnostics.push(DesignerDiagnostic {
                    code: CODE_ROUTE_DUPLICATE.to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: format!(
                        "screens {previous} and {screen_id} both declare route {}",
                        screen.route
                    ),
                    node_id: Some(screen.root_node_id.clone()),
                    interaction_id: None,
                    collection_id: None,
                    binding_id: None,
                    form_id: None,
                    record_id: None,
                });
            }
        }

        let interaction_graph = InteractionGraph::from_design(design);
        diagnostics.extend(interaction_graph.diagnostics().iter().cloned());
        let mut edges = BTreeMap::new();
        for (interaction_id, interaction) in &design.interactions {
            if let InteractionAction::Navigate { screen_id, mode } = &interaction.action {
                edges.insert(
                    interaction_id.clone(),
                    NavigationEdge {
                        interaction_id: interaction_id.clone(),
                        source_node_id: interaction.source.node_id.clone(),
                        source_screen_id: screen_screen_id(design, &interaction.source.node_id),
                        target_screen_id: screen_id.clone(),
                        mode: *mode,
                    },
                );
            }
        }

        Self {
            screens,
            routes,
            edges,
            diagnostics,
        }
    }

    /// Alias for [`Self::from_design`] convenient for domain callers.
    #[must_use]
    pub fn new(design: &StudioDesign) -> Self {
        Self::from_design(design)
    }

    /// Return all screens in stable identity order.
    #[must_use]
    pub fn screens(&self) -> &BTreeMap<ScreenId, NavigationScreen> {
        &self.screens
    }

    /// Return the screen selected by a canonical route.
    #[must_use]
    pub fn screen_for_route(&self, route: &str) -> Option<&ScreenId> {
        self.routes.get(route)
    }

    /// Return a route for a stable screen identity.
    #[must_use]
    pub fn route_for_screen(&self, screen_id: &ScreenId) -> Option<&str> {
        self.screens
            .get(screen_id)
            .map(|screen| screen.route.as_str())
    }

    /// Return one screen by identity.
    #[must_use]
    pub fn screen(&self, screen_id: &ScreenId) -> Option<&NavigationScreen> {
        self.screens.get(screen_id)
    }

    /// Return all declared navigation edges in stable interaction order.
    #[must_use]
    pub fn edges(&self) -> &BTreeMap<InteractionId, NavigationEdge> {
        &self.edges
    }

    /// Return edges originating from one screen.
    #[must_use]
    pub fn outgoing(&self, screen_id: &ScreenId) -> Vec<&NavigationEdge> {
        self.edges
            .values()
            .filter(|edge| edge.source_screen_id.as_ref() == Some(screen_id))
            .collect()
    }

    /// Return source-linked graph diagnostics in deterministic order.
    #[must_use]
    pub fn diagnostics(&self) -> &[DesignerDiagnostic] {
        &self.diagnostics
    }

    /// Return whether this graph has no invalid route or interaction links.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// One interaction with its source screen resolved for the event inspector.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InteractionInspectorEntry {
    /// Stable interaction identity.
    pub interaction_id: InteractionId,
    /// Stable source node identity.
    pub source_node_id: NodeId,
    /// Screen containing the source node, if available.
    pub source_screen_id: Option<ScreenId>,
    /// Typed event that activates the interaction.
    pub event: InteractionEvent,
    /// Typed effect shown by the inspector.
    pub action: InteractionAction,
}

/// Alias emphasizing that this is the event-inspector projection.
pub type EventInspectorEntry = InteractionInspectorEntry;

/// A typed interaction graph used by both inspection and prototype execution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InteractionGraph {
    interactions: BTreeMap<InteractionId, InteractionInspectorEntry>,
    attached: BTreeMap<NodeId, Vec<InteractionId>>,
    sequence_edges: BTreeMap<InteractionId, Vec<InteractionId>>,
    diagnostics: Vec<DesignerDiagnostic>,
}

impl InteractionGraph {
    /// Derive the interaction graph and retain all source-linked diagnostics.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_design(design: &StudioDesign) -> Self {
        let mut interactions = BTreeMap::new();
        let mut sequence_edges = BTreeMap::new();
        let mut diagnostics = Vec::new();

        for (interaction_id, interaction) in &design.interactions {
            let source_exists = design.nodes.contains_key(&interaction.source.node_id);
            if !source_exists {
                diagnostics.push(interaction_diagnostic(
                    CODE_INTERACTION_SOURCE_MISSING,
                    format!(
                        "interaction {interaction_id} names missing source node {}",
                        interaction.source.node_id
                    ),
                    interaction_id,
                    Some(interaction.source.node_id.clone()),
                ));
            }
            let entry = InteractionInspectorEntry {
                interaction_id: interaction_id.clone(),
                source_node_id: interaction.source.node_id.clone(),
                source_screen_id: screen_screen_id(design, &interaction.source.node_id),
                event: interaction.source.event,
                action: interaction.action.clone(),
            };
            let sequence = match &interaction.action {
                InteractionAction::Sequence { interaction_ids } => interaction_ids.clone(),
                _ => Vec::new(),
            };
            for target in &sequence {
                if !design.interactions.contains_key(target) {
                    diagnostics.push(interaction_diagnostic(
                        CODE_INTERACTION_TARGET_MISSING,
                        format!(
                            "interaction {interaction_id} sequences missing interaction {target}"
                        ),
                        interaction_id,
                        Some(interaction.source.node_id.clone()),
                    ));
                }
            }
            sequence_edges.insert(interaction_id.clone(), sequence);

            match &interaction.action {
                InteractionAction::Navigate { screen_id, .. }
                    if !design.screens.contains_key(screen_id) =>
                {
                    diagnostics.push(interaction_diagnostic(
                        CODE_INTERACTION_TARGET_MISSING,
                        format!(
                            "interaction {interaction_id} navigates to missing screen {screen_id}"
                        ),
                        interaction_id,
                        Some(interaction.source.node_id.clone()),
                    ));
                }
                InteractionAction::SetProperty { node_id, .. }
                | InteractionAction::ToggleVisibility { node_id }
                    if !design.nodes.contains_key(node_id) =>
                {
                    diagnostics.push(interaction_diagnostic(
                        CODE_INTERACTION_TARGET_MISSING,
                        format!("interaction {interaction_id} targets missing node {node_id}"),
                        interaction_id,
                        Some(interaction.source.node_id.clone()),
                    ));
                }
                _ => {}
            }
            interactions.insert(interaction_id.clone(), entry);
        }

        let cycle_ids = find_cycle_ids(&sequence_edges);
        for interaction_id in cycle_ids {
            let source_node_id = interactions
                .get(&interaction_id)
                .map(|entry| entry.source_node_id.clone());
            diagnostics.push(interaction_diagnostic(
                CODE_INTERACTION_CYCLE,
                format!("interaction sequence contains a cycle at {interaction_id}"),
                &interaction_id,
                source_node_id,
            ));
        }

        let mut attached = BTreeMap::new();
        for (node_id, node) in &design.nodes {
            let ids = node
                .interaction_ids
                .iter()
                .filter(|interaction_id| {
                    interactions
                        .get(*interaction_id)
                        .is_some_and(|entry| entry.source_node_id == *node_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            if !ids.is_empty() {
                attached.insert(node_id.clone(), ids);
            }
        }

        Self {
            interactions,
            attached,
            sequence_edges,
            diagnostics,
        }
    }

    /// Return all entries in stable interaction identity order.
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<InteractionId, InteractionInspectorEntry> {
        &self.interactions
    }

    /// Return one interaction's inspector projection.
    #[must_use]
    pub fn entry(&self, interaction_id: &InteractionId) -> Option<&InteractionInspectorEntry> {
        self.interactions.get(interaction_id)
    }

    /// Return the interactions activated by one node event.
    #[must_use]
    pub fn inspect(
        &self,
        node_id: &NodeId,
        event: InteractionEvent,
    ) -> Vec<&InteractionInspectorEntry> {
        if let Some(attached) = self.attached.get(node_id) {
            return attached
                .iter()
                .filter_map(|interaction_id| self.interactions.get(interaction_id))
                .filter(|entry| entry.event == event)
                .collect();
        }
        self.interactions
            .values()
            .filter(|entry| entry.source_node_id == *node_id && entry.event == event)
            .collect()
    }

    /// Alias for [`Self::inspect`] used by event-inspector consumers.
    #[must_use]
    pub fn event_inspector(
        &self,
        node_id: &NodeId,
        event: InteractionEvent,
    ) -> Vec<&InteractionInspectorEntry> {
        self.inspect(node_id, event)
    }

    /// Return nested interaction IDs for one sequence action.
    #[must_use]
    pub fn sequence_targets(&self, interaction_id: &InteractionId) -> &[InteractionId] {
        self.sequence_edges
            .get(interaction_id)
            .map_or(&[][..], Vec::as_slice)
    }

    /// Return source-linked graph diagnostics in deterministic order.
    #[must_use]
    pub fn diagnostics(&self) -> &[DesignerDiagnostic] {
        &self.diagnostics
    }

    /// Return whether no missing references or sequence cycles were found.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

fn interaction_diagnostic(
    code: &str,
    message: String,
    interaction_id: &InteractionId,
    node_id: Option<NodeId>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message,
        node_id,
        interaction_id: Some(interaction_id.clone()),
        collection_id: None,
        binding_id: None,
        form_id: None,
        record_id: None,
    }
}

/// Resolve the owning screen through the validated flat parent index.
#[must_use]
pub fn screen_screen_id(design: &StudioDesign, node_id: &NodeId) -> Option<ScreenId> {
    let mut seen = BTreeSet::new();
    let mut cursor = node_id;
    loop {
        if !seen.insert(cursor) {
            return None;
        }
        match design.parents.get(cursor) {
            Some(NodeParent::Screen { screen_id }) => return Some(screen_id.clone()),
            Some(NodeParent::Node { node_id: parent }) => cursor = parent,
            Some(NodeParent::Composition { .. }) | None => return None,
        }
    }
}

fn find_cycle_ids(edges: &BTreeMap<InteractionId, Vec<InteractionId>>) -> BTreeSet<InteractionId> {
    fn visit(
        id: &InteractionId,
        edges: &BTreeMap<InteractionId, Vec<InteractionId>>,
        state: &mut BTreeMap<InteractionId, u8>,
        stack: &mut Vec<InteractionId>,
        cycle_ids: &mut BTreeSet<InteractionId>,
    ) {
        state.insert(id.clone(), 1);
        stack.push(id.clone());
        if let Some(targets) = edges.get(id) {
            for target in targets {
                match state.get(target).copied().unwrap_or(0) {
                    0 => visit(target, edges, state, stack, cycle_ids),
                    1 => {
                        if let Some(start) = stack.iter().position(|entry| entry == target) {
                            cycle_ids.extend(stack[start..].iter().cloned());
                        }
                    }
                    _ => {}
                }
            }
        }
        stack.pop();
        state.insert(id.clone(), 2);
    }

    let mut state = BTreeMap::new();
    let mut stack = Vec::new();
    let mut cycle_ids = BTreeSet::new();
    for id in edges.keys() {
        if state.get(id).copied().unwrap_or(0) == 0 {
            visit(id, edges, &mut state, &mut stack, &mut cycle_ids);
        }
    }
    cycle_ids
}

/// Validate the bounded route syntax used by the domain model.
#[must_use]
pub fn valid_route(route: &str) -> bool {
    route == "/"
        || (route.starts_with('/')
            && route.len() <= 2048
            && !route.contains("//")
            && !route.contains(['?', '#', '\\'])
            && !route.chars().any(char::is_control)
            && route[1..]
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != ".."))
}
