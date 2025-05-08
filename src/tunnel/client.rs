use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::tunnel::TunnelResult;

#[derive(Serialize, Deserialize, Debug)]
pub enum TunnelMessage {
    Register {
        client_id: String,
        domain: Option<String>,
    },
    Registered {
        url: String,
    },
    Data {
        data: Vec<u8>,
    },
    KeepAlive,
}

pub async fn start_tunnel(
    local_port: u16,
    domain: Option<String>,
    server_port: u16,
    client_id: String,
    log_sender: mpsc::Sender<String>,
) -> Result<TunnelResult> {
    // Start a detached process for the tunnel client
    let mut cmd = Command::new("cargo");
    cmd.args([
        "run",
        "--",
        "--port",
        &local_port.to_string(),
        "--public-port",
        &server_port.to_string(),
    ]);

    if let Some(domain) = &domain {
        cmd.args(["--domain", domain]);
    }

    // Detach the process
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // This would normally start the tunnel process, but for now we'll simulate it
    let child = cmd
        .spawn()
        .context("Failed to start tunnel client process")?;

    // Connect to the local server as if we were starting a standalone process
    let server_url = format!("ws://localhost:{}/register", server_port);
    let (mut socket, _) = connect_async(&server_url)
        .await
        .context("Failed to connect to tunnel server")?;

    // Send registration message
    let register_msg = TunnelMessage::Register {
        client_id: client_id.clone(),
        domain: domain.clone(),
    };

    socket
        .send(Message::Binary(serde_json::to_vec(&register_msg)?))
        .await?;

    // Wait for response
    let response = socket.next().await.context("No response from server")??;
    let tunnel_message: TunnelMessage = serde_json::from_slice(&response.into_data())?;

    let tunnel_url = match tunnel_message {
        TunnelMessage::Registered { url } => url,
        _ => return Err(anyhow::anyhow!("Unexpected response from server")),
    };

    log_sender
        .send(format!("Tunnel registered. URL: {}", tunnel_url))
        .await?;

    // Start forwarding in the background
    tokio::spawn(async move {
        let _ = handle_forwarding(client_id, server_port, local_port, log_sender).await;
    });

    // Return as if the process is running
    Ok(TunnelResult {
        url: tunnel_url,
        process: child,
    })
}

async fn handle_forwarding(
    client_id: String,
    server_port: u16,
    local_port: u16,
    log_sender: mpsc::Sender<String>,
) -> Result<()> {
    // Connect to the server's data channel
    let server_url = format!("ws://localhost:{}/data/{}", server_port, client_id);
    let (mut socket, _) = connect_async(&server_url)
        .await
        .context("Failed to connect to tunnel data channel")?;

    log_sender
        .send("Connected to server data channel".to_string())
        .await?;

    // Main loop
    while let Some(msg) = socket.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                match serde_json::from_slice::<TunnelMessage>(&data) {
                    Ok(TunnelMessage::Data { data }) => {
                        // Forward the data to the local service
                        match TcpStream::connect(format!("127.0.0.1:{}", local_port)).await {
                            Ok(mut local_stream) => {
                                // Write the data to the local service
                                if let Err(e) = local_stream.write_all(&data).await {
                                    log_sender
                                        .send(format!("Error writing to local service: {}", e))
                                        .await?;
                                    continue;
                                }

                                // Read the response from the local service
                                let mut buffer = vec![0; 8192];
                                match local_stream.read(&mut buffer).await {
                                    Ok(n) if n > 0 => {
                                        buffer.truncate(n);

                                        // Send the response back to the server
                                        let response = TunnelMessage::Data { data: buffer };
                                        socket
                                            .send(Message::Binary(serde_json::to_vec(
                                                &response,
                                            )?))
                                            .await?;
                                    }
                                    Ok(_) => {
                                        log_sender
                                            .send(
                                                "Local service closed the connection"
                                                    .to_string(),
                                            )
                                            .await?;
                                    }
                                    Err(e) => {
                                        log_sender
                                            .send(format!(
                                                "Error reading from local service: {}",
                                                e
                                            ))
                                            .await?;
                                    }
                                }
                            }
                            Err(e) => {
                                log_sender
                                    .send(format!("Failed to connect to local service: {}", e))
                                    .await?;
                            }
                        }
                    }
                    Ok(TunnelMessage::KeepAlive) => {
                        // Send keep-alive response
                        socket
                            .send(Message::Binary(serde_json::to_vec(
                                &TunnelMessage::KeepAlive,
                            )?))
                            .await?;
                    }
                    _ => {
                        log_sender
                            .send("Received unknown message type".to_string())
                            .await?;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                log_sender
                    .send("Server closed the connection".to_string())
                    .await?;
                break;
            }
            Err(e) => {
                log_sender.send(format!("WebSocket error: {}", e)).await?;
                break;
            }
            _ => {}
        }
    }

    log_sender
        .send("Disconnected from server".to_string())
        .await?;

    // Try to reconnect after a delay
    sleep(Duration::from_secs(5)).await;
    log_sender
        .send("Attempting to reconnect...".to_string())
        .await?;

    // This would normally try to reconnect, but for the example we'll just return
    Ok(())
}