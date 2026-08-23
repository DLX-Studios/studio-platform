//! Host-owned deterministic route resolution and lazy screen creation.

use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};

use crate::{RouteDefinition, RouteError, RouteErrorCode, route::parse_route};

/// Result of resolving and lazily constructing exactly one screen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteResolution<T> {
    /// A declared route matched.
    Matched {
        /// Declared pattern that matched.
        pattern: String,
        /// Canonical parameter values derived by the host.
        params: BTreeMap<String, String>,
        /// Lazily constructed screen.
        screen: T,
    },
    /// No declaration matched; the explicit not-found screen was constructed.
    NotFound {
        /// Requested canonical route.
        route: String,
        /// Lazily constructed not-found screen.
        screen: T,
    },
}

impl<T> RouteResolution<T> {
    /// Borrow the constructed screen for either outcome.
    #[must_use]
    pub const fn screen(&self) -> &T {
        match self {
            Self::Matched { screen, .. } | Self::NotFound { screen, .. } => screen,
        }
    }

    /// Matched declaration, absent for not-found.
    #[must_use]
    pub fn pattern(&self) -> Option<&str> {
        match self {
            Self::Matched { pattern, .. } => Some(pattern),
            Self::NotFound { .. } => None,
        }
    }

    /// Host-derived parameters; empty for not-found.
    #[must_use]
    pub fn params(&self) -> &BTreeMap<String, String> {
        match self {
            Self::Matched { params, .. } => params,
            Self::NotFound { .. } => empty_params(),
        }
    }
}

fn empty_params() -> &'static BTreeMap<String, String> {
    static EMPTY: std::sync::LazyLock<BTreeMap<String, String>> =
        std::sync::LazyLock::new(BTreeMap::new);
    &EMPTY
}

/// Validated declared route tree with one explicit not-found factory.
pub struct RouteTree<T> {
    definitions: Vec<RouteDefinition<T>>,
    not_found: Box<dyn FnMut() -> T>,
}

impl<T> RouteTree<T> {
    /// Validate declarations without constructing any screen.
    ///
    /// # Errors
    ///
    /// Rejects empty or structurally ambiguous declaration sets.
    pub fn new(
        definitions: impl IntoIterator<Item = RouteDefinition<T>>,
        not_found: impl FnMut() -> T + 'static,
    ) -> Result<Self, RouteError> {
        let mut definitions = definitions.into_iter().collect::<Vec<_>>();
        if definitions.is_empty() {
            return Err(RouteError::new(RouteErrorCode::InvalidPattern));
        }
        let mut shapes = HashSet::new();
        if definitions
            .iter()
            .any(|definition| !shapes.insert(definition.shape()))
        {
            return Err(RouteError::new(RouteErrorCode::AmbiguousDeclaration));
        }
        definitions.sort_by_key(|definition| std::cmp::Reverse(definition.static_count()));
        Ok(Self {
            definitions,
            not_found: Box::new(not_found),
        })
    }

    /// Resolve one concrete route and invoke only the selected screen factory.
    ///
    /// # Errors
    ///
    /// Rejects malformed concrete routes before invoking any factory.
    pub fn resolve(&mut self, route: &str) -> Result<RouteResolution<T>, RouteError> {
        let segments = parse_route(route)?;
        for definition in &mut self.definitions {
            if let Some(params) = definition.matches(&segments) {
                return Ok(RouteResolution::Matched {
                    pattern: definition.pattern.clone(),
                    params,
                    screen: (definition.factory)(),
                });
            }
        }
        Ok(RouteResolution::NotFound {
            route: route.to_owned(),
            screen: (self.not_found)(),
        })
    }
}

impl<T> fmt::Debug for RouteTree<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteTree")
            .field("definitions", &self.definitions)
            .finish_non_exhaustive()
    }
}
