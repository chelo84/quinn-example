mod common;
mod protocol;

use std::collections::HashMap;

use anyhow::anyhow;
use num_traits::FromPrimitive;
use std::sync::OnceLock;

use crate::protocol::LoginInput;
use crate::protocol::{Command, LoginOutput, PingInput, PingOutput};
use common::{make_client_endpoint, make_server_endpoint};
use example_core::Payload;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use uuid::Uuid;

enum ServerResponse {
    Success = 0x00,
    Error = 0x01,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let (endpoint, server_cert) = make_server_endpoint(server_addr)?;
    // accept a single connection
    let endpoint2 = endpoint.clone();
    tokio::spawn(async move {
        let incoming_conn = endpoint2.accept().await.unwrap();
        let conn = incoming_conn.await.unwrap();
        println!(
            "[server] connection accepted: addr={}",
            conn.remote_address()
        );

        let _ = receive_ping(conn).await;
        // Dropping all handles associated with a connection implicitly closes it
    });

    let endpoint = make_client_endpoint("0.0.0.0:0".parse().unwrap(), &[&server_cert])?;
    // connect to server
    let connection = endpoint
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    println!("[client] connected: addr={}", connection.remote_address());

    let mut i: u32 = 0;
    let mut uuid: Option<Uuid> = None;
    // Waiting for a stream will complete with an error when the server closes the connection
    loop {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| anyhow!("failed to open stream: {}", e))?;
        if i == 0 {
            send.write_u8(Command::Login as u8).await?;

            let username: String = "test".to_string();
            let password: String = "test".to_string();

            let login_payload = LoginInput::new(username, password);
            login_payload.write_to_send_stream(&mut send).await?;
            send.finish()
                .await
                .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;

            let resp: u8 = recv.read_u8().await?;
            if resp == ServerResponse::Success as u8 {
                let resp: LoginOutput = LoginOutput::read_from_recv_stream(&mut recv).await?;

                println!("uuid = {}", resp.client_id());
                uuid = Some(resp.client_id());
            } else {
                let message_bytes = recv.read_to_end(usize::MAX).await?;
                let message: String = String::from_utf8(message_bytes)?;
                panic!("{}", format!("Login failed! {}", message));
            }
        } else if i == 100 {
            println!("closing");
            connection.close(0u32.into(), b"done");
            break;
        } else {
            send.write_u8(Command::Ping as u8).await?;

            let payload = PingInput::new(uuid.unwrap(), i);

            payload.write_to_send_stream(&mut send).await?;
            send.finish()
                .await
                .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;

            let output: PingOutput = PingOutput::read_from_recv_stream(&mut recv).await?;
            println!(" -> Pong");
            assert_eq!(output.iteration(), i);
        }

        i += 1;
    }

    // Make sure the server has a chance to clean up
    endpoint.wait_idle().await;

    Ok(())
}

#[allow(dead_code)]
struct User {
    pub client_id: Uuid,
}

impl User {
    pub fn new(client_id: Uuid) -> Self {
        Self { client_id }
    }
}

static MAP: OnceLock<Mutex<HashMap<Uuid, User>>> = OnceLock::new();

async fn receive_ping(connection: quinn::Connection) -> anyhow::Result<()> {
    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
        // Because it is a bidirectional stream, we can both send and receive.
        // println!("request: {:?}", String::from_utf8(recv.read_to_end(50).await?)?);
        let command = recv.read_u8().await?;
        match Command::from_u8(command).unwrap_or(Command::Unknown) {
            Command::Login => {
                println!("Login");

                let payload: LoginInput = LoginInput::read_from_recv_stream(&mut recv).await?;

                if payload.username() == "test" && payload.password() == "test" {
                    let uuid = Uuid::new_v4();

                    MAP.get_or_init(|| Mutex::new(HashMap::new()))
                        .lock()
                        .await
                        .insert(uuid, User::new(uuid));

                    send.write_u8(ServerResponse::Success as u8).await?;
                    let output = LoginOutput::new(uuid);
                    output.write_to_send_stream(&mut send).await?;
                } else {
                    send.write_u8(ServerResponse::Error as u8).await?;
                    send.write_all("Invalid username or password.".as_bytes())
                        .await?;
                }
                send.finish().await?;
            }
            Command::Ping => {
                print!("Ping");

                let input: PingInput = PingInput::read_from_recv_stream(&mut recv).await?;

                {
                    let guard = MAP.get().unwrap().lock().await;
                    let user: Option<&User> = guard.get(&input.client_id());
                    assert!(user.is_some());
                }

                let output: PingOutput = PingOutput::new(input.iteration());

                output.write_to_send_stream(&mut send).await?;
                send.finish().await?;
            }
            Command::Unknown => {
                println!("Nothing");
            }
        }
    }

    Ok(())
}
