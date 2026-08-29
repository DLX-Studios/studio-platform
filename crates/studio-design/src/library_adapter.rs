//! T48 Library adapter seam — persistence/identity interface without studio-host dependency.
use crate::LibraryAssetId;

pub trait LibraryAdapter {
    fn admitted(&self) -> Vec<LibraryAssetId>;
    fn provenance(&self, id: &LibraryAssetId) -> Option<String>;
    fn admit(&mut self, id: LibraryAssetId) -> Result<(), String>;
    fn insert_bind(&mut self, id: LibraryAssetId) -> Result<(), String>;
    fn unsupported_format(&self, detail: &str) -> (String, String);
}
