mod chat;
mod common;

use std::error::Error;
use std::sync::OnceLock;

use anyhow::anyhow;
use num_traits::FromPrimitive;
use quinn::Connection;

use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::chat::protocol::{
    ClientCommand, LoginInput, Message, SendMessageInput, ServerCommand, ServerResponse, User,
};
use crate::common::{create_stop_signal, make_client_endpoint};
use example_core::Payload;

#[tokio::main]
async fn main() -> anyhow::Result<(), Box<dyn Error>> {
    let server_addr = "127.0.0.1:5000".parse().unwrap();
    let cert_der = fs::read("certs/cert.der")
        .await
        .map_err(|e| anyhow!("Unable to read cert.der file: {}", e))?;
    let endpoint = make_client_endpoint("0.0.0.0:0".parse().unwrap(), &[&cert_der])?;
    // connect to server
    let connection = endpoint
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();

    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);

    println!("Type your username and press enter.");
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut username: String = String::new();
    reader.read_line(&mut username).await?;

    let _user: User;
    {
        let (mut send, mut recv) = connection.open_bi().await?;
        let login_input: LoginInput = LoginInput::new(username.trim());

        send.write_u8(ServerCommand::Login as u8).await?;
        login_input.write_to_send_stream(&mut send).await?;
        if ServerResponse::Error
            == ServerResponse::from_u8(recv.read_u8().await?).unwrap_or(ServerResponse::Error)
        {
            let error_message = String::read_from_recv_stream(&mut recv).await?;

            Err(anyhow!("Failed to login! {error_message}"))?;
        };

        _user = User::read_from_recv_stream(&mut recv).await?;
    }
    reload_screen().await;

    // receive commands loop
    tokio::spawn({
        let connection = connection.clone();

        async move {
            let _ = receive_commands(connection).await;
        }
    });

    // send message loop
    tokio::spawn({
        let connection = connection.clone();

        async move {
            let _ = send_messages(connection).await;
        }
    });

    let (stop_signal_sender, mut stop_signal_recv) = create_stop_signal().await;

    tokio::spawn({
        let endpoint = endpoint.clone();
        let connection = connection.clone();

        async move {
            endpoint.wait_idle().await;
            println!(
                "Lost connection! Reason: {}",
                connection.close_reason().unwrap()
            );
            let _ = stop_signal_sender.send(()).await;
        }
    });

    let _ = stop_signal_recv.recv().await;

    connection.close(0_u32.into(), b"done");

    Ok(())
}

async fn send_messages(connection: Connection) -> anyhow::Result<()> {
    loop {
        let (mut send, mut recv) = connection.open_bi().await?;

        let mut reader = BufReader::new(tokio::io::stdin());
        let mut message_text: String = String::new();
        reader.read_line(&mut message_text).await?;

        let message: SendMessageInput = SendMessageInput::new(message_text.trim());
        send.write_u8(ServerCommand::SendMessage as u8).await?;
        message.write_to_send_stream(&mut send).await?;

        assert_eq!(recv.read_u8().await?, ServerResponse::Success as u8);
    }
}

static MESSAGES: OnceLock<Mutex<Vec<Message>>> = OnceLock::new();

async fn receive_commands(connection: Connection) -> anyhow::Result<()> {
    loop {
        let mut recv = connection.accept_uni().await?;

        let command: u8 = recv.read_u8().await?;

        match ClientCommand::from_u8(command).unwrap_or(ClientCommand::Unknown) {
            ClientCommand::NewMessage => {
                let mut messages = Vec::<Message>::read_from_recv_stream(&mut recv).await?;

                MESSAGES
                    .get_or_init(|| Mutex::new(vec![]))
                    .lock()
                    .await
                    .append(&mut messages);

                reload_screen().await;
            }
            ClientCommand::Unknown => {
                println!("Unknown client command: {}", command);
            }
        }
    }
}

async fn reload_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);

    let messages = MESSAGES.get_or_init(|| Mutex::new(vec![])).lock().await;
    for message in messages.iter() {
        if let Some(user) = message.sent_by() {
            println!(
                "{sent_by}: {message}",
                sent_by = user.username(),
                message = message.message()
            );
        } else {
            println!("{message}", message = message.message());
        }
    }
    println!("Send a new message by pressing enter.");
}
