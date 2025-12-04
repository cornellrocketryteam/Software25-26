mod command;
// mod hardware;
// mod components;

use anyhow::Result;
use async_tungstenite::{WebSocketStream, tungstenite};
use smol::Async;
use smol::stream::StreamExt;
use std::net::{TcpListener, TcpStream};
use tracing::{Instrument, Level, debug, error, info, span, warn};
use tracing_subscriber::fmt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tungstenite::Message;

use crate::command::{Command, CommandResponse};
// use crate::hardware::Hardware;

fn main() -> Result<()> {
    // Create a log layer for file output
    let file_appender = tracing_appender::rolling::hourly("logs", "tracing.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false); // Disable colors in file

    // Create a log layer for stdout
    let stdout_layer = fmt::layer().with_writer(std::io::stdout);

    // Combine both layers and enable logging
    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    smol::block_on(async {
        info!("Initializing fill station...");
        // let hardware = Hardware::new().await?;

        info!("Initializing web socket server...");
        let listener = Async::<TcpListener>::bind(([0, 0, 0, 0], 9000))?;
        let host = listener.get_ref().local_addr()?;

        info!("Server listening on ws://{}", host);

        loop {
            // Accept incoming connection - don't crash server on error
            let Ok((stream, _)) = listener.accept().await else {
                error!("Failed to accept connection");
                continue;
            };

            let client_ip = stream
                .get_ref()
                .peer_addr()
                .map(|s| s.ip().to_string())
                .unwrap_or("unknown".to_string());

            // Perform WebSocket handshake - don't crash server on error
            let Ok(stream) = async_tungstenite::accept_async(stream).await else {
                error!("WebSocket handshake failed");
                continue;
            };

            // Spawn handler for this connection
            let span = span!(Level::INFO, "websocket", client_ip);
            smol::spawn(handle_socket(stream).instrument(span)).detach();
        }
    })
}

/// Handle WebSocket connection lifecycle
async fn handle_socket(mut stream: WebSocketStream<Async<TcpStream>>) {
    info!("Client connected");
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(message)) => {
                let response = process_message(&message).await;
                if let Err(e) = send_response(&mut stream, response).await {
                    error!("Error sending message: {}", e);
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(e) => {
                error!("Error receiving message: {}", e);
                break;
            }
        }
    }
    info!("Client disconnected")
}

async fn process_message(message: &str) -> CommandResponse {
    debug!("Received message: {}", message);

    match serde_json::from_str(message) {
        Ok(command) => {
            info!("Received command: {:?}", command);
            execute_command(command).await
        }
        Err(e) => {
            warn!("Failed to parse command: {}", e);
            CommandResponse::Error
        }
    }
}

async fn execute_command(command: Command) -> CommandResponse {
    match command {
        Command::Ignite => CommandResponse::Success,
    }
}

/// Send JSON response back through WebSocket
async fn send_response(
    socket: &mut WebSocketStream<Async<TcpStream>>,
    response: CommandResponse,
) -> Result<()> {
    let json = serde_json::to_string(&response)?;
    socket.send(Message::Text(json.into())).await?;
    Ok(())
}
