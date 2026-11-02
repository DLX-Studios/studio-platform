//! Host-independent editor state for one local project source file.

use thiserror::Error;

use crate::source_events::SourceChangeEvent;
use crate::source_workspace::FileSnapshot;

/// A remote update's effect on the current source buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRefresh {
    /// A clean buffer adopted the newly saved project file.
    Refreshed,
    /// The local buffer was preserved and now conflicts with the saved file.
    Conflict,
}

/// Local and incoming versions retained for explicit conflict resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceConflict {
    /// Saved source the current buffer was based on.
    pub base: FileSnapshot,
    /// Current unsaved local buffer contents.
    pub local_content: String,
    /// New contents currently saved in the project.
    pub incoming: FileSnapshot,
    /// MCP client display name associated with the incoming write.
    pub client_name: String,
}

/// A complete file replacement guarded by the revision the caller observed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionGuardedWrite {
    /// Project-relative target path.
    pub path: String,
    /// Replacement source contents.
    pub content: String,
    /// SHA-256 revision that must still be on disk for the write to succeed.
    pub expected_revision: String,
    /// Buffer contents at the time the write was requested.
    pub buffer_at_request: String,
}

/// Failure while reconciling a local source buffer.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SourceViewError {
    /// An event or snapshot belongs to another source file.
    #[error("source update path does not match the open document")]
    PathMismatch,
    /// The delivered file snapshot is not the event's claimed result revision.
    #[error("source event revision does not match the saved file snapshot")]
    EventRevisionMismatch,
    /// A previous version is not available for a safe revert.
    #[error("the latest source change has no prior file contents to restore")]
    NoRevertContent,
    /// The file changed again after the agent edit that would be reverted.
    #[error("the source file changed again; reread it before reverting")]
    StaleRevert,
    /// A saved snapshot does not belong to this source document.
    #[error("saved source snapshot path does not match the open document")]
    SavedPathMismatch,
    /// A buffer has unresolved changes from both the local user and the project file.
    #[error("resolve the incoming source conflict before saving this buffer")]
    ConflictUnresolved,
    /// The local buffer already matches the saved source.
    #[error("the source buffer has no unsaved changes")]
    NoChanges,
}

/// One editor buffer and the last revision it observed on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceView {
    base: FileSnapshot,
    saved: FileSnapshot,
    buffer: String,
    conflict: Option<SourceConflict>,
    latest_agent: Option<String>,
    latest_agent_event: Option<SourceChangeEvent>,
}

impl SourceView {
    /// Open a clean editor buffer from a source workspace snapshot.
    #[must_use]
    pub fn open(snapshot: FileSnapshot) -> Self {
        Self {
            base: snapshot.clone(),
            saved: snapshot.clone(),
            buffer: snapshot.content,
            conflict: None,
            latest_agent: None,
            latest_agent_event: None,
        }
    }

