use lib::Payload;
use uuid::Uuid;

#[derive(Payload, Clone)]
pub struct User {
    client_id: Uuid,
    username: String,
}

impl User {
    #[allow(unused)]
    pub fn new(client_id: Uuid, username: &str) -> Self {
        Self {
            client_id,
            username: username.to_string(),
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    #[allow(unused)]
    pub fn client_id(&self) -> &Uuid {
        &self.client_id
    }
}
