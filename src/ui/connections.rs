use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
};

use crate::app::App;
use crate::ui::{format_bytes, format_duration};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_summary(f, app, chunks[0]);
    render_connections_table(f, app, chunks[1]);
}

fn render_summary(f: &mut Frame, app: &App, area: Rect) {
    let text = Span::styled(
        format!(
            "总下载: {}  |  总上传: {}  |  活跃连接: {}",
            format_bytes(app.download_total),
            format_bytes(app.upload_total),
            app.connections.len()
        ),
        Style::default().fg(Color::Cyan),
    );

    let summary = ratatui::widgets::Paragraph::new(text)
        .block(Block::default().title(" 流量统计 ").borders(Borders::ALL))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(summary, area);
}

fn render_connections_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["主机", "网络", "进程", "上传", "下载", "时长"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let visible_rows = (area.height as usize).saturating_sub(5);
    let start = app.connection_scroll;

    let rows: Vec<Row> = app
        .connections
        .iter()
        .skip(start)
        .take(visible_rows)
        .enumerate()
        .map(|(idx, conn)| {
            let actual_idx = start + idx;
            let is_selected = actual_idx == app.connection_scroll;

            let host = conn
                .metadata
                .host
                .as_deref()
                .or(conn.metadata.destination_ip.as_deref())
                .unwrap_or("-");

            let port = &conn.metadata.destination_port;
            let host_with_port = format!("{}:{}", host, port);

            // Get process info (Meta specific)
            let process = conn
                .process
                .as_deref()
                .or_else(|| {
                    conn.process_path.as_deref().and_then(|p| {
                        p.split('/')
                            .next_back()
                            .or_else(|| p.split('\\').next_back())
                    })
                })
                .unwrap_or("-");

            // Calculate duration from start time
            let duration_str = format_duration(
                chrono::Local::now().timestamp() as u64
                    - chrono::DateTime::parse_from_rfc3339(&conn.start)
                        .map(|d| d.timestamp() as u64)
                        .unwrap_or(0),
            );

            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(host_with_port),
                Cell::from(conn.metadata.network.clone()),
                Cell::from(process.to_string()),
                Cell::from(Span::styled(
                    format_bytes(conn.upload),
                    Style::default().fg(Color::Green),
                )),
                Cell::from(Span::styled(
                    format_bytes(conn.download),
                    Style::default().fg(Color::Blue),
                )),
                Cell::from(duration_str),
            ])
            .style(style)
        })
        .collect();

    let widths = vec![
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(13),
        Constraint::Percentage(13),
        Constraint::Percentage(14),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .title(" 连接列表 (c:关闭全部, d:关闭选中) ")
            .borders(Borders::ALL),
    );

    f.render_widget(table, area);

    // Render scrollbar
    if app.connections.len() > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(app.connections.len())
            .position(app.connection_scroll)
            .content_length(app.connections.len())
            .viewport_content_length(visible_rows);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
