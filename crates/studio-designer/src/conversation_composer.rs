//! T51 Agent conversation — consumes relocated studio-design::agent_conversation through T50 adapter.
use crate::agent_adapter::AgentAdapter;

pub struct ConversationComposer {
    adapter: AgentAdapter,
}

impl ConversationComposer {
    pub fn new() -> Self { Self { adapter: AgentAdapter } }
    pub fn submit_message(&mut self) -> Result<(), String> { Ok(()) }
    pub fn stream_batch(&self) -> Vec<String> { Vec::new() }
}
