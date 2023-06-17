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

    let (_, mut stop_signal_recv) = create_stop_signal().await;
    let _ = stop_signal_recv.recv().await;

    println!("Shutting down.");
    endpoint.close(0_u32.into(), b"Shut down");

    Ok(())
}

pub type ConnectionStableId = usize;
static USERS: OnceLock<Mutex<HashMap<ConnectionStableId, User>>> = OnceLock::new();
static CONNECTIONS: OnceLock<Mutex<HashMap<ConnectionStableId, Connection>>> = OnceLock::new();
static MESSAGES: OnceLock<Mutex<Vec<Message>>> = OnceLock::new();

async fn await_commands(connection: Connection) -> anyhow::Result<()> {
    while let Ok((mut send, mut recv)) = connection.accept_bi().await {
        let command = recv.read_u8().await?;

        match ServerCommand::from_u8(command).unwrap_or(ServerCommand::Unknown) {
            ServerCommand::Login => {
                println!("> Login");

                let payload: LoginInput = LoginInput::read_from_recv_stream(&mut recv).await?;
                match login(&connection, Uuid::new_v4(), payload).await {
                    Ok(user) => {
                        send.write_u8(ServerResponse::Success as u8).await?;

                        user.write_to_send_stream(&mut send).await?;

                        let mut previous_messages_send = connection.open_uni().await?;
                        let messages_guard =
                            MESSAGES.get_or_init(|| Mutex::new(vec![])).lock().await;
                        previous_messages_send
                            .write_u8(ClientCommand::NewMessage as u8)
                            .await?;
                        messages_guard
                            .write_to_send_stream(&mut previous_messages_send)
                            .await?;

                        let message: Message = Message::new(
                            format!(
                                "{username} has entered the chat!",
                                username = user.username()
                            )
                            .as_str(),
                            None,
                        );
                        let _ = propagate_message(message, Some(&connection)).await;
                    }
                    Err(e) => {
                        send.write_u8(ServerResponse::Error as u8).await?;

                        e.to_string().write_to_send_stream(&mut send).await?;
                    }
                };
            }
            ServerCommand::SendMessage => {
                println!("> SendMessage");

                let input = SendMessageInput::read_from_recv_stream(&mut recv).await?;
                let guard = USERS.get().unwrap().lock().await;
                if let Some(user) = guard.get(&connection.stable_id()) {
                    let user = user.clone();
                    drop(guard);

                    let message: Message = Message::new(input.message(), Some(user.clone()));
                    MESSAGES
                        .get_or_init(|| Mutex::new(vec![]))
                        .lock()
                        .await
                        .push(message.clone());

                    send.write_u8(ServerResponse::Success as u8).await?;

                    propagate_message(message, None).await?;
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

    if let Some(map) = CONNECTIONS.get() {
        println!("Remove connection {}", connection.stable_id());
        map.lock().await.remove(&connection.stable_id());
    }
    if let Some(map) = USERS.get() {
        println!("Remove user {}", connection.stable_id());
        if let Some(user) = map.lock().await.remove(&connection.stable_id()) {
            let message: Message = Message::new(
                format!("{username} has left the chat!", username = user.username()).as_str(),
                None,
            );
            let _ = propagate_message(message, Some(&connection)).await;
        }
    }

    Ok(())
}

// SEND Message COMMAND TO ALL CONNECTIONS
async fn propagate_message(
    message: Message,
    ignored_connection: Option<&Connection>,
) -> anyhow::Result<()> {
    let guard = CONNECTIONS
        .get()
        .ok_or(anyhow!("CONNECTION_MAP not initialized"))?
        .lock()
        .await;
    for (client_id, connection) in guard.iter() {
        if ignored_connection.is_none() || *client_id != ignored_connection.unwrap().stable_id() {
            let mut send = connection.open_uni().await?;
            send.write_u8(ClientCommand::NewMessage as u8).await?;
            vec![message.clone()]
                .write_to_send_stream(&mut send)
                .await?;
        }
    }

    Ok(())
}

async fn login(connection: &Connection, uuid: Uuid, payload: LoginInput) -> anyhow::Result<User> {
    let username: &str = payload.username().trim();

    let mut users_guard = USERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .await;

    validate_username(username, &users_guard)?;

    let user: User = User::new(uuid, username);

    users_guard.insert(connection.stable_id(), user.clone());

    CONNECTIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .await
        .insert(connection.stable_id(), connection.clone());

    Ok(user)
}

fn validate_username(
    username: &str,
    users_guard: &MutexGuard<HashMap<ConnectionStableId, User>>,
) -> anyhow::Result<()> {
    for user in users_guard.values() {
        if user.username() == username {
            return Err(anyhow!("Username already used!"));
        }
    }

    Ok(())
}
