//! Request admission: matching one guest request against compiled route groups.
//!
//! Denial is ordered and stable: origin first, then path/method, then headers. Every denial
//! carries a distinct code from [`crate::error::BrokerErrorCode`] so guests and diagnostics can
//! distinguish an undeclared origin from a disallowed method without leaking declaration
//! contents.

use crate::declaration::{CompiledRouteGroup, HttpMethod, Origin};
use crate::error::{BrokerError, BrokerErrorCode};

/// Admit a request described by its origin, method, path, and header names.
///
/// Header values are never inspected here; admission needs names only, so credential-bearing
/// values need not exist yet at this stage (named-secret injection happens at send time).
///
/// # Errors
///
/// Returns [`BrokerErrorCode::OriginNotDeclared`], [`BrokerErrorCode::PathNotDeclared`],
/// [`BrokerErrorCode::MethodNotAllowed`], or [`BrokerErrorCode::HeaderNotAllowed`] with stable
/// codes for every denial shape.
pub fn admit<'a>(
    groups: &'a [CompiledRouteGroup],
    request_origin: &str,
    method: HttpMethod,
    request_path: &str,
    request_header_names: &[String],
) -> Result<&'a CompiledRouteGroup, BrokerError> {
    let Ok(parsed_origin) = Origin::parse(request_origin) else {
        return Err(BrokerError::new(BrokerErrorCode::OriginNotDeclared));
    };
    let mut origin_matched: Vec<&CompiledRouteGroup> = Vec::new();
    for group in groups {
        if group
            .origins()
            .iter()
            .any(|declared| declared.matches(&parsed_origin))
        {
            origin_matched.push(group);
        }
    }
    if origin_matched.is_empty() {
        return Err(BrokerError::new(BrokerErrorCode::OriginNotDeclared));
    }
    let path = normalize_request_path(request_path)?;
    let mut path_matched: Vec<&CompiledRouteGroup> = origin_matched
        .iter()
        .copied()
        .filter(|group| group.paths().iter().any(|pattern| pattern.matches(&path)))
        .collect();
    if path_matched.is_empty() {
        return Err(BrokerError::new(BrokerErrorCode::PathNotDeclared));
    }
    path_matched.retain(|group| group.methods().contains(&method));
    if path_matched.is_empty() {
        return Err(BrokerError::new(BrokerErrorCode::MethodNotAllowed));
    }
    let group = path_matched.first().expect("non-empty after retain above");
    for name in request_header_names {
        let lowercase = name.to_ascii_lowercase();
        if !group.allowed_headers().contains(&lowercase) {
            return Err(BrokerError::with_detail(
                BrokerErrorCode::HeaderNotAllowed,
                format!("header `{lowercase}` is not declared"),
            ));
        }
    }
    Ok(group)
}

fn normalize_request_path(path: &str) -> Result<String, BrokerError> {
    if path.is_empty() || !path.starts_with('/') || path.contains('?') || path.contains('#') {
        return Err(BrokerError::new(BrokerErrorCode::PathNotDeclared));
    }
    Ok(path.to_owned())
}
