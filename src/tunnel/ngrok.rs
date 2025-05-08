use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::io::{BufReader, AsyncBufReadExt};
use std::process::Stdio;
use regex::Regex;

use crate::app::App;
use super::TunnelResult;

pub async fn start_tunnel(port: u16, domain: Option<String>, app: &mut App) -> Result<TunnelResult> {
    app.add_log("Starting ngrok tunnel...");
    
    // Check if ngrok is installed
    let status = Command::new("which")
        .arg("ngrok")
        .status()
        .await
        .context("Failed to check if ngrok is installed")?;
    
    if !status.success() {
        app.add_log("Error: ngrok not found. Please install it first.");
        app.add_log("Installation instructions: https://ngrok.com/download");
        return Err(anyhow::anyhow!("ngrok not installed"));
    }
    
    // Prepare the command
    let mut cmd = Command::new("ngrok");
    cmd.arg("http");
    
    // Add custom domain if provided
    if let Some(domain) = domain {
        cmd.args(["--domain", &domain]);
    }
    
    // Add the port
    cmd.arg(port.to_string());
    
    // stdout and stderr will be captured so we can parse the URL
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    app.add_log(&format!("Running command: ngrok http {}", port));
    
    // Start the process
    let mut child = cmd.spawn().context("Failed to start ngrok process")?;
    
    // Get stdout and stderr
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    
    // Create readers
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();
    
    // Regex to extract the URL
    let url_regex = Regex::new(r"https://[^/\s]+").unwrap();
    
    // This will store the tunnel URL once we find it
    let mut tunnel_url = String::new();
    
    // Process stdout and stderr lines to find the URL
    let stdout_future = async {
        while let Some(line) = stdout_reader.next_line().await.unwrap_or(None) {
            app.add_log(&format!("ngrok: {}", line));
            
            // Try to find the tunnel URL
            if tunnel_url.is_empty() {
                if let Some(url_match) = url_regex.find(&line) {
                    tunnel_url = url_match.as_str().to_string();
                    app.add_log(&format!("Tunnel URL found: {}", tunnel_url));
                    break;
                }
            }
        }
    };
    
    let stderr_future = async {
        while let Some(line) = stderr_reader.next_line().await.unwrap_or(None) {
            app.add_log(&format!("ngrok error: {}", line));
        }
    };
    
    // Run both futures concurrently
    tokio::select! {
        _ = stdout_future => {},
        _ = stderr_future => {},
    }
    
    // If we couldn't find a URL, use a default format
    if tunnel_url.is_empty() {
        if let Some(domain) = domain {
            tunnel_url = format!("https://{}", domain);
        } else {
            tunnel_url = "https://unknown.ngrok.io".to_string();
        }
        app.add_log("Warning: Could not detect the tunnel URL. Using a default value.");
    }
    
    app.add_log(&format!("Tunnel started: localhost:{} -> {}", port, tunnel_url));
    
    Ok(TunnelResult {
        url: tunnel_url,
        process: child,
    })
}