//! T49 Content editors — session-backed collection and typed-form editing.
//!
//! Every mutation is submitted as an ordinary tracked [`CommandBatch`] through
//! the host-independent [`DesignerSession`] authority; the editor never mutates
//! design state directly and every failure is returned as a safe, structured
//! error instead of partially mutating the session.
use std::collections::BTreeMap;

use studio_design::{
    Actor, BatchConflict, CollectionId, Command, CommandBatch, CommandOutcome, CommandReceipt,
    ContentCollection, ContentCollectionSchema, ContentRecord, DesignerDiagnostic, DesignerQuery,
    DesignerQueryResult, DesignerSession, FormDefinition, FormId, FormValidationResult,
    OperationId, PropertyValue, RecordId, STUDIO_DESIGN_SCHEMA_VERSION, SessionStateSnapshot,
    UndoGroupId,
};
use thiserror::Error;

/// Safe failure modes for session-backed content editing.
#[derive(Debug, Error)]
pub enum ContentEditorError {
    /// The collection referenced by the edit does not exist.
    #[error("content collection {0} does not exist")]
    CollectionMissing(CollectionId),
    /// The form referenced by the validation request does not exist.
    #[error("form {0} does not exist")]
    FormMissing(FormId),
    /// The session authority rejected the command batch.
    #[error("content command batch was rejected")]
    Rejected(Vec<DesignerDiagnostic>),
    /// The command batch was built on a stale base revision.
    #[error("content command batch conflicts with the session authority")]
    Conflict(BatchConflict),
    /// The durable commit behind the command batch failed.
    #[error("content command persistence failed: {0}")]
    Persistence(String),
}

/// Session-backed editor for typed content collections and declarative forms.
#[derive(Debug, Default, Clone, Copy)]
pub struct ContentEditor;

