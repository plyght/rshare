use anyhow::Result;
use tokio::process::Child;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::Config;
use crate::tunnel;

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    ConfigPort,
    ConfigServerPort,
}

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
    pub mode: AppMode,
    pub config: Config,
    pub input_buffer: String,
}

impl App {
    pub fn new(port: u16, domain: Option<String>, server_port: u16) -> Self {
        // Load config
        let config = Config::load().unwrap_or_else(|e| {
            eprintln!("Error loading config: {}", e);
            Config::default()
        });
        
        // Command line arguments override config values
        let port = if port != 8080 { port } else { config.port };
        let server_port = if server_port != 8000 { server_port } else { config.server_port };
        let domain = domain.or_else(|| config.domain.clone());
        
        // Update config with any command line overrides
        let config = Config {
            port,
            server_port,
            domain: domain.clone(),
        };
        
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
            mode: AppMode::Normal,
            config,
            input_buffer: String::new(),
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
    
    pub fn enter_config_port_mode(&mut self) {
        self.mode = AppMode::ConfigPort;
        self.input_buffer = self.port.to_string();
    }
    
    pub fn enter_config_server_port_mode(&mut self) {
        self.mode = AppMode::ConfigServerPort;
        self.input_buffer = self.server_port.to_string();
    }
    
    pub fn exit_config_mode(&mut self) {
        self.mode = AppMode::Normal;
        self.input_buffer.clear();
    }
    
    pub fn apply_config(&mut self) -> Result<()> {
        match self.mode {
            AppMode::ConfigPort => {
                if let Ok(port) = self.input_buffer.parse::<u16>() {
                    if port > 0 {
                        self.port = port;
                        self.config.port = port;
                        self.add_log(&format!("Port updated to: {}", port));
                        self.config.save()?;
                    } else {
                        self.add_log("Invalid port: must be greater than 0");
                    }
                } else {
                    self.add_log("Invalid port number");
                }
            }
            AppMode::ConfigServerPort => {
                if let Ok(port) = self.input_buffer.parse::<u16>() {
                    if port > 0 {
                        self.server_port = port;
                        self.config.server_port = port;
                        self.add_log(&format!("Server port updated to: {}", port));
                        self.config.save()?;
                    } else {
                        self.add_log("Invalid port: must be greater than 0");
                    }
                } else {
                    self.add_log("Invalid port number");
                }
            }
            _ => {}
        }
        
        self.exit_config_mode();
        Ok(())
    }
    
    pub fn handle_key_input(&mut self, key_char: char) {
        if self.mode == AppMode::Normal {
            return;
        }
        
        // Allow only digits in port config
        if key_char.is_ascii_digit() {
            self.input_buffer.push(key_char);
        } else if key_char == '\u{8}' || key_char == '\u{7f}' { // backspace
            self.input_buffer.pop();
        }
    }
}