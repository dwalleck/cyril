/// A streaming or complete message from the agent.
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub text: String,
    pub is_streaming: bool,
}

/// A thought/reasoning block from the agent (usually collapsed in UI).
#[derive(Debug, Clone)]
pub struct AgentThought {
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_message_streaming() {
        let msg = AgentMessage { text: "Hello".into(), is_streaming: true };
        assert_eq!(msg.text, "Hello");
        assert!(msg.is_streaming);
    }

    #[test]
    fn agent_message_complete() {
        let msg = AgentMessage { text: "Done".into(), is_streaming: false };
        assert!(!msg.is_streaming);
    }

    #[test]
    fn agent_thought_construction() {
        let thought = AgentThought { text: "Thinking...".into() };
        assert_eq!(thought.text, "Thinking...");
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_clone<T: Clone>() {}

    #[test]
    fn message_types_are_send_sync_clone() {
        assert_send::<AgentMessage>();
        assert_sync::<AgentMessage>();
        assert_clone::<AgentMessage>();
        assert_send::<AgentThought>();
        assert_sync::<AgentThought>();
        assert_clone::<AgentThought>();
    }
}
