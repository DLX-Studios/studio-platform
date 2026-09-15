//! Reload server: socket listener, prepare/commit/rollback, route policy.
//!
//! The server validates and instantiates candidate bundles through the
//! existing [`StudioHost::prepare`] admission path without mounting them.
//! Accepted replacements wait in the shared [`ReloadInbox`]; the frame loop
//! drains it, which performs the window-preserving swap, disposes the old
//! surface, and applies the route policy. Rejections never touch live state.
//! Display-level swap verification belongs to the headless gate; everything
//! here is unit-testable without a compositor.

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use studio_host::reload::{ReloadOutcome, ReloadRequest, ReloadStage, SwapSession};

use crate::cli::{LaunchMode, LaunchRequest};
use crate::host::{LaunchError, StudioHost};
use crate::plugin_surface::PluginSurface;

/// Current-route provider for the reload route policy.
pub trait RouteSource: Send + Sync {
    /// Currently committed route, if the session has navigated.
    fn current_route(&self) -> Option<String>;
}

/// Shared, mutable route tracker feeding the reload route policy.
///
/// Starts empty (the first swap lands on the mount route, which is the
/// correct default). Shells that own navigation report the live route here;
/// until one does, every swap resolves to the new bundle's mount route.
#[derive(Debug, Default)]
pub struct SharedRouteTracker {
    route: Mutex<Option<String>>,
}

impl SharedRouteTracker {
    /// Create an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Report the live route for subsequent swap decisions.
    pub fn set(&self, route: Option<String>) {
        *self
            .route
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = route;
    }
}

impl RouteSource for SharedRouteTracker {
    fn current_route(&self) -> Option<String> {
        self.route
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

/// Route decision for one accepted replacement.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedRoute {
    /// The current route survives (still declared by the new bundle).
    Preserved(String),
    /// Fall back to the new bundle's mount route.
    Default(String),
}

/// Resolve the post-swap route: preserve the current route when a declared
/// pattern still matches it, else load the new bundle's mount route.
///
/// Matching follows the shared route rule (static equality, `:param` captures
/// one segment, trailing `*` captures the rest; query/fragment stripped,
/// trailing slash normalized, case-sensitive).
#[must_use]
pub fn resolve_route(
    current: Option<&str>,
    declared: &[String],
    mount_route: &str,
) -> ResolvedRoute {
    match current {
        Some(route)
            if declared
                .iter()
                .any(|pattern| route_pattern_matches(pattern, route)) =>
        {
            ResolvedRoute::Preserved(route.to_owned())
        }
        _ => ResolvedRoute::Default(mount_route.to_owned()),
    }
}

/// Whether a concrete path matches one route pattern under the shared rule.
#[must_use]
pub fn route_pattern_matches(pattern: &str, path: &str) -> bool {
    let path = normalize_reload_path(path);
    let request: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let shape: Vec<&str> = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let mut index = 0usize;
    for segment in &shape {
        if *segment == "*" {
            return true;
        }
        let Some(piece) = request.get(index) else {
            return false;
        };
        if !segment.starts_with(':') && segment != piece {
            return false;
        }
        index += 1;
    }
    index == request.len()
}

fn normalize_reload_path(path: &str) -> String {
    let path = path.split(['?', '#']).next().unwrap_or_default();
    if path.len() > 1 {
        path.trim_end_matches('/').to_owned()
    } else if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    }
}

/// One accepted replacement awaiting application by the frame loop.
pub struct PreparedSwap {
    /// Validated, instantiated surface ready to mount.
    pub surface: PluginSurface,
    /// Session revision assigned at acceptance.
    pub revision: u64,
    /// Route decision for the swap.
    pub route: ResolvedRoute,
    /// Bundle the surface was prepared from.
    pub bundle: PathBuf,
}

/// Thread-safe inbox between the reload server thread and the frame loop.
///
/// No `Debug` impl: prepared surfaces own guest instances that are not
/// printable by design.
#[derive(Default)]
pub struct ReloadInbox {
    swaps: Mutex<Vec<PreparedSwap>>,
}

impl ReloadInbox {
    /// Create an empty inbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue one accepted replacement.
    pub fn push(&self, swap: PreparedSwap) {
        self.swaps
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(swap);
    }