impl ContentEditor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Return every content collection in the session's current revision.
    pub fn collections(&self, session: &dyn DesignerSession) -> Vec<ContentCollection> {
        match session.query(DesignerQuery::Collections) {
            DesignerQueryResult::Collections(collections) => collections,
            _ => Vec::new(),
        }
    }

    /// Return one content collection, or an error when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`ContentEditorError::CollectionMissing`] when the session has
    /// no collection with the requested identity.
    pub fn collection(
        &self,
        session: &dyn DesignerSession,
        collection_id: &CollectionId,
    ) -> Result<ContentCollection, ContentEditorError> {
        match session.query(DesignerQuery::Collection {
            collection_id: collection_id.clone(),
        }) {
            DesignerQueryResult::Collection(Some(collection)) => Ok(collection),
            DesignerQueryResult::Collection(None) => {
                Err(ContentEditorError::CollectionMissing(collection_id.clone()))
            }
            _ => Err(ContentEditorError::CollectionMissing(collection_id.clone())),
        }
    }

    /// Return every declarative form in the session's current revision.
    pub fn forms(&self, session: &dyn DesignerSession) -> Vec<FormDefinition> {
        match session.query(DesignerQuery::Forms) {
            DesignerQueryResult::Forms(forms) => forms,
            _ => Vec::new(),
        }
    }

    /// Evaluate a declarative form without mutating the session.
    ///
    /// # Errors
    ///
    /// Returns [`ContentEditorError::FormMissing`] when the form does not exist.
    pub fn validate_form(
        &self,
        session: &dyn DesignerSession,
        form_id: &FormId,
        values: BTreeMap<String, PropertyValue>,
    ) -> Result<FormValidationResult, ContentEditorError> {
        if !self.forms(session).iter().any(|form| &form.id == form_id) {
            return Err(ContentEditorError::FormMissing(form_id.clone()));
        }
        match session.query(DesignerQuery::ValidateForm {
            form_id: form_id.clone(),
            values,
        }) {
            DesignerQueryResult::FormValidation(result) => Ok(result),
            _ => Err(ContentEditorError::FormMissing(form_id.clone())),
        }
    }

    /// Create a typed content collection through a tracked command.
    ///
    /// # Errors
    ///
    /// Returns the structured session rejection when the batch is invalid.
    pub async fn create_collection(
        &self,
        session: &mut dyn DesignerSession,
        collection: ContentCollection,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandReceipt, ContentEditorError> {
        let state = session_state(session);
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: state.project_id,
            base_revision: state.revision_id,
            undo_group_id,
            undo_group_name: format!("Create collection {}", collection.name),
            preconditions: Vec::new(),
            commands: vec![Command::CreateCollection { collection }],
        };
        outcome(session.submit(batch).await)
    }

    /// Replace a collection's typed schema through a tracked command.
    ///
    /// # Errors
    ///
    /// Returns [`ContentEditorError::CollectionMissing`] when the collection
    /// does not exist, or the structured session rejection otherwise.
    pub async fn update_collection_schema(
        &self,
        session: &mut dyn DesignerSession,
        collection_id: CollectionId,
        schema: ContentCollectionSchema,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandReceipt, ContentEditorError> {
        let name = self.collection(session, &collection_id)?.name;
        let state = session_state(session);
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: state.project_id,
            base_revision: state.revision_id,
            undo_group_id,
            undo_group_name: format!("Update collection {name} schema"),
            preconditions: Vec::new(),
            commands: vec![Command::UpdateCollectionSchema {
                collection_id,
                schema,
            }],
        };
        outcome(session.submit(batch).await)
    }

    /// Create a schema-validated record through a tracked command.
    ///
    /// # Errors
    ///
    /// Returns [`ContentEditorError::CollectionMissing`] when the collection
    /// does not exist, or the structured session rejection otherwise.
    pub async fn create_record(
        &self,
        session: &mut dyn DesignerSession,
        collection_id: CollectionId,
        record: ContentRecord,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandReceipt, ContentEditorError> {
        self.collection(session, &collection_id)?;
        let state = session_state(session);
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: state.project_id,
            base_revision: state.revision_id,
            undo_group_id,
            undo_group_name: format!("Create record in {}", collection_id.as_str()),
            preconditions: Vec::new(),
            commands: vec![Command::CreateRecord {
                collection_id,
                record,
            }],
        };
        outcome(session.submit(batch).await)
    }

    /// Delete a record through a tracked command.
    ///
    /// # Errors
    ///
    /// Returns [`ContentEditorError::CollectionMissing`] when the collection
    /// does not exist, or the structured session rejection otherwise.
    pub async fn delete_record(
        &self,
        session: &mut dyn DesignerSession,
        collection_id: CollectionId,
        record_id: RecordId,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandReceipt, ContentEditorError> {
        self.collection(session, &collection_id)?;
        let state = session_state(session);
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: state.project_id,
            base_revision: state.revision_id,
            undo_group_id,
            undo_group_name: format!("Delete record in {}", collection_id.as_str()),
            preconditions: Vec::new(),
            commands: vec![Command::DeleteRecord {
                collection_id,
                record_id,
            }],
        };
        outcome(session.submit(batch).await)
    }
}

fn session_state(session: &dyn DesignerSession) -> SessionStateSnapshot {
    match session.query(DesignerQuery::SessionState) {
        DesignerQueryResult::SessionState(state) => state,
        _ => unreachable!("DesignerSession returned the wrong query result"),
    }
}

fn outcome(outcome: CommandOutcome) -> Result<CommandReceipt, ContentEditorError> {
    match outcome {
        CommandOutcome::Accepted(receipt) => Ok(receipt),
        CommandOutcome::Rejected(diagnostics) => Err(ContentEditorError::Rejected(diagnostics)),
        CommandOutcome::Conflict(conflict) => Err(ContentEditorError::Conflict(conflict)),
        CommandOutcome::PersistenceFailed(error) => {
            Err(ContentEditorError::Persistence(error.to_string()))
        }
    }
}
