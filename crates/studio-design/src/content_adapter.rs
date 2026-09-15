//! T49 Content adapter seam.
use crate::{CollectionId, RecordId};

/// Host-neutral access to authored content collections and form validation.
pub trait ContentAdapter {
    /// Return the collections visible to the current Designer session.
    fn collections(&self) -> Vec<CollectionId>;
    /// Replace the schema for a collection after adapter-level validation.
    ///
    /// # Errors
    ///
    /// Returns a safe message when the collection or schema is invalid.
    fn edit_collection(&mut self, id: CollectionId, schema: &str) -> Result<(), String>;
    /// Return the fixture or authored records available for a collection.
    fn record_states(&self, id: CollectionId) -> Vec<RecordId>;
    /// Return deterministic validation messages for an authored form record.
    fn validate_form(&self, id: RecordId) -> Vec<String>;
}
