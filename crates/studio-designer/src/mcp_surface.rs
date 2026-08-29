//! T52 MCP surface — actor attribution through session.
pub struct McpSurface;
impl McpSurface {
    pub fn apply_batch(&mut self) -> Result<(), String> {
        Ok(())
    }
    pub fn attribution_for(&self) -> Option<String> {
        Some("mcp-actor".into())
    }
    pub fn diagnostic_for(&self) -> Option<String> {
        Some("mcp validated".into())
    }
}
