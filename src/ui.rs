use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, AppMode};

pub fn draw<B: Backend>(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(10),
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_header(f, app, chunks[0]);
    draw_status(f, app, chunks[1]);
    draw_logs(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, _app: &App, area: Rect) {
    let title = "rshare - Securely expose localhost to the web";
    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    match app.mode {
        AppMode::ConfigPort => {
            draw_config_input(f, app, area, "Configure Port", "Enter new port value:", &app.input_buffer);
            return;
        }
        AppMode::ConfigServerPort => {
            draw_config_input(f, app, area, "Configure Server Port", "Enter new server port value:", &app.input_buffer);
            return;
        }
        _ => {}
    }

    let status_text;
    let color;

    if let Some(error) = &app.connection_error {
        // Show error state
        status_text = format!("ERROR: {}", error);
        color = Color::Red;
    } else if app.tunnel_active {
        // Show active tunnel
        status_text = format!(
            "Tunnel active: localhost:{} -> {}",
            app.port,
            app.tunnel_url.as_ref().unwrap()
        );
        color = Color::Green;
    } else {
        // Show inactive state
        status_text = format!(
            "Tunnel inactive. Press 's' to start tunnel on port {}",
            app.port
        );
        color = Color::Yellow;
    }

    let help = " [s] Start/Stop  [p] Configure port  [P] Configure server port  [q] Quit  [↑/↓] Scroll logs";

    let paragraphs = [status_text, help.to_string()];
    let text = paragraphs.join("\n");

    let status_widget = Paragraph::new(text)
        .style(Style::default().fg(color))
        .block(Block::default().borders(Borders::ALL).title("Status"));

    f.render_widget(status_widget, area);
}

fn draw_config_input(f: &mut Frame, _app: &App, area: Rect, title: &str, prompt: &str, input: &str) {
    let input_text = format!("{} {}\n[Enter] Save  [Esc] Cancel", prompt, input);
    
    let input_widget = Paragraph::new(Text::from(input_text))
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title(title));
    
    f.render_widget(input_widget, area);
}

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let logs: Vec<ListItem> = app
        .visible_logs()
        .iter()
        .map(|log| ListItem::new(log.as_str()))
        .collect();

    let logs = List::new(logs)
        .block(Block::default().borders(Borders::ALL).title("Logs"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(logs, area);
}