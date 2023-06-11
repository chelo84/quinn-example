use lib::Payload;
use uuid::Uuid;

#[derive(Payload)]
pub struct LoginInput {
    username: String,
    password: String,
}

impl LoginInput {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
    pub fn username(&self) -> &str {
        &self.username
    }
    pub fn password(&self) -> &str {
        &self.password
    }
}

#[derive(Payload)]
pub struct LoginOutput {
    client_id: Uuid,
}

impl LoginOutput {
    pub fn client_id(&self) -> Uuid {
        self.client_id
    }
    pub fn new(client_id: Uuid) -> Self {
        Self { client_id }
    }
}
