//! This example intends to use the smallest amount of code to make a simple QUIC connection.
//!
//! Checkout the `README.md` for guidance.

mod common;


use std::collections::HashMap;

use std::sync::{OnceLock};
use anyhow::anyhow;
use async_trait::async_trait;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use quinn::{RecvStream, SendStream};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex};
use uuid::{Uuid};
use common::{make_client_endpoint, make_server_endpoint};

#[repr(u8)]
#[derive(Eq, PartialEq, ToPrimitive, FromPrimitive)]
enum Command {
    Login = 0x01,
    Ping = 0x02,

    Unknown = u8::MAX
}

#[async_trait]
trait Payload<T> {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<T>;

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()>;
}

pub struct LoginPayload {
    username: String,
    password: String
}

impl LoginPayload {
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

#[async_trait]
impl Payload<LoginPayload> for LoginPayload {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<Self> {
        let username_size: u16 = recv.read_u16().await?;
        let username: String = String::from_utf8(recv.read_chunk(username_size.into(), true).await?.unwrap().bytes.to_vec())?;

        let password_size: u16 = recv.read_u16().await?;
        let password: String = String::from_utf8(recv.read_chunk(password_size.into(), true).await?.unwrap().bytes.to_vec())?;

        return Ok(LoginPayload::new(username, password));
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        let mut buf: Vec<u8> = vec![];

        let username: &[u8] = self.username.as_bytes();
        buf.extend_from_slice(&username.len().to_u16().unwrap().to_be_bytes());
        buf.extend_from_slice(username);

        let password: &[u8] = self.password.as_bytes();
        buf.extend_from_slice(&password.len().to_u16().unwrap().to_be_bytes());
        buf.extend_from_slice(password);

        send.write_all(buf.as_slice())
            .await
            .map_err(|e| anyhow!("Failed to send request: {}" , e))?;

        Ok(())
    }
}

pub struct PingPayload {
    client_id: Uuid,
    iteration: u32
}

#[async_trait]
impl Payload<PingPayload> for PingPayload {
    async fn read_from_recv_stream(recv: &mut RecvStream) -> anyhow::Result<Self> {
        let uuid = Uuid::from_u128(recv.read_u128().await?);
        let iteration = recv.read_u32().await?;

        Ok( PingPayload::new(uuid, iteration) )
    }

    async fn write_to_send_stream(&self, send: &mut SendStream) -> anyhow::Result<()> {
        let mut buf: Vec<u8> = vec![];

        // add client_id
        buf.extend_from_slice(self.client_id.to_bytes_le().as_slice());
        // add iteration
        buf.append(&mut self.iteration.to_be_bytes().to_vec());

        send.write_all(buf.as_slice())
            .await
            .map_err(|e| anyhow!("Failed to send request: {}" , e))?;

        Ok(())
    }
}

impl PingPayload {
    pub fn new(client_id: Uuid, iteration: u32) -> Self {
        Self { client_id, iteration }
    }
    pub fn iteration(&self) -> u32 {
        self.iteration
    }
}

enum ServerResponse {
    Success = 0x00,
    Error = 0x01
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
        let (mut send, mut recv) = connection.open_bi().await.map_err(|e| anyhow!("failed to open stream: {}", e))?;
        if i == 0 {
            send.write_u8(Command::Login as u8).await?;

            let username: String = "test".to_string();
            let password: String = "test".to_string();

            let login_payload = LoginPayload::new(username, password);
            login_payload.write_to_send_stream(&mut send).await?;
            send.finish()
                .await
                .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;

            let resp: u8 = recv.read_u8().await?;
            println!("resp = {resp}");
            if resp == ServerResponse::Success as u8 {
                let resp: u128 = recv.read_u128().await?;
                uuid = Some(Uuid::from_u128(resp));
                println!("uuid = {}", uuid.unwrap());
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

            let payload = PingPayload::new(uuid.unwrap(), i);

            payload.write_to_send_stream(&mut send).await?;
            send.finish()
                .await
                .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;

            println!("request: {:?}", String::from_utf8(recv.read_to_end(50).await?)?);
            println!("pong received");
        }

        i += 1;
    }

    // Make sure the server has a chance to clean up
    // endpoint.wait_idle().await;

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

                let payload = LoginPayload::read_from_recv_stream(&mut recv).await?;

                let mut response: Vec<u8> = vec![];
                if payload.username() == "test" && payload.password() == "test" {
                    let uuid = Uuid::new_v4();

                    MAP.get_or_init(|| Mutex::new(HashMap::new())).lock().await.insert(uuid, User::new(uuid));

                    response.push(ServerResponse::Success as u8);
                    response.append(&mut uuid.to_bytes_le().to_vec());
                } else {
                    response.push(ServerResponse::Error as u8);
                    response.append(&mut "Invalid username or password.".as_bytes().to_vec());
                }

                send.write_all(response.as_slice()).await?;
                send.finish().await?;
            },
            Command::Ping => {
                println!("Ping");

                let payload = PingPayload::read_from_recv_stream(&mut recv).await?;

                send.write_all(format!("PONG {}", payload.iteration).as_bytes()).await?;
                send.finish().await?;
            },
            Command::Unknown => {
                println!("Nothing");
            }
        }
    }

    Ok(())
}