    /// Project-relative path of the open document.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.saved.path
    }

    /// Current editable text buffer.
    #[must_use]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Most recently observed file contents and revision on disk.
    #[must_use]
    pub fn saved_snapshot(&self) -> &FileSnapshot {
        &self.saved
    }

    /// Snapshot against which the current buffer was last rebased.
    #[must_use]
    pub fn base_snapshot(&self) -> &FileSnapshot {
        &self.base
    }

    /// Current external-change conflict, if one needs resolution.
    #[must_use]
    pub fn conflict(&self) -> Option<&SourceConflict> {
        self.conflict.as_ref()
    }

    /// Most recent MCP client label associated with a successful write.
    #[must_use]
    pub fn latest_agent(&self) -> Option<&str> {
        self.latest_agent.as_deref()
    }

    /// Whether the local text diverges from its base or a conflict is open.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.conflict.is_some() || self.buffer != self.base.content
    }

    /// Replace the local text while preserving any open conflict details.
    pub fn replace_buffer(&mut self, content: impl Into<String>) {
        self.buffer = content.into();
        if let Some(conflict) = &mut self.conflict {
            conflict.local_content.clone_from(&self.buffer);
        }
    }

    /// Reconcile an agent write with the current file contents.
    ///
    /// A clean buffer adopts the saved file immediately. A dirty buffer keeps
    /// its text and records the base, local, and incoming versions together.
    pub fn observe_agent_change(
        &mut self,
        event: SourceChangeEvent,
        current: FileSnapshot,
    ) -> Result<SourceRefresh, SourceViewError> {
        if event.path != self.path() || current.path != self.path() {
            return Err(SourceViewError::PathMismatch);
        }
        if current.revision != event.after_revision {
            return Err(SourceViewError::EventRevisionMismatch);
        }

        let was_dirty = self.is_dirty();
        self.latest_agent = Some(event.client_name.clone());
        self.latest_agent_event = Some(event.clone());
        self.saved = current.clone();
        if was_dirty {
            self.conflict = Some(SourceConflict {
                base: self.base.clone(),
                local_content: self.buffer.clone(),
                incoming: current,
                client_name: event.client_name,
            });
            Ok(SourceRefresh::Conflict)
        } else {
            self.base = current.clone();
            self.buffer.clone_from(&current.content);
            self.conflict = None;
            Ok(SourceRefresh::Refreshed)
        }
    }

    /// Build a guarded save for the current local buffer.
    pub fn save_plan(&self) -> Result<RevisionGuardedWrite, SourceViewError> {
        if self.conflict.is_some() {
            return Err(SourceViewError::ConflictUnresolved);
        }
        if !self.is_dirty() {
            return Err(SourceViewError::NoChanges);
        }
        Ok(RevisionGuardedWrite {
            path: self.path().to_owned(),
            content: self.buffer.clone(),
            expected_revision: self.base.revision.clone(),
            buffer_at_request: self.buffer.clone(),
        })
    }

    /// Build a guarded write that restores the latest agent edit's prior text.
    pub fn revert_plan(&self) -> Result<RevisionGuardedWrite, SourceViewError> {
        let event = self
            .latest_agent_event
            .as_ref()
            .ok_or(SourceViewError::NoRevertContent)?;
        if event.path != self.path() {
            return Err(SourceViewError::PathMismatch);
        }
        let before_content = event
            .before_content
            .as_ref()
            .ok_or(SourceViewError::NoRevertContent)?;
        if event.after_revision != self.saved.revision {
            return Err(SourceViewError::StaleRevert);
        }
        Ok(RevisionGuardedWrite {
            path: event.path.clone(),
            content: before_content.clone(),
            expected_revision: event.after_revision.clone(),
            buffer_at_request: self.buffer.clone(),
        })
    }

    /// Accept the currently saved file and discard local or conflicting text.
    pub fn accept_disk(&mut self) {
        self.base = self.saved.clone();
        self.buffer.clone_from(&self.saved.content);
        self.conflict = None;
    }

    /// Keep the local buffer and rebase it onto the latest saved file.
    ///
    /// The buffer remains dirty when it differs from the incoming version, so
    /// its next save is guarded against the revision the user just reviewed.
    pub fn keep_local(&mut self) {
        self.base = self.saved.clone();
        self.conflict = None;
    }

    /// Rebase after a guarded file write succeeds.
    ///
    /// If the user continued typing while the write was in flight, the newer
    /// buffer remains intact and dirty against the newly saved snapshot.
    pub fn accept_saved(
        &mut self,
        snapshot: FileSnapshot,
        buffer_at_request: &str,
    ) -> Result<(), SourceViewError> {
        if snapshot.path != self.path() {
            return Err(SourceViewError::SavedPathMismatch);
        }
        if self.buffer == buffer_at_request {
            self.buffer.clone_from(&snapshot.content);
        }
        self.saved = snapshot.clone();
        self.base = snapshot.clone();
        self.conflict = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(revision: &str, content: &str) -> FileSnapshot {
        FileSnapshot {
            path: "app.studio".to_owned(),
            revision: revision.to_owned(),
            content: content.to_owned(),
        }
    }

    fn event(before: &str, after: &str, before_content: &str) -> SourceChangeEvent {
        SourceChangeEvent::new(
            "app.studio",
            Some(before.to_owned()),
            Some(before_content.to_owned()),
            after.to_owned(),
            Some("Codex"),
        )
    }

    #[test]
    fn clean_buffer_refreshes_from_an_agent_change() {
        let mut view = SourceView::open(snapshot("a", "original"));
        let result = view
            .observe_agent_change(event("a", "b", "original"), snapshot("b", "agent edit"))
            .unwrap();
        assert_eq!(result, SourceRefresh::Refreshed);
        assert_eq!(view.buffer(), "agent edit");
        assert_eq!(view.base_snapshot().revision, "b");
        assert_eq!(view.latest_agent(), Some("Codex"));
        assert!(!view.is_dirty());
    }

    #[test]
    fn dirty_buffer_is_preserved_as_a_three_way_conflict() {
        let mut view = SourceView::open(snapshot("a", "original"));
        view.replace_buffer("local edit");
        let result = view
            .observe_agent_change(event("a", "b", "original"), snapshot("b", "agent edit"))
            .unwrap();
        assert_eq!(result, SourceRefresh::Conflict);
        assert_eq!(view.buffer(), "local edit");
        assert!(view.is_dirty());
        let conflict = view.conflict().unwrap();
        assert_eq!(conflict.base.content, "original");
        assert_eq!(conflict.local_content, "local edit");
        assert_eq!(conflict.incoming.content, "agent edit");
        assert_eq!(conflict.client_name, "Codex");
    }

    #[test]
    fn accept_disk_discards_the_buffer_and_guarded_revert_uses_latest_revision() {
        let mut view = SourceView::open(snapshot("a", "original"));
        view.replace_buffer("local edit");
        view.observe_agent_change(event("a", "b", "original"), snapshot("b", "agent edit"))
            .unwrap();
        view.accept_disk();
        assert_eq!(view.buffer(), "agent edit");
        assert!(!view.is_dirty());

        let revert = view.revert_plan().unwrap();
        assert_eq!(revert.path, "app.studio");
        assert_eq!(revert.content, "original");
        assert_eq!(revert.expected_revision, "b");
        view.accept_saved(snapshot("c", "original"), &revert.buffer_at_request)
            .unwrap();
        assert_eq!(view.buffer(), "original");
        assert!(!view.is_dirty());
    }

    #[test]
    fn keep_local_resolves_a_conflict_against_the_incoming_revision() {
        let mut view = SourceView::open(snapshot("a", "original"));
        view.replace_buffer("local edit");
        view.observe_agent_change(event("a", "b", "original"), snapshot("b", "agent edit"))
            .unwrap();

        view.keep_local();

        assert_eq!(view.buffer(), "local edit");
        assert!(view.conflict().is_none());
        assert!(view.is_dirty());
        assert_eq!(view.base_snapshot().revision, "b");
        assert_eq!(view.save_plan().unwrap().expected_revision, "b");
    }

    #[test]
    fn successful_guarded_write_clears_conflict_but_keeps_newer_typing() {
        let mut view = SourceView::open(snapshot("a", "original"));
        view.replace_buffer("local edit");
        view.observe_agent_change(event("a", "b", "original"), snapshot("b", "agent edit"))
            .unwrap();
        let plan = view.revert_plan().unwrap();
        view.replace_buffer("continued typing");

        view.accept_saved(snapshot("c", &plan.content), &plan.buffer_at_request)
            .unwrap();

        assert_eq!(view.buffer(), "continued typing");
        assert!(view.conflict().is_none());
        assert!(view.is_dirty());
        assert_eq!(view.base_snapshot().revision, "c");
    }

    #[test]
    fn revert_refuses_a_file_that_changed_after_the_agent_edit() {
        let mut view = SourceView::open(snapshot("a", "original"));
        view.observe_agent_change(event("a", "b", "original"), snapshot("b", "agent edit"))
            .unwrap();
        view.observe_agent_change(event("b", "c", "agent edit"), snapshot("c", "newer edit"))
            .unwrap();
        assert_eq!(view.revert_plan().unwrap().expected_revision, "c");
        view.saved = snapshot("d", "outside edit");
        assert_eq!(view.revert_plan(), Err(SourceViewError::StaleRevert));
    }
}
