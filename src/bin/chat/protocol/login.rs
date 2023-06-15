use lib::Payload;
use uuid::Uuid;

#[derive(Payload)]
pub struct LoginInput {
    username: String,
}

impl LoginInput {
    #[allow(unused)]
    pub fn new(username: &str) -> Self {
        Self {
            username: username.to_string(),
        }
    }

    #[allow(unused)]
    pub fn username(&self) -> &str {
        &self.username
    }
}

#[derive(Payload)]
pub struct LoginOutput {
    client_id: Uuid,
}

impl LoginOutput {
    #[allow(unused)]
    pub fn new(client_id: Uuid) -> Self {
        Self { client_id }
    }

    #[allow(unused)]
    pub fn client_id(&self) -> &Uuid {
        &self.client_id
    }
}
