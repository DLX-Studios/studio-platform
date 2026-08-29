//! T49 Content editors — collection/binding/form over content model.
pub struct ContentEditor;
impl ContentEditor {
    pub fn edit_collection(&self) -> Result<(), String> { Ok(()) }
    pub fn validate_form(&self) -> Vec<String> { Vec::new() }
}
