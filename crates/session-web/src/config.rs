use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "session-web", about = "AI Session Viewer Web Server")]
pub struct Config {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1", env = "ASV_HOST")]
    pub host: String,

    /// Port to listen on
    #[arg(long, default_value_t = 3000, env = "ASV_PORT")]
    pub port: u16,

    /// Bearer token for authentication (optional, no auth if not set)
    #[arg(long, env = "ASV_TOKEN")]
    pub token: Option<String>,
}
