pub mod connections;
pub mod logs;
pub mod overview;
pub mod providers;
pub mod proxies;
pub mod rules;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::app::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview = 0,
    Proxies = 1,
    Providers = 2,
    Connections = 3,
    Rules = 4,
    Logs = 5,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview => " 概览 ",
            Tab::Proxies => " 代理 ",
            Tab::Providers => " 订阅 ",
            Tab::Connections => " 连接 ",
            Tab::Rules => " 规则 ",
            Tab::Logs => " 日志 ",
        }
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let (header, main, footer) = app.get_main_layout(f.area());

    // Render header
    render_header(f, app, header);

    // Render main content based on current tab
    match app.current_tab {
        Tab::Overview => overview::render(f, app, main),
        Tab::Proxies => proxies::render(f, app, main),
        Tab::Providers => providers::render(f, app, main),
        Tab::Connections => connections::render(f, app, main),
        Tab::Rules => rules::render(f, app, main),
        Tab::Logs => logs::render(f, app, main),
    }

    // Render footer
    render_footer(f, app, footer);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(0)])
        .split(area);

    // Logo/Title
    let title = Paragraph::new("⚡ Clash TUI")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Left);
    f.render_widget(title, chunks[0]);

    // Tabs
    let titles: Vec<_> = [
        Tab::Overview,
        Tab::Proxies,
        Tab::Providers,
        Tab::Connections,
        Tab::Rules,
        Tab::Logs,
    ]
    .iter()
    .map(|t| Line::from(t.title()))
    .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(app.current_tab as usize)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        );
    f.render_widget(tabs, chunks[1]);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut status_parts = vec![];

    if let Some(ref version) = app.version {
        let meta_tag = if version.meta.unwrap_or(false) {
            " (Meta)"
        } else {
            ""
        };
        status_parts.push(format!("Clash{}: {}", meta_tag, version.version));
    } else {
        status_parts.push("未连接".to_string());
    }

    status_parts.push(format!("模式: {}", app.get_current_mode()));

    if let Some(ref memory) = app.memory {
        status_parts.push(format!("内存: {}", format_bytes(memory.inuse)));
    }

    status_parts.push("q:退出".to_string());
    status_parts.push("1-6:切换".to_string());
    status_parts.push("Tab:标签".to_string());

    let status_text = status_parts.join(" | ");

    let footer = Paragraph::new(format!(" {}", status_text))
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(footer, area);
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

pub fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
