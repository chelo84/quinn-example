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
}