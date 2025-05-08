use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
    Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::protocol::Message};

use crate::tunnel::client::TunnelMessage;

type Clients = Arc<Mutex<HashMap<String, ClientInfo>>>;

struct ClientInfo {
    domain: Option<String>,
    sender: mpsc::Sender<Message>,
}

pub async fn run(port: u16) -> Result<()> {
    // Create shared state
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    // Start WebSocket server for tunneling
    let ws_addr = format!("0.0.0.0:{}", port);
    let ws_listener = TcpListener::bind(&ws_addr).await?;
    println!("WebSocket server listening on {}", ws_addr);

    // Start HTTP server for handling public requests
    let http_addr = format!("0.0.0.0:{}", port + 1);
    let http_listener = TcpListener::bind(&http_addr).await?;
    println!("HTTP server listening on {}", http_addr);

    // Clone the clients reference for the HTTP server
    let http_clients = clients.clone();

    // Spawn HTTP server task
    tokio::spawn(async move {
        loop {
            match http_listener.accept().await {
                Ok((stream, _)) => {
                    let clients = http_clients.clone();
                    tokio::spawn(async move {
                        if let Err(err) = handle_http_connection(stream, clients).await {
                            eprintln!("Error in HTTP connection: {}", err);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Failed to accept HTTP connection: {}", e);
                }
            }
        }
    });

    // Handle WebSocket connections
    loop {
        if let Ok((stream, addr)) = ws_listener.accept().await {
            let clients = clients.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_ws_connection(stream, addr, clients).await {
                    eprintln!("Error in WebSocket connection: {}", err);
                }
            });
        }
    }
}

async fn handle_ws_connection(stream: TcpStream, addr: SocketAddr, clients: Clients) -> Result<()> {
    println!("New WebSocket connection: {}", addr);

    let ws_stream = accept_async(stream)
        .await
        .context("Failed to accept WebSocket connection")?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Handle the first message to determine the type of connection
    if let Some(Ok(Message::Binary(data))) = ws_receiver.next().await {
        match serde_json::from_slice::<TunnelMessage>(&data) {
            Ok(TunnelMessage::Register { client_id, domain }) => {
                println!(
                    "Client registered: {} with domain: {:?}",
                    client_id, &domain
                );

                // Create a channel for this client
                let (sender, mut receiver) = mpsc::channel::<Message>(100);

                // Store client info with cloned domain
                {
                    let mut clients_lock = clients.lock().await;
                    clients_lock.insert(
                        client_id.clone(),
                        ClientInfo {
                            domain: domain.clone(),
                            sender: sender.clone(),
                        },
                    );
                }

                // Generate and send the tunnel URL
                let domain_part = if let Some(domain_val) = &domain {
                    domain_val.clone()
                } else {
                    format!("{}.public.dev.peril.lol", client_id)
                };

                let tunnel_url = format!("https://{}", domain_part);
                let response = TunnelMessage::Registered {
                    url: tunnel_url.clone(),
                };

                ws_sender
                    .send(Message::Binary(serde_json::to_vec(&response)?))
                    .await?;
                println!("Sent registration confirmation: {}", tunnel_url);

                // Create a separate task for handling messages to the client
                let sender_for_ws = sender.clone();

                // Now handle the data forwarding for this client
                tokio::spawn(async move {
                    // Forward messages from receiver to WebSocket
                    while let Some(msg) = receiver.recv().await {
                        if sender_for_ws.send(msg).await.is_err() {
                            break;
                        }
                    }
                });

                // Handle incoming WebSocket messages
                while let Some(Ok(msg)) = ws_receiver.next().await {
                    match msg {
                        Message::Binary(data) => {
                            if let Ok(tunnel_msg) = serde_json::from_slice::<TunnelMessage>(&data) {
                                match tunnel_msg {
                                    TunnelMessage::Data { data: response_data } => {
                                        // This would be handled by the HTTP connection handler
                                        println!(
                                            "Received data response from client: {} bytes",
                                            response_data.len()
                                        );
                                    }
                                    TunnelMessage::KeepAlive => {
                                        // Send keep-alive response
                                        if let Err(e) = ws_sender
                                            .send(Message::Binary(serde_json::to_vec(
                                                &TunnelMessage::KeepAlive,
                                            )?))
                                            .await
                                        {
                                            eprintln!("Error sending keep-alive: {}", e);
                                            break;
                                        }
                                    }
                                    _ => {
                                        println!("Ignoring unexpected message type");
                                    }
                                }
                            }
                        }
                        Message::Close(_) => {
                            break;
                        }
                        _ => {}
                    }
                }

                // Client disconnected, remove from active clients
                let mut clients_lock = clients.lock().await;
                clients_lock.remove(&client_id);
                println!("Client disconnected: {}", client_id);
            }
            _ => {
                println!("Received unexpected message type on initial connection");
            }
        }
    }

    Ok(())
}

async fn handle_http_connection(tcp_stream: TcpStream, clients: Clients) -> Result<()> {
    let io = TokioIo::new(tcp_stream);

    // Process the HTTP request
    if let Err(err) = http1::Builder::new()
        .serve_connection(
            io,
            service_fn(move |req| {
                let clients = clients.clone();
                async move {
                    let result = handle_request(req, clients).await;
                    match result {
                        Ok(response) => Ok::<_, anyhow::Error>(response),
                        Err(e) => {
                            eprintln!("Error handling request: {}", e);

                            // Return 500 Internal Server Error
                            let body = json!({
                                "error": "Internal server error",
                            })
                            .to_string();

                            Ok(Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .header("Content-Type", "application/json")
                                .body(full_body(body))
                                .unwrap())
                        }
                    }
                }
            }),
        )
        .await
    {
        eprintln!("Error serving HTTP connection: {}", err);
    }

    Ok(())
}

async fn handle_request(
    req: Request<Incoming>,
    clients: Clients,
) -> Result<Response<BoxBody<Bytes, anyhow::Error>>> {
    // Extract the host from the request
    let host = match req.headers().get("host") {
        Some(h) => h.to_str().unwrap_or("").to_string(),
        None => return Ok(not_found_response()),
    };
    
    // Get the original URI and convert to string
    let uri = req.uri().to_string();
    
    // Extract the base domain without path components for routing
    let base_domain = host.split('/').next().unwrap_or(&host).to_string();

    // Find the client based on the host
    let (client_id, sender) = {
        let clients_lock = clients.lock().await;

        // First try to match by domain
        let mut matched_client = None;

        for (id, info) in clients_lock.iter() {
            if let Some(domain) = &info.domain {
                if base_domain.starts_with(domain) {
                    matched_client = Some((id.clone(), info.sender.clone()));
                    break;
                }
            }
        }

        // If no domain match, try to match by client ID subdomain
        if matched_client.is_none() {
            for (id, info) in clients_lock.iter() {
                if base_domain.starts_with(&format!("{}.public.dev.peril.lol", id)) {
                    matched_client = Some((id.clone(), info.sender.clone()));
                    break;
                }
            }
        }

        match matched_client {
            Some(client) => client,
            None => return Ok(not_found_response()),
        }
    };

    println!("Forwarding request to client: {} with URI: {}", client_id, uri);

    // Create a channel for the response
    let (_tx, mut rx) = mpsc::channel::<Vec<u8>>(1);

    // Create a request structure that includes the full URI and method
    let request_data = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n\r\n",
        req.method(),
        req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("/"),
        host
    );

    // Create a message to send to the client with the full request data
    let tunnel_msg = TunnelMessage::Data { data: request_data.into_bytes() };

    // Send the request to the client
    sender
        .send(Message::Binary(serde_json::to_vec(&tunnel_msg)?))
        .await?;

    // Wait for the response with a timeout
    let response_data =
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(full_body("Client disconnected".to_string()))
                    .unwrap());
            }
            Err(_) => {
                return Ok(Response::builder()
                    .status(StatusCode::GATEWAY_TIMEOUT)
                    .body(full_body("Request timed out".to_string()))
                    .unwrap());
            }
        };

    // Parse and return the response
    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(full_body(response_data))
        .unwrap())
}

fn not_found_response() -> Response<BoxBody<Bytes, anyhow::Error>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(full_body("Not found".to_string()))
        .unwrap()
}

fn full_body<T: Into<Bytes>>(body: T) -> BoxBody<Bytes, anyhow::Error> {
    Full::new(body.into())
        .map_err(|never| match never {})
        .boxed()
}