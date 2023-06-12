mod login;
mod ping;

use num_derive::{FromPrimitive, ToPrimitive};

pub use login::{LoginInput, LoginOutput};
pub use ping::{PingInput, PingOutput};

#[repr(u8)]
#[derive(Eq, PartialEq, ToPrimitive, FromPrimitive)]
pub enum Command {
    Login = 0x01,
    Ping = 0x02,

    Unknown = u8::MAX,
}
