//! T48 Library adapter seam — persistence/identity interface without studio-host dependency.
use crate::LibraryAssetId;

/// Host-neutral access to assets admitted into the Studio Library.
pub trait LibraryAdapter {
    /// Return assets admitted for the current Designer session.
    fn admitted(&self) -> Vec<LibraryAssetId>;
    /// Return safe provenance text for an admitted asset.
    fn provenance(&self, id: &LibraryAssetId) -> Option<String>;
    /// Admit an already-validated asset identity into the session library.
    ///
    /// # Errors
    ///
    /// Returns a safe message when the asset cannot be admitted.
    fn admit(&mut self, id: LibraryAssetId) -> Result<(), String>;
    /// Insert and bind an admitted asset through the command engine.
    ///
    /// # Errors
    ///
    /// Returns a safe message when the asset is unavailable or the bind is rejected.
    fn insert_bind(&mut self, id: LibraryAssetId) -> Result<(), String>;
    /// Build the stable diagnostic code and safe detail for a rejected format.
    fn unsupported_format(&self, detail: &str) -> (String, String);
}
