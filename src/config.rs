use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub server_port: u16,
    pub domain: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            server_port: 8000,
            domain: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_path()?;
        
        // Check if config file exists
        if !config_path.exists() {
            // Create default config
            let default_config = Config::default();
            let config_dir = config_path.parent().unwrap();
            fs::create_dir_all(config_dir)
                .context("Failed to create config directory")?;
            
            let config_str = serde_json::to_string_pretty(&default_config)
                .context("Failed to serialize default config")?;
            
            fs::write(&config_path, config_str)
                .context("Failed to write default config file")?;
            
            return Ok(default_config);
        }
        
        // Read and parse config file
        let config_str = fs::read_to_string(&config_path)
            .context("Failed to read config file")?;
        
        serde_json::from_str(&config_str)
            .context("Failed to parse config file")
    }
    
    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;
        let config_dir = config_path.parent().unwrap();
        
        fs::create_dir_all(config_dir)
            .context("Failed to create config directory")?;
        
        let config_str = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        fs::write(&config_path, config_str)
            .context("Failed to write config file")?;
        
        Ok(())
    }
}

fn get_config_path() -> Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .context("Failed to determine home directory")?;
    
    let config_dir = home_dir.join(".config").join("rshare");
    let config_file = config_dir.join("config.json");
    
    Ok(config_file)
}