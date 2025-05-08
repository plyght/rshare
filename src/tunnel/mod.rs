pub mod client;
pub mod server;

pub struct TunnelResult {
    pub url: String,
    pub process: tokio::process::Child,
}
