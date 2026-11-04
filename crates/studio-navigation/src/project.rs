//! Bounded signed project routing metadata and host-owned navigation.
//!
//! Page identity is the compiled component ID, independent of concrete parameter
//! values. Navigating between `/streams/1` and `/streams/2` reuses that page's
//! retained state while updating route props, following SvelteKit-style page reuse.
//! Hosts can therefore bound their page-session cache by the admitted route count
//! rather than retaining an unbounded session for every visited parameter value.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use studio_protocol::NavigationCommand;

use crate::{
    GuardDecision, GuardResponse, NavigationGuard, NavigationOperation, NavigationStack,
    RouteDefinition, RouteResolution, RouteTree, StackOwner,
};

/// Reserved signed asset containing the compiled application's routing declaration.
pub const ROUTING_ASSET: &str = "assets/studio/routes.json";
/// Largest accepted routing metadata document.
pub const MAX_ROUTING_BYTES: usize = 1024 * 1024;
/// Maximum number of compiled project pages.
pub const MAX_PROJECT_ROUTES: usize = 64;

/// One page declared by the compiler and admitted by the host.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRoute {
    /// Stable compiled component identity.
    pub id: String,
    /// Absolute route pattern.
    pub path: String,
    /// Normalized project-relative `.studio` source path.
    pub source: String,
}

/// Closed versioned routing declaration stored with signed package assets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingManifest {
    /// Routing asset schema version; currently one.
    pub schema_version: u32,
    /// Route entries in deterministic compiler order.
    pub routes: Vec<ProjectRoute>,
}

impl RoutingManifest {
    /// Decode and validate bounded routing metadata without invoking page factories.
    ///
    /// # Errors
    /// Rejects oversized, malformed, unknown-field, incompatible or ambiguous metadata.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_ROUTING_BYTES {
            return Err("routing metadata exceeds 1 MiB".to_owned());
        }
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid routing metadata: {error}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate a routing declaration supplied directly by the compiler.
    ///
    /// # Errors
    /// Rejects invalid versions, limits, identities, paths, sources or ambiguous patterns.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("routing schemaVersion must be 1".to_owned());
        }
        if self.routes.is_empty() || self.routes.len() > MAX_PROJECT_ROUTES {
            return Err("routing metadata must declare between 1 and 64 pages".to_owned());
        }
        let mut ids = BTreeSet::new();
        let mut wildcards = 0;
        let mut definitions = Vec::new();
        for entry in &self.routes {
            if entry.id.is_empty()
                || entry.id.len() > 128
                || !entry
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                || !ids.insert(entry.id.as_str())
            {
                return Err(
                    "routing component IDs must be unique nonempty ASCII identifiers".to_owned(),
                );
            }
            if !valid_source(&entry.source) {
                return Err(format!("invalid route source `{}`", entry.source));
            }
            wildcards += usize::from(entry.path.split('/').any(|segment| segment == "*"));
            definitions.push(
                RouteDefinition::new(entry.path.clone(), || ())
                    .map_err(|error| format!("{}: {error}", entry.source))?,
            );
        }
        if wildcards > 1 {
            return Err("routing metadata permits at most one catch-all page".to_owned());
        }
        RouteTree::new(definitions, || ()).map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Choose the root page, or the first declared static path when no root exists.
    ///
    /// # Errors
    /// Rejects invalid metadata and projects containing only parameter/catch-all pages,
    /// because those declarations cannot supply a concrete startup destination.
    pub fn initial_route(&self) -> Result<String, String> {
        self.validate()?;
        self.routes
            .iter()
            .find(|entry| entry.path == "/")
            .or_else(|| {
                self.routes
                    .iter()
                    .find(|entry| !entry.path.contains([':', '*']))
            })
            .map(|entry| entry.path.clone())
            .ok_or_else(|| "routing metadata needs a static startup page".to_owned())
    }
}

fn valid_source(source: &str) -> bool {
    !source.is_empty()
        && source.len() <= 1024
        && source.ends_with(".studio")
        && !source.contains(['\\', ':'])
        && !source.chars().any(char::is_control)
        && source
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// The host's admitted concrete destination and selected compiled page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteSelection {
    /// Concrete, validated route in the navigation history.
    pub route: String,
    /// Compiled component identity selected by the declared route tree.
    pub id: String,
    /// Canonical parameter values from the shared Rust matcher.
    pub params: BTreeMap<String, String>,
}

