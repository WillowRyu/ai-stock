//! A multi-turn conversation: an ordered list of user/assistant messages.
//! Pure — no IO, no async.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Conversation {
    messages: Vec<Message>,
}

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push_user(&mut self, content: impl Into<String>) {
        self.messages.push(Message::user(content));
    }
    pub fn push_assistant(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
    }
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    /// The most recent `n` messages — used to bound how much history is sent
    /// to the model. Returns all messages when `n >= len`, or an empty slice
    /// when `n = 0`.
    pub fn recent(&self, n: usize) -> &[Message] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_records_messages_in_order() {
        let mut c = Conversation::new();
        assert!(c.is_empty());
        c.push_user("hello");
        c.push_assistant("hi there");
        assert_eq!(c.messages().len(), 2);
        assert_eq!(c.messages()[0].role, Role::User);
        assert_eq!(c.messages()[0].content, "hello");
        assert_eq!(c.messages()[1].role, Role::Assistant);
        assert_eq!(c.messages()[1].content, "hi there");
        assert!(!c.is_empty());
    }

    #[test]
    fn recent_returns_only_the_last_n_messages() {
        let mut c = Conversation::new();
        for i in 0..10 {
            c.push_user(format!("m{i}"));
        }
        let tail = c.recent(3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].content, "m7");
        assert_eq!(tail[2].content, "m9");
    }

    #[test]
    fn recent_caps_at_total_length() {
        let mut c = Conversation::new();
        c.push_user("only");
        assert_eq!(c.recent(50).len(), 1);
    }

    #[test]
    fn recent_zero_returns_empty() {
        let mut c = Conversation::new();
        c.push_user("message");
        assert_eq!(c.recent(0).len(), 0);
    }

    #[test]
    fn recent_on_empty_conversation_returns_empty() {
        let c = Conversation::new();
        assert_eq!(c.recent(10).len(), 0);
    }
}
