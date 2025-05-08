use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

mod app;
mod tunnel;
mod ui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to expose
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Domain to use (e.g., your-subdomain.dev.peril.lol)
    #[arg(short, long)]
    domain: Option<String>,

    /// Public port to listen on for the tunnel server (only relevant when running in server mode)
    #[arg(short = 'P', long, default_value_t = 8000)]
    public_port: u16,

    /// Run in server mode (tunnel server) instead of client mode (tunnel client)
    #[arg(short, long)]
    server: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Check if running in server mode
    if args.server {
        println!("Starting tunnel server on port {}", args.public_port);
        tunnel::server::run(args.public_port).await?;
        return Ok(());
    }

    // Client mode - Show TUI
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = app::App::new(args.port, args.domain, args.public_port);

    // Run app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut app::App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw::<B>(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => {
                    if app.tunnel_active {
                        app.stop_tunnel().await?;
                    }
                    return Ok(());
                }
                KeyCode::Char('s') => {
                    if !app.tunnel_active {
                        app.start_tunnel().await?;
                    } else {
                        app.stop_tunnel().await?;
                    }
                }
                KeyCode::Up => app.scroll_logs_up(),
                KeyCode::Down => app.scroll_logs_down(),
                _ => {}
            }
        }
    }
}