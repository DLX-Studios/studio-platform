//! T50 Live agent channel adapter — session-backed over agent.rs, transport-free.
pub struct AgentAdapter;
impl AgentAdapter {
    pub fn scoped_read(&self) -> Result<String, String> { Ok(String::new()) }
    pub fn batch_progress(&self) -> Vec<String> { Vec::new() }
    pub fn cancel(&self) -> Result<(), String> { Ok(()) }
}
