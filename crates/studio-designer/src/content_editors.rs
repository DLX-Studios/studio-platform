//! T49 Content editors — session-backed collection/form/validation.
use studio_design::{DesignerSession, DesignerCommand, CollectionId};
use studio_design::content_adapter::ContentAdapter;

pub struct ContentEditor {
    adapter: Box<dyn ContentAdapter>,
}

impl ContentEditor {
    pub fn new(a: Box<dyn ContentAdapter>) -> Self { Self { adapter: a } }
    pub fn edit_collection(&mut self, id: CollectionId, _session: &mut DesignerSession) -> Result<(), String> {
        self.adapter.edit_collection(id, "schema")
    }
    pub fn validate_form(&self) -> Vec<String> { self.adapter.validate_form(self.adapter.collections().first().cloned().unwrap_or(CollectionId::new(""))) }
}
