use lib::Payload;
use uuid::Uuid;

#[derive(Payload)]
pub struct PingInput {
    client_id: Uuid,
    iteration: u32
}

impl PingInput {
    pub fn new(client_id: Uuid, iteration: u32) -> Self {
        Self { client_id, iteration }
    }
    pub fn iteration(&self) -> u32 {
        self.iteration
    }
    pub fn client_id(&self) -> Uuid {
        self.client_id
    }
}

#[derive(Payload)]
pub struct PingOutput {
    iteration: u32,
}

impl PingOutput {
    pub fn new(iteration: u32) -> Self {
        Self { iteration }
    }
    pub fn iteration(&self) -> u32 {
        self.iteration
    }
}