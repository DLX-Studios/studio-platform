//! T52 MCP surface — actor activity through session/inspector.
pub struct McpSurface;
impl McpSurface {
    pub fn apply_batch(&self) -> Result<(), String> { Ok(()) }
    pub fn diagnostic_for(&self) -> Option<String> { None }
}
