use crate::chat::protocol::User;
use lib::Payload;
use uuid::Uuid;

#[derive(Payload, Clone)]
pub struct Message {
    message: String,
    sent_by: User,
}

impl Message {
    #[allow(unused)]
    pub fn new(message: &str, sent_by: User) -> Self {
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
    pub fn sent_by(&self) -> &User {
        &self.sent_by
    }
}

#[derive(Payload)]
pub struct SendMessageInput {
    message: String,
    client_id: Uuid,
}

impl SendMessageInput {
    #[allow(unused)]
    pub fn new(message: &str, client_id: Uuid) -> Self {
        Self {
            message: message.to_string(),
            client_id,
        }
    }

    #[allow(unused)]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[allow(unused)]
    pub fn client_id(&self) -> &Uuid {
        &self.client_id
    }
}
