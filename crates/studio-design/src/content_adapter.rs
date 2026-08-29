//! T49 Content adapter seam.
use crate::{CollectionId, RecordId};

pub trait ContentAdapter {
    fn collections(&self) -> Vec<CollectionId>;
    fn edit_collection(&mut self, id: CollectionId, schema: &str) -> Result<(), String>;
    fn record_states(&self, id: CollectionId) -> Vec<RecordId>;
    fn validate_form(&self, id: RecordId) -> Vec<String>;
}
