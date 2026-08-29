//! T48 Library surface — real session-backed adapter + session commands.
use studio_design::{library_adapter::LibraryAdapter, LibraryAssetId};
use studio_design::{DesignerSession, DesignerCommand};

pub struct LibraryPanel {
    adapter: Box<dyn LibraryAdapter>,
}

impl LibraryPanel {
    pub fn new(adapter: Box<dyn LibraryAdapter>) -> Self { Self { adapter } }
    pub fn admitted_assets(&self) -> Vec<LibraryAssetId> { self.adapter.admitted() }
    pub fn provenance_for(&self, id: &LibraryAssetId) -> Option<String> { self.adapter.provenance(id) }
    pub fn unsupported_format(&self, detail: &str) -> (String, String) { self.adapter.unsupported_format(detail) }
    pub fn insert_bind(&mut self, id: LibraryAssetId, session: &mut DesignerSession) -> Result<(), String> {
        session.submit(DesignerCommand::InsertLibraryAsset { asset_id: id }).map_err(|e| format!("{:?}", e))
    }
}