/// One project's private owner-bound history and declared page resolver.
#[derive(Debug)]
pub struct ProjectRouter {
    manifest: RoutingManifest,
    owner: StackOwner,
    stack: NavigationStack,
    tree: RouteTree<String>,
    selection: RouteSelection,
}

struct OrdinaryNavigation;

impl NavigationGuard for OrdinaryNavigation {
    fn evaluate(&mut self, _: &str, _: &str, _: bool) -> GuardResponse {
        GuardResponse::new(GuardDecision::Allow, Duration::ZERO)
    }
}

impl ProjectRouter {
    /// Admit a routing declaration and initialize its private history.
    ///
    /// # Errors
    /// Rejects invalid metadata or an initial path without a declared page.
    pub fn new(manifest: &RoutingManifest, initial: &str) -> Result<Self, String> {
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        manifest.validate()?;
        let definitions = manifest
            .routes
            .iter()
            .map(|entry| {
                let id = entry.id.clone();
                RouteDefinition::new(entry.path.clone(), move || id.clone())
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut tree =
            RouteTree::new(definitions, String::new).map_err(|error| error.to_string())?;
        let selection = resolve(&mut tree, initial)?;
        let mut identity = [0; 16];
        identity[..8].copy_from_slice(&NEXT_OWNER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
        let owner = StackOwner::new(identity);
        let stack = NavigationStack::new(owner, initial).map_err(|error| error.to_string())?;
        Ok(Self {
            manifest: manifest.clone(),
            owner,
            stack,
            tree,
            selection,
        })
    }

    /// Apply a closed guest navigation request after resolving its destination.
    ///
    /// # Errors
    /// Invalid paths, unknown destinations, stack overflow, root pop and missing pop-to
    /// targets leave both history and active selection unchanged.
    pub fn navigate(&mut self, command: &NavigationCommand) -> Result<RouteSelection, String> {
        studio_protocol::navigation::validate_navigation_command(command)
            .map_err(|error| error.to_string())?;
        let operation = match command {
            NavigationCommand::Push { route } => NavigationOperation::Push(route),
            NavigationCommand::Replace { route } => NavigationOperation::Replace(route),
            NavigationCommand::Pop => NavigationOperation::Pop,
            NavigationCommand::PopTo { route } => NavigationOperation::PopTo(route),
            NavigationCommand::Reset { route } => NavigationOperation::Reset(route),
        };
        let mut candidate = self.stack.clone();
        candidate
            .apply(&self.owner, operation, &mut OrdinaryNavigation)
            .map_err(|error| error.to_string())?;
        let selection = resolve(&mut self.tree, candidate.current_route())?;
        self.stack = candidate;
        self.selection = selection.clone();
        Ok(selection)
    }

    /// Inspect the committed selection without exposing mutable stack authority.
    #[must_use]
    pub fn current(&self) -> RouteSelection {
        self.selection.clone()
    }

    /// Admit edited declarations while preserving valid active history.
    ///
    /// If the current concrete route still resolves, its owner and history are
    /// retained. Otherwise a fresh history starts at the new static entry page.
    /// Removed historical destinations remain unavailable to subsequent pops.
    ///
    /// # Errors
    /// Invalid replacement metadata, or a replacement without a concrete startup
    /// page after removal of the active destination, leaves this router unchanged.
    pub fn reconfigure(&self, manifest: &RoutingManifest) -> Result<Self, String> {
        manifest.validate()?;
        if let Ok(mut replacement) = Self::new(manifest, &self.selection.route) {
            replacement.owner = self.owner;
            replacement.stack = self.stack.clone();
            return Ok(replacement);
        }
        Self::new(manifest, &manifest.initial_route()?)
    }
}

impl Clone for ProjectRouter {
    fn clone(&self) -> Self {
        let mut cloned = Self::new(&self.manifest, &self.selection.route)
            .expect("admitted routing metadata and selection remain valid");
        cloned.owner = self.owner;
        cloned.stack = self.stack.clone();
        cloned
    }
}

fn resolve(tree: &mut RouteTree<String>, route: &str) -> Result<RouteSelection, String> {
    match tree.resolve(route).map_err(|error| error.to_string())? {
        RouteResolution::Matched { screen, params, .. } => Ok(RouteSelection {
            route: route.to_owned(),
            id: screen,
            params,
        }),
        RouteResolution::NotFound { .. } => Err(format!("no declared page matches `{route}`")),
    }
}
