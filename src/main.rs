mod common;
mod protocol;

use std::collections::HashMap;

use std::net::SocketAddr;

use anyhow::anyhow;
use num_traits::FromPrimitive;
use quinn::Connection;
use std::sync::OnceLock;

use crate::protocol::LoginInput;
use crate::protocol::{Command, LoginOutput, PingInput, PingOutput};
use common::{make_client_endpoint, make_server_endpoint};
use example_core::Payload;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::signal;
use tokio::sync::{mpsc, Mutex};

use uuid::Uuid;

enum ServerResponse {
    Success = 0x00,
    Error = 0x01,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let (endpoint, server_cert) = make_server_endpoint(server_addr)?;
    tokio::spawn({
        let endpoint = endpoint.clone();

        async move {
            loop {
                let incoming_conn = endpoint.accept().await.unwrap();
                let conn = incoming_conn.await.unwrap();
                println!(
                    "[server] connection accepted: addr={}",
                    conn.remote_address()
                );

                tokio::spawn(async move {
                    let _ = receive_command(conn).await;
                });
            }
        }
    });

    let endpoint = make_client_endpoint("0.0.0.0:0".parse().unwrap(), &[&server_cert])?;
    // connect to server
    let connection = endpoint
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    println!("[client] connected: addr={}", connection.remote_address());

    let (stop_signal_sender, mut stop_signal_recv) = mpsc::channel(1);
    tokio::spawn({
        let endpoint = endpoint.clone();
        let connection = connection.clone();
        let stop_signal_sender = stop_signal_sender.clone();

        async move {
            match signal::ctrl_c().await {
                Ok(()) => {
                    println!("CTRL_C CLICKED!!!");
                    connection.close(0u32.into(), b"closed prematurely");

                    let _ = stop_signal_sender.send(()).await;

                    endpoint.wait_idle().await;
                }
                Err(err) => {
                    eprintln!("Unable to listen for shutdown signal: {}", err);
                    // we also shut down in case of error
                }
            }
        }
    });

    let mut i: u16 = 0;
    let mut futures = vec![];
    while i < 10 {
        futures.push(tokio::spawn({
            let server_cert = server_cert.clone();
            async move {
                let _ = connect_and_ping(i, server_addr, &server_cert).await;
            }
        }));
        i += 1;
    }

    tokio::spawn(async move {
        for handle in futures {
            let _ = handle.await;
        }

        let _ = stop_signal_sender.send(()).await;
    });

    let _ = stop_signal_recv.recv().await;

    println!("Shutting down.");
    endpoint.wait_idle().await;

    Ok(())
}

async fn connect_and_ping(
    i: u16,
    server_addr: SocketAddr,
    server_cert: &Vec<u8>,
) -> anyhow::Result<()> {
    let endpoint = make_client_endpoint("0.0.0.0:0".parse().unwrap(), &[server_cert])
        .map_err(|_| anyhow!("error while creating client endpoint"))?;
    // connect to server
    let connection = endpoint
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    println!("[client] connected: addr={}", connection.remote_address());

    let mut j: u32 = 0;
    let uuid: Uuid = login(&connection).await?;

    loop {
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| anyhow!("failed to open stream: {}", e))?;

        if j == 100 {
            println!("closing");
            connection.close(0u32.into(), b"done");
            break;
        } else {
            send.write_u8(Command::Ping as u8).await?;

            let payload = PingInput::new(uuid, j);

            payload.write_to_send_stream(&mut send).await?;
            send.finish()
                .await
                .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;

            let _response_code: u8 = recv.read_u8().await?;
            let output: PingOutput = PingOutput::read_from_recv_stream(&mut recv).await?;

            println!(" -> Pong ({i},{j})");
            assert_eq!(output.iteration(), j);
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }

        j += 1;
    }

    Ok(())
}

async fn login(connection: &Connection) -> anyhow::Result<Uuid> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|e| anyhow!("failed to open stream: {}", e))?;

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
        Ok(resp.client_id())
    } else {
        let message_bytes = recv.read_to_end(usize::MAX).await?;
        let message: String = String::from_utf8(message_bytes)?;
        panic!("{}", format!("Login failed! {}", message));
    }
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

async fn receive_command(connection: Connection) -> anyhow::Result<()> {
    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
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
                    let output: LoginOutput = LoginOutput::new(uuid);
                    output.write_to_send_stream(&mut send).await?;
                } else {
                    send.write_u8(ServerResponse::Error as u8).await?;
                    send.write_all("Invalid username or password.".as_bytes())
                        .await?;
                }

                send.finish()
                    .await
                    .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;
            }
            Command::Ping => {
                print!("Ping");

                let input: PingInput = PingInput::read_from_recv_stream(&mut recv).await?;

                {
                    let guard = MAP.get().unwrap().lock().await;
                    let user: Option<&User> = guard.get(&input.client_id());
                    assert!(user.is_some());
                }

                send.write_u8(ServerResponse::Success as u8).await?;
                let output: PingOutput = PingOutput::new(input.iteration());
                output.write_to_send_stream(&mut send).await?;
                send.finish()
                    .await
                    .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;
            }
            Command::Unknown => {
                println!("Nothing");
            }
        }
    }

    Ok(())
}
