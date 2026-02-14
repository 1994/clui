use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
};

use crate::app::App;
use crate::ui::format_bytes;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(4),  // Status + Mode
            Constraint::Length(10), // Traffic stats
            Constraint::Min(0),     // Recent connections or info
        ])
        .split(area);

    // Status section
    render_status(f, app, chunks[0]);

    // Traffic section
    render_traffic(f, app, chunks[1]);

    // Info/Connections section
    render_info(f, app, chunks[2]);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Status block
    let status = if app.version.is_some() {
        ("● 运行中", Color::Green)
    } else {
        ("● 未连接", Color::Red)
    };

    let mut status_lines = vec![
        Line::from(vec![Span::styled(
            status.0,
            Style::default().fg(status.1).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            format!(
                "代理组: {}  |  连接数: {}",
                app.proxies.len(),
                app.connections.len()
            ),
            Style::default().fg(Color::Gray),
        )]),
    ];

    // Add memory usage if available (Meta only)
    if let Some(ref memory) = app.memory {
        let mem_percent = (memory.inuse as f64 / memory.os_limit as f64 * 100.0).min(100.0);
        let mem_color = if mem_percent > 80.0 {
            Color::Red
        } else if mem_percent > 50.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        status_lines.push(Line::from(vec![Span::styled(
            format!(
                "内存使用: {} / {} ({:.1}%)",
                format_bytes(memory.inuse),
                format_bytes(memory.os_limit),
                mem_percent
            ),
            Style::default().fg(mem_color),
        )]));
    }

    let status_widget = Paragraph::new(status_lines)
        .block(Block::default().title(" 状态 ").borders(Borders::ALL))
        .alignment(Alignment::Left);
    f.render_widget(status_widget, chunks[0]);

    // Mode block
    let mode = app.get_current_mode();
    let mode_color = match mode {
        "Global" => Color::Magenta,
        "Rule" => Color::Green,
        "Direct" => Color::Blue,
        "Reject" => Color::Red,
        _ => Color::Gray,
    };

    let mode_lines = vec![
        Line::from(vec![Span::styled(
            mode,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "按 m 切换模式",
            Style::default().fg(Color::Gray),
        )]),
    ];

    let mode_widget = Paragraph::new(mode_lines)
        .block(Block::default().title(" 当前模式 ").borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(mode_widget, chunks[1]);
}

fn render_traffic(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Upload section
    let upload_history: Vec<u64> = app.traffic_history.iter().map(|(up, _)| *up).collect();
    let upload_text = format!(
        "↑ 上传\n{}\n{}/s",
        format_bytes(app.upload_total),
        format_bytes(app.traffic.up)
    );

    let upload_inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(chunks[0]);

    let upload_widget = Paragraph::new(upload_text)
        .block(
            Block::default()
                .title(" 上传流量 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center);
    f.render_widget(upload_widget, upload_inner[0]);

    if upload_history.len() > 1 {
        let sparkline = Sparkline::default()
            .data(&upload_history)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(sparkline, upload_inner[1]);
    }

    // Download section
    let download_history: Vec<u64> = app.traffic_history.iter().map(|(_, down)| *down).collect();
    let download_text = format!(
        "↓ 下载\n{}\n{}/s",
        format_bytes(app.download_total),
        format_bytes(app.traffic.down)
    );

    let download_inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(chunks[1]);

    let download_widget = Paragraph::new(download_text)
        .block(
            Block::default()
                .title(" 下载流量 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .style(Style::default().fg(Color::Blue))
        .alignment(Alignment::Center);
    f.render_widget(download_widget, download_inner[0]);

    if download_history.len() > 1 {
        let sparkline = Sparkline::default()
            .data(&download_history)
            .style(Style::default().fg(Color::Blue))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(sparkline, download_inner[1]);
    }
}

fn render_info(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Active connections table
    let header = Row::new(vec!["目标", "上传", "下载"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .connections
        .iter()
        .take(10)
        .map(|c| {
            let dest = c
                .metadata
                .host
                .as_deref()
                .or(c.metadata.destination_ip.as_deref())
                .unwrap_or("Unknown");

            let process = c
                .process
                .as_ref()
                .map(|p| format!(" [{}]", p))
                .or_else(|| {
                    c.process_path.as_ref().and_then(|p| {
                        p.split('/')
                            .next_back()
                            .or_else(|| p.split('\\').next_back())
                            .map(|s| format!(" [{}]", s))
                    })
                })
                .unwrap_or_default();

            Row::new(vec![
                Cell::from(format!("{}{}", dest, process)),
                Cell::from(format_bytes(c.upload)),
                Cell::from(format_bytes(c.download)),
            ])
        })
        .collect();

    let connections_table = Table::new(
        rows,
        vec![
            Constraint::Percentage(50),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(Block::default().title(" 活跃连接 ").borders(Borders::ALL))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(connections_table, chunks[0]);

    // System info or proxy groups summary
    let proxy_groups_text = app
        .proxies
        .iter()
        .take(10)
        .map(|p| {
            let current = p.now.as_deref().unwrap_or("-");
            // Get delay if available
            let delay_str = p
                .history
                .last()
                .map(|h| format!(" ({}ms)", h.delay))
                .unwrap_or_default();
            format!("{} → {}{}", p.name, current, delay_str)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let info_widget = Paragraph::new(proxy_groups_text)
        .block(
            Block::default()
                .title(" 当前节点选择 ")
                .borders(Borders::ALL),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(info_widget, chunks[1]);
}