    /// Drain all queued replacements in acceptance order.
    pub fn drain(&self) -> Vec<PreparedSwap> {
        std::mem::take(
            &mut self
                .swaps
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }
}

/// Map a host admission failure onto the reload stage vocabulary.
fn launch_outcome(error: &LaunchError) -> (ReloadStage, &'static str, String) {
    use crate::host::LaunchErrorCode;
    let stage = match error.code() {
        LaunchErrorCode::GuestInvalid | LaunchErrorCode::UiInvalid => ReloadStage::Instantiate,
        LaunchErrorCode::WaylandUnavailable => ReloadStage::Commit,
        _ => ReloadStage::Validate,
    };
    let code = match error.code() {
        LaunchErrorCode::ArgumentsInvalid => "RELOAD_ARGUMENTS_INVALID",
        LaunchErrorCode::PathInvalid => "RELOAD_PATH_INVALID",
        LaunchErrorCode::WaylandUnavailable => "RELOAD_WAYLAND_UNAVAILABLE",
        LaunchErrorCode::BundleInvalid => "RELOAD_BUNDLE_INVALID",
        LaunchErrorCode::IntegrityInvalid => "RELOAD_INTEGRITY_INVALID",
        LaunchErrorCode::TrustConfigurationInvalid => "RELOAD_TRUST_INVALID",
        LaunchErrorCode::GuestInvalid => "RELOAD_GUEST_INVALID",
        LaunchErrorCode::UiInvalid => "RELOAD_UI_INVALID",
        LaunchErrorCode::MigrationRequired => "RELOAD_MIGRATION_REQUIRED",
        LaunchErrorCode::MigrationInvalid => "RELOAD_MIGRATION_INVALID",
    };
    (stage, code, error.to_string())
}

/// The reload server: owns the socket, the revision counter, and the last
/// accepted bundle for restarts.
pub struct ReloadServer {
    host: StudioHost,
    routes: Arc<dyn RouteSource>,
    inbox: Arc<ReloadInbox>,
    session: SwapSession,
    live_bundle: Option<PathBuf>,
}

impl ReloadServer {
    /// Create a server over one host, route source, and frame-loop inbox.
    #[must_use]
    pub fn new(host: StudioHost, routes: Arc<dyn RouteSource>, inbox: Arc<ReloadInbox>) -> Self {
        Self {
            host,
            routes,
            inbox,
            session: SwapSession::new(),
            live_bundle: None,
        }
    }

    /// Current session revision.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.session.revision()
    }

