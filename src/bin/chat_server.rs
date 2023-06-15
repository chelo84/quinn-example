mod chat;
mod common;

use anyhow::anyhow;
use std::collections::HashMap;
use std::sync::OnceLock;

use example_core::Payload;
use num_traits::FromPrimitive;
use quinn::Connection;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, MutexGuard};
use uuid::Uuid;

use crate::chat::protocol::{
    ClientCommand, LoginInput, Message, SendMessageInput, ServerCommand, ServerResponse, User,
};
use crate::common::{create_stop_signal, make_server_endpoint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let (endpoint, server_cert) = make_server_endpoint(server_addr).await?;

    fs::create_dir_all("certs/").await?;
    fs::write("certs/cert.der", &server_cert).await?;

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
                    let _ = await_commands(conn).await;
                });
            }
        }
    });

    let mut stop_signal_recv = create_stop_signal().await;
    let _ = stop_signal_recv.recv().await;

    println!("Shutting down.");
    endpoint.wait_idle().await;

    Ok(())
}

static USERS_MAP: OnceLock<Mutex<HashMap<Uuid, User>>> = OnceLock::new();
static CONNECTIONS_MAP: OnceLock<Mutex<HashMap<Uuid, Connection>>> = OnceLock::new();
static MESSAGES_BUFFER: OnceLock<Mutex<Vec<Message>>> = OnceLock::new();

async fn await_commands(connection: Connection) -> anyhow::Result<()> {
    let uuid: Uuid = Uuid::new_v4();

    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
        let command = recv.read_u8().await?;

        match ServerCommand::from_u8(command).unwrap_or(ServerCommand::Unknown) {
            ServerCommand::Login => {
                println!("> Login");

                let payload: LoginInput = LoginInput::read_from_recv_stream(&mut recv).await?;
                match login(&connection, uuid, payload).await {
                    Ok(user) => {
                        send.write_u8(ServerResponse::Success as u8).await?;

                        user.write_to_send_stream(&mut send).await?;

                        let mut previous_messages_send = connection.open_uni().await?;
                        let messages_guard = MESSAGES_BUFFER
                            .get_or_init(|| Mutex::new(vec![]))
                            .lock()
                            .await;
                        previous_messages_send
                            .write_u8(ClientCommand::NewMessage as u8)
                            .await?;
                        messages_guard
                            .write_to_send_stream(&mut previous_messages_send)
                            .await?;
                    }
                    Err(e) => {
                        send.write_u8(ServerResponse::Error as u8).await?;

                        e.to_string().write_to_send_stream(&mut send).await?;
                    }
                };

                send.finish()
                    .await
                    .map_err(|e| anyhow!("failed to shutdown stream: {}", e))?;
            }
            ServerCommand::SendMessage => {
                println!("> SendMessage");

                let input = SendMessageInput::read_from_recv_stream(&mut recv).await?;
                let guard = USERS_MAP.get().unwrap().lock().await;
                if let Some(user) = guard.get(input.client_id()) {
                    let user = user.clone();
                    drop(guard);

                    let message: Message = Message::new(input.message(), user.clone());
                    MESSAGES_BUFFER
                        .get_or_init(|| Mutex::new(vec![]))
                        .lock()
                        .await
                        .push(message.clone());

                    send.write_u8(ServerResponse::Success as u8).await?;

                    propagate_message(message).await?;
                } else {
                    // user not found
                    send.write_u8(ServerResponse::Error as u8).await?;
                }
            }
            ServerCommand::Unknown => {
                println!("> Unknown");
            }
        }
    }

    if let Some(map) = CONNECTIONS_MAP.get() {
        println!("Remove connection {}", &uuid);
        map.lock().await.remove(&uuid);
    }
    if let Some(map) = USERS_MAP.get() {
        println!("Remove user {}", &uuid);
        map.lock().await.remove(&uuid);
    }

    Ok(())
}

// SEND Message COMMAND TO ALL CONNECTIONS
async fn propagate_message(message: Message) -> anyhow::Result<()> {
    let guard = CONNECTIONS_MAP
        .get()
        .ok_or(anyhow!("CONNECTION_MAP not initialized"))?
        .lock()
        .await;
    for connection in guard.values() {
        let mut send = connection.open_uni().await?;
        send.write_u8(ClientCommand::NewMessage as u8).await?;
        vec![message.clone()]
            .write_to_send_stream(&mut send)
            .await?;
    }

    Ok(())
}

async fn login(connection: &Connection, uuid: Uuid, payload: LoginInput) -> anyhow::Result<User> {
    let username: &str = payload.username().trim();

    let mut users_guard = USERS_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .await;

    validate_username(username, &users_guard)?;

    let user: User = User::new(uuid, username);

    users_guard.insert(uuid, user.clone());

    CONNECTIONS_MAP
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .await
        .insert(uuid, connection.clone());

    Ok(user)
}

fn validate_username(
    username: &str,
    users_guard: &MutexGuard<HashMap<Uuid, User>>,
) -> anyhow::Result<()> {
    for user in users_guard.values() {
        if user.username() == username {
            return Err(anyhow!("Username already used!"));
        }
    }

    Ok(())
}
