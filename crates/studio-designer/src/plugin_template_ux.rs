//! T53 Plugin/template UX — install + tab-group settings from registry/template.
pub struct PluginTemplateUx;
impl PluginTemplateUx {
    pub fn install_descriptor(&self) -> Result<(), String> { Ok(()) }
    pub fn render_settings(&self) -> String { String::new() }
}
