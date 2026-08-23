//! Closed route pattern parsing and matching.

use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt,
};

/// Stable route declaration or resolution failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteErrorCode {
    /// A declared route pattern is malformed.
    InvalidPattern,
    /// Two declarations have the same structural matching shape.
    AmbiguousDeclaration,
    /// A requested concrete route is malformed.
    InvalidRoute,
}

/// Safe host route error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteError {
    code: RouteErrorCode,
}

impl RouteError {
    pub(crate) const fn new(code: RouteErrorCode) -> Self {
        Self { code }
    }

    /// Stable failure code.
    #[must_use]
    pub const fn code(&self) -> RouteErrorCode {
        self.code
    }
}

impl fmt::Display for RouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            RouteErrorCode::InvalidPattern => "route pattern invalid",
            RouteErrorCode::AmbiguousDeclaration => "route declarations are ambiguous",
            RouteErrorCode::InvalidRoute => "requested route invalid",
        })
    }
}

impl Error for RouteError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Segment {
    Static(String),
    Parameter(String),
}

/// One declared route and its lazy screen factory.
pub struct RouteDefinition<T> {
    pub(crate) pattern: String,
    pub(crate) segments: Vec<Segment>,
    pub(crate) factory: Box<dyn FnMut() -> T>,
}

impl<T> RouteDefinition<T> {
    /// Parse a route declaration and retain, but do not invoke, its screen factory.
    ///
    /// # Errors
    ///
    /// Rejects non-absolute, root-only, unsafe, repeated-parameter, or empty-segment patterns.
    pub fn new(
        pattern: impl Into<String>,
        factory: impl FnMut() -> T + 'static,
    ) -> Result<Self, RouteError> {
        let pattern = pattern.into();
        let segments = parse_pattern(&pattern)?;
        Ok(Self {
            pattern,
            segments,
            factory: Box::new(factory),
        })
    }

    pub(crate) fn shape(&self) -> String {
        self.segments
            .iter()
            .map(|segment| match segment {
                Segment::Static(value) => value.as_str(),
                Segment::Parameter(_) => ":",
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    pub(crate) fn static_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| matches!(segment, Segment::Static(_)))
            .count()
    }

    pub(crate) fn matches(&self, route: &[&str]) -> Option<BTreeMap<String, String>> {
        if route.len() != self.segments.len() {
            return None;
        }
        let mut params = BTreeMap::new();
        for (declared, actual) in self.segments.iter().zip(route) {
            match declared {
                Segment::Static(expected) if expected != actual => return None,
                Segment::Static(_) => {}
                Segment::Parameter(name) => {
                    params.insert(name.clone(), (*actual).to_owned());
                }
            }
        }
        Some(params)
    }
}

impl<T> fmt::Debug for RouteDefinition<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RouteDefinition")
            .field("pattern", &self.pattern)
            .finish_non_exhaustive()
    }
}

pub(crate) fn parse_route(route: &str) -> Result<Vec<&str>, RouteError> {
    if !valid_common(route) || route == "/" {
        return Err(RouteError::new(RouteErrorCode::InvalidRoute));
    }
    let segments = route[1..].split('/').collect::<Vec<_>>();
    if segments.iter().any(|segment| {
        segment.is_empty() || matches!(*segment, "." | "..") || segment.starts_with(':')
    }) {
        return Err(RouteError::new(RouteErrorCode::InvalidRoute));
    }
    Ok(segments)
}

fn parse_pattern(pattern: &str) -> Result<Vec<Segment>, RouteError> {
    if !valid_common(pattern) || pattern == "/" {
        return Err(RouteError::new(RouteErrorCode::InvalidPattern));
    }
    let mut names = HashSet::new();
    pattern[1..]
        .split('/')
        .map(|segment| {
            if segment.is_empty() || matches!(segment, "." | "..") {
                return Err(RouteError::new(RouteErrorCode::InvalidPattern));
            }
            if let Some(name) = segment.strip_prefix(':') {
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    || !names.insert(name.to_owned())
                {
                    return Err(RouteError::new(RouteErrorCode::InvalidPattern));
                }
                Ok(Segment::Parameter(name.to_owned()))
            } else {
                Ok(Segment::Static(segment.to_owned()))
            }
        })
        .collect()
}

fn valid_common(route: &str) -> bool {
    route.starts_with('/')
        && route.len() <= 2048
        && !route.contains("//")
        && !route.contains(['?', '#'])
        && !route.contains('\\')
        && !route.chars().any(char::is_control)
}
