//! T49 Content editors — session-backed collection/form/validation.
use studio_design::content_adapter::ContentAdapter;
use studio_design::{CollectionId, DesignerSession};

pub struct ContentEditor {
    adapter: Box<dyn ContentAdapter>,
}

impl ContentEditor {
    pub fn new(a: Box<dyn ContentAdapter>) -> Self {
        Self { adapter: a }
    }
    pub fn edit_collection(
        &mut self,
        id: CollectionId,
        _session: &mut dyn DesignerSession,
    ) -> Result<(), String> {
        self.adapter.edit_collection(id, "schema")
    }
    pub fn validate_form(&self) -> Vec<String> {
        let Some(collection) = self.adapter.collections().first().cloned() else {
            return Vec::new();
        };
        let Some(record) = self.adapter.record_states(collection).first().cloned() else {
            return Vec::new();
        };
        self.adapter.validate_form(record)
    }
}
