use crate::chat::protocol::User;
use lib::Payload;

#[derive(Payload, Clone)]
pub struct Message {
    message: String,
    sent_by: Option<User>,
}

impl Message {
    #[allow(unused)]
    pub fn new(message: &str, sent_by: Option<User>) -> Self {
        Self {
            message: message.to_string(),
            sent_by,
        }
    }

    #[allow(unused)]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[allow(unused)]
    pub fn sent_by(&self) -> Option<&User> {
        self.sent_by.as_ref()
    }
}

#[derive(Payload)]
pub struct SendMessageInput {
    message: String,
}

impl SendMessageInput {
    #[allow(unused)]
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    #[allow(unused)]
    pub fn message(&self) -> &str {
        &self.message
    }
}
