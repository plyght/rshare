use anyhow::Result;
use tokio::process::Child;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::tunnel;

pub struct App {
    pub port: u16,
    pub domain: Option<String>,
    pub server_port: u16,
    pub tunnel_active: bool,
    pub tunnel_url: Option<String>,
    pub tunnel_process: Option<Child>,
    pub logs: Vec<String>,
    pub log_offset: usize,
    pub client_id: String,
    pub connection_error: Option<String>,
}

impl App {
    pub fn new(port: u16, domain: Option<String>, server_port: u16) -> Self {
        Self {
            port,
            domain,
            server_port,
            tunnel_active: false,
            tunnel_url: None,
            tunnel_process: None,
            logs: Vec::new(),
            log_offset: 0,
            client_id: Uuid::new_v4().to_string(),
            connection_error: None,
        }
    }

    pub async fn start_tunnel(&mut self) -> Result<()> {
        self.connection_error = None;
        self.add_log("Starting tunnel...");
        
        let (sender, _) = mpsc::channel::<String>(100);
        
        // Try to start the tunnel
        match tunnel::client::start_tunnel(
            self.port,
            self.domain.clone(),
            self.server_port,
            self.client_id.clone(),
            sender,
        )
        .await {
            Ok(result) => {
                let url = result.url.clone();  // Clone the URL before moving it
                self.tunnel_process = Some(result.process);
                self.tunnel_url = Some(result.url);
                self.tunnel_active = true;
                self.add_log(&format!("Tunnel established at: {}", url));
                Ok(())
            }
            Err(err) => {
                let error_msg = format!("Failed to start tunnel: {}", err);
                self.add_log(&error_msg);
                self.connection_error = Some(error_msg);
                Ok(()) // Return Ok to prevent app crash
            }
        }
    }

    pub async fn stop_tunnel(&mut self) -> Result<()> {
        if let Some(mut process) = self.tunnel_process.take() {
            self.add_log("Stopping tunnel...");

            // Kill the process
            if let Err(e) = process.kill().await {
                self.add_log(&format!("Error stopping tunnel: {}", e));
            } else {
                self.add_log("Tunnel stopped");
            }

            self.tunnel_active = false;
            self.tunnel_url = None;
            self.connection_error = None;
        }

        Ok(())
    }

    pub fn add_log(&mut self, message: &str) {
        self.logs.push(format!(
            "[{}] {}",
            chrono::Local::now().format("%H:%M:%S"),
            message
        ));
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }

    pub fn scroll_logs_up(&mut self) {
        if self.log_offset < self.logs.len() {
            self.log_offset += 1;
        }
    }

    pub fn scroll_logs_down(&mut self) {
        if self.log_offset > 0 {
            self.log_offset -= 1;
        }
    }

    pub fn visible_logs(&self) -> Vec<&String> {
        let start = if self.logs.len() > self.log_offset {
            self.logs.len() - self.log_offset
        } else {
            0
        };

        self.logs.iter().skip(start).collect()
    }
}