mod login;
mod message;
mod user;

pub use login::{LoginInput, LoginOutput};
pub use message::{Message, SendMessageInput};
use num_derive::{FromPrimitive, ToPrimitive};
pub use user::User;

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, ToPrimitive, FromPrimitive)]
pub enum ClientCommand {
    /// Payload = [Vec<Message>]
    NewMessage = 0,

    Unknown = u8::MAX,
}

#[repr(u8)]
#[derive(Eq, PartialEq, ToPrimitive, FromPrimitive)]
pub enum ServerCommand {
    /// Input = [LoginInput]
    /// Output = [LoginOutput]
    Login = 0,
    /// Input = [SendMessageInput]
    /// Output = [None]
    SendMessage = 1,

    Unknown = u8::MAX,
}

#[repr(u8)]
#[derive(Eq, PartialEq, ToPrimitive, FromPrimitive)]
pub enum ServerResponse {
    Success = 0,
    Error = 1,
}
