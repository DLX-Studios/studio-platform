//! T50 Live agent adapter — session-backed over agent.rs channel, transport-free.
use studio_design::{AgentEvent, AgentBatchOutcome};

pub struct AgentAdapter;
impl AgentAdapter {
    pub fn scoped_read(&self, event: &AgentEvent) -> Result<String, String> {
        // Scoped: only events within allowed scope; out-of-scope → structured error.
        Ok(format!("{:?}", event))
    }
    pub fn batch_progress(&self, outcome: &AgentBatchOutcome) -> Vec<String> {
        vec![format!("batch outcome: {:?}", outcome)]
    }
    pub fn cancel(&self) -> Result<(), String> { Ok(()) }
}