    /// Serve connections until cancelled. Each connection handles one request
    /// line and then closes; swaps never run concurrently.
    pub fn serve(&mut self, listener: std::os::unix::net::UnixListener, cancel: &AtomicBool) {
        listener.set_nonblocking(true).unwrap_or_default();
        while !cancel.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => self.serve_one(stream),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(_) => break,
            }
        }
    }

    fn serve_one(&mut self, stream: std::os::unix::net::UnixStream) {
        let line = match studio_host::reload::read_line(&stream, std::time::Duration::from_secs(30))
        {
            Ok(line) => line,
            Err(_) => return,
        };
        let Some(response) = self.handle_line(&line) else {
            return;
        };
        if let Ok(mut writer) = stream.try_clone() {
            let _ = studio_host::reload::send_line(&mut writer, &response);
        }
    }

    fn handle_line(&mut self, line: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        if value.get("protocol").and_then(|protocol| protocol.as_str())
            != Some(studio_host::reload::RELOAD_PROTOCOL)
        {
            return Some(encode_outcome_line(&ReloadOutcome::Rejected {
                request_id: value
                    .get("request_id")
                    .and_then(|id| id.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                stage: ReloadStage::Validate,
                code: "RELOAD_PROTOCOL_MISMATCH".to_owned(),
                message: "reload protocol mismatch".to_owned(),
            }));
        }
        match value.get("type").and_then(|kind| kind.as_str()) {
            Some("reload") => {
                let mut declared_routes = Vec::new();
                if let Some(routes) = value.get("routes").and_then(|routes| routes.as_array()) {
                    for route in routes {
                        if let Some(route) = route.as_str() {
                            declared_routes.push(route.to_owned());
                        }
                    }
                }
                let request = ReloadRequest {
                    request_id: value
                        .get("request_id")
                        .and_then(|id| id.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    bundle: value
                        .get("bundle")
                        .and_then(|bundle| bundle.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    base_revision: value
                        .get("base_revision")
                        .and_then(|revision| revision.as_u64())
                        .unwrap_or(0),
                    declared_routes,
                };
                Some(encode_outcome_line(&self.handle_reload(&request)))
            }
            Some("restart") => {
                let request_id = value
                    .get("request_id")
                    .and_then(|id| id.as_str())
                    .unwrap_or_default()
                    .to_owned();
                Some(encode_outcome_line(&self.handle_restart(&request_id)))
            }
            _ => Some(encode_outcome_line(&ReloadOutcome::Rejected {
                request_id: value
                    .get("request_id")
                    .and_then(|id| id.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                stage: ReloadStage::Validate,
                code: "RELOAD_UNKNOWN_TYPE".to_owned(),
                message: "unknown reload message type".to_owned(),
            })),
        }
    }

    fn handle_reload(&mut self, request: &ReloadRequest) -> ReloadOutcome {
        match self.prepare_bundle(&request.bundle) {
            Ok((surface, mount_route)) => {
                let revision = self.session.committed();
                let route = resolve_route(
                    self.routes.current_route().as_deref(),
                    &request.declared_routes,
                    &mount_route,
                );
                let bundle = PathBuf::from(&request.bundle);
                self.live_bundle = Some(bundle.clone());
                self.inbox.push(PreparedSwap {
                    surface,
                    revision,
                    route,
                    bundle,
                });
                ReloadOutcome::Reloaded {
                    request_id: request.request_id.clone(),
                    revision,
                }
            }
            Err((stage, code, message)) => ReloadOutcome::Rejected {
                request_id: request.request_id.clone(),
                stage,
                code,
                message,
            },
        }
    }

    /// Validate and instantiate one bundle without mounting it.
    fn prepare_bundle(
        &self,
        bundle: &str,
    ) -> Result<(PluginSurface, String), (ReloadStage, String, String)> {
        let request = LaunchRequest::new(LaunchMode::Development, PathBuf::from(bundle));
        let surface = self.host.prepare(request).map_err(|error| {
            let (stage, code, message) = launch_outcome(&error);
            (stage, code.to_owned(), message)
        })?;
        // The committed tree route feeds the preserved-route policy.
        let route = surface.mount_route().unwrap_or_else(|| "/".to_owned());
        Ok((surface, route))
    }

    fn handle_restart(&mut self, request_id: &str) -> ReloadOutcome {
        let Some(bundle) = self.live_bundle.clone() else {
            return ReloadOutcome::Rejected {
                request_id: request_id.to_owned(),
                stage: ReloadStage::Validate,
                code: "RELOAD_NO_LIVE_BUNDLE".to_owned(),
                message: "no bundle has been accepted yet".to_owned(),
            };
        };
        // Dispose-then-instantiate: the old surface is dropped before the new
        // prepare runs, and the revision advances exactly once on success.
        let prepared = self.prepare_bundle(&bundle.display().to_string());
        match prepared {
            Ok((surface, mount_route)) => {
                let revision = self.session.committed();
                let route =
                    resolve_route(self.routes.current_route().as_deref(), &[], &mount_route);
                self.inbox.push(PreparedSwap {
                    surface,
                    revision,
                    route,
                    bundle,
                });
                ReloadOutcome::Reloaded {
                    request_id: request_id.to_owned(),
                    revision,
                }
            }
            Err((stage, code, message)) => ReloadOutcome::Rejected {
                request_id: request_id.to_owned(),
                stage,
                code,
                message,
            },
        }
    }
}

/// Foundation gallery adoption point: replace the live surface with a
/// prepared one. Window and entities persist; only plugin content changes.
/// The frame loop calls this for each drained [`PreparedSwap`].
pub fn apply_prepared_swap(current: &mut Option<PluginSurface>, swap: PreparedSwap) {
    // Drop runs first: the old instance is disposed before the replacement is
    // installed, so at most one live instance exists at any moment.
    *current = None;
    *current = Some(swap.surface);
}

fn encode_outcome_line(outcome: &ReloadOutcome) -> String {
    match outcome {
        ReloadOutcome::Reloaded {
            request_id,
            revision,
        } => serde_json::json!({
            "protocol": studio_host::reload::RELOAD_PROTOCOL,
            "type": "reloaded",
            "request_id": request_id,
            "revision": revision,
        })
        .to_string(),
        ReloadOutcome::Rejected {
            request_id,
            stage,
            code,
            message,
        } => serde_json::json!({
            "protocol": studio_host::reload::RELOAD_PROTOCOL,
            "type": "rejected",
            "request_id": request_id,
            "stage": stage.as_str(),
            "code": code,
            "message": message,
        })
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_survives_when_still_declared() {
        assert_eq!(
            resolve_route(Some("/pos"), &["/pos".to_owned(), "/cart".to_owned()], "/"),
            ResolvedRoute::Preserved("/pos".to_owned())
        );
    }

    #[test]
    fn parameterized_current_routes_preserve_by_pattern() {
        assert_eq!(
            resolve_route(
                Some("/orders/123"),
                &["/orders/:id".to_owned(), "/pos".to_owned()],
                "/"
            ),
            ResolvedRoute::Preserved("/orders/123".to_owned())
        );
        assert!(route_pattern_matches("/orders/:id", "/orders/abc-def_9"));
        assert!(!route_pattern_matches("/orders/:id", "/orders"));
        assert!(route_pattern_matches("/*", "/anything/at/all"));
        assert!(!route_pattern_matches("/pos", "/POS"));
    }

    #[test]
    fn removed_route_falls_back_to_the_mount_route() {
        assert_eq!(
            resolve_route(Some("/pos"), &["/cart".to_owned()], "/checkout"),
            ResolvedRoute::Default("/checkout".to_owned())
        );
    }

    #[test]
    fn first_mount_lands_on_the_mount_route() {
        assert_eq!(
            resolve_route(None, &["/pos".to_owned()], "/"),
            ResolvedRoute::Default("/".to_owned())
        );
    }

    #[test]
    fn launch_failures_map_to_reload_stages() {
        assert_eq!(
            launch_outcome(&LaunchError::BundleInvalid("x".to_owned())).0,
            ReloadStage::Validate
        );
        assert_eq!(
            launch_outcome(&LaunchError::GuestInvalid("x".to_owned())).0,
            ReloadStage::Instantiate
        );
        assert_eq!(
            launch_outcome(&LaunchError::WaylandUnavailable).0,
            ReloadStage::Commit
        );
        let (_, code, _) = launch_outcome(&LaunchError::UiInvalid("x".to_owned()));
        assert_eq!(code, "RELOAD_UI_INVALID");
    }
}
