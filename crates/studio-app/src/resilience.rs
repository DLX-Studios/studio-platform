//! Pre-editor entry points for conflict and recovery centers.
//!
//! The native shell owns these routes so a failed project open never traps a
//! user inside the editor.  The routes are intentionally independent of a
//! cloud client: ticket 57 can supply a sync provider behind the
//! `studio-design` persistence traits without changing this access seam.

use std::{error::Error, fmt};

/// Center that a trusted shell should present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResilienceCenter {
    /// Review and resolve competing local/remote authoring intents.
    Conflicts,
    /// Restore logical snapshots, migrate, or inspect quarantine state.
    Recovery,
}

/// Product surface from which a center was opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResilienceEntryPoint {
    /// Authenticated project dashboard, before a project is opened.
    Dashboard,
    /// A project's settings surface, which may be opened before its editor.
    ProjectSettings,
}

/// A validated route request passed from dashboard or project settings to the native shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResilienceRoute {
    center: ResilienceCenter,
    entry_point: ResilienceEntryPoint,
    project_id: Option<String>,
    path: String,
}

impl ResilienceRoute {
    /// Build a dashboard route that does not require opening a project first.
    #[must_use]
    pub fn from_dashboard(center: ResilienceCenter) -> Self {
        let suffix = match center {
            ResilienceCenter::Conflicts => "conflicts",
            ResilienceCenter::Recovery => "recovery",
        };
        Self {
            center,
            entry_point: ResilienceEntryPoint::Dashboard,
            project_id: None,
            path: format!("/dashboard/{suffix}"),
        }
    }

    /// Build a project-settings route while the project remains unopened.
    ///
    /// # Errors
    ///
    /// Rejects an empty, control-bearing, or route-delimiting project identity.
    pub fn from_project_settings(
        project_id: impl Into<String>,
        center: ResilienceCenter,
    ) -> Result<Self, ResilienceRouteError> {
        let project_id = project_id.into();
        if project_id.is_empty()
            || project_id.len() > 128
            || project_id.chars().any(char::is_control)
            || project_id.contains(['/', '\\', '?', '#'])
        {
            return Err(ResilienceRouteError::InvalidProjectId);
        }
        let suffix = match center {
            ResilienceCenter::Conflicts => "conflicts",
            ResilienceCenter::Recovery => "recovery",
        };
        Ok(Self {
            center,
            entry_point: ResilienceEntryPoint::ProjectSettings,
            project_id: Some(project_id.clone()),
            path: format!("/projects/{project_id}/settings/{suffix}"),
        })
    }

    /// Center selected by this request.
    #[must_use]
    pub const fn center(&self) -> ResilienceCenter {
        self.center
    }

    /// Surface from which this request originated.
    #[must_use]
    pub const fn entry_point(&self) -> ResilienceEntryPoint {
        self.entry_point
    }

    /// Project identity for a settings route, absent for dashboard routes.
    #[must_use]
    pub fn project_id(&self) -> Option<&str> {
        self.project_id.as_deref()
    }

    /// Canonical route consumed by the native navigation shell.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Safe route-construction rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResilienceRouteError {
    /// The project ID cannot be inserted into a concrete route safely.
    InvalidProjectId,
}

impl fmt::Display for ResilienceRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("resilience project identity is invalid")
    }
}

impl Error for ResilienceRouteError {}
