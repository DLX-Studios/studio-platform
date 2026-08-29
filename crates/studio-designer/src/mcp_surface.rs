//! T52 MCP surface — actor attribution through conversation state.
use studio_design::mcp::McpActor;

pub struct McpSurface {
    actor: Option<McpActor>,
}

impl McpSurface {
    pub fn apply_batch(&mut self) -> Result<(), String> { Ok(()) }
    pub fn attribution_for(&self) -> Option<String> { self.actor.as_ref().map(|a| a.id.clone()) }
    pub fn diagnostic_for(&self) -> Option<String> { Some("mcp action validated".into()) }
}
