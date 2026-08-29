//! T48 Library surface — admission, browse, insert/bind, diagnostics.
pub struct LibraryPanel;
impl LibraryPanel {
    pub fn admitted_assets(&self) -> Vec<String> { Vec::new() }
    pub fn provenance_for(&self, _id: &str) -> Option<String> { None }
    pub fn insert_bind(&self, _asset: String) -> Result<(), String> { Ok(()) }
    pub fn unsupported_diagnostic(&self) -> (String, String) { ("UNKNOWN_FORMAT".into(), "format unsupported".into()) }
}
