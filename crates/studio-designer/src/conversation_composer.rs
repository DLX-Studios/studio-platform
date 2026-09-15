//! T51 Agent conversation — session-backed composer over the relocated
//! `studio-design::agent_conversation` domain state.
//!
//! The composer owns the welcome-to-thread state machine and projects it for
//! the native shell. Message flow, validation, and run lifecycle stay in the
//! domain state so the same rules govern agents, MCP, and tests.
use studio_design::agent_conversation::{
    AgentConversationState, AgentMessage, AgentMessageId, AgentReference, AgentRunId,
    ConversationInputError, ConversationSurface, ModelSelection,
};

/// Welcome-to-thread composer backed by the Studio agent conversation domain.
#[derive(Debug, Default)]
pub struct ConversationComposer {
    state: AgentConversationState,
}

impl ConversationComposer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Read-only access to the full conversation domain state.
    #[must_use]
    pub const fn state(&self) -> &AgentConversationState {
        &self.state
    }

    /// Mutable access for shell-level composition concerns.
    pub const fn state_mut(&mut self) -> &mut AgentConversationState {
        &mut self.state
    }

    /// Current presentation surface (welcome before the first accepted message).
    #[must_use]
    pub fn surface(&self) -> ConversationSurface {
        self.state.surface()
    }

    /// Current composer draft text.
    #[must_use]
    pub fn draft(&self) -> &str {
        self.state.composer.draft.as_str()
    }

    /// Set the composer draft through the domain validation rules.
    ///
    /// # Errors
    ///
    /// Returns the domain error for empty or oversized drafts.
    pub fn set_draft(&mut self, draft: impl Into<String>) -> Result<(), ConversationInputError> {
        self.state.composer.set_draft(draft)
    }

    /// Choose the model captured when the next run starts.
    ///
    /// # Errors
    ///
    /// Returns the domain error when the selection is not in the catalog.
    pub fn select_model(
        &mut self,
        selection: ModelSelection,
    ) -> Result<(), ConversationInputError> {
        self.state.select_model(selection)
    }

    /// Submit the drafted message and start (or append to) the thread run.
    ///
    /// # Errors
    ///
    /// Returns the domain error for invalid text or unknown run identity.
    pub fn submit_draft(
        &mut self,
        message_id: AgentMessageId,
        run_id: AgentRunId,
    ) -> Result<ConversationSurface, ConversationInputError> {
        self.state.submit_message(message_id, run_id)?;
        Ok(self.state.surface())
    }

    /// Append an assistant response to the active run.
    ///
    /// # Errors
    ///
    /// Returns the domain error when no active run exists.
    pub fn receive_assistant(
        &mut self,
        message_id: AgentMessageId,
        content: impl Into<String>,
        references: Vec<AgentReference>,
    ) -> Result<(), ConversationInputError> {
        self.state
            .push_assistant_message(message_id, content, references)
    }

    /// Copy of the active run's transcript; empty before the thread starts.
    #[must_use]
    pub fn active_transcript(&self) -> Vec<AgentMessage> {
        let Some(active) = self.state.active_run_id.as_ref() else {
            return Vec::new();
        };
        self.state
            .runs
            .iter()
            .filter(|run| &run.id == active)
            .flat_map(|run| run.messages.iter().cloned())
            .collect()
    }
}
