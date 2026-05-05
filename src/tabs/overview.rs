use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table},
};

use crate::context::UiState;
use crate::state::AppState;
use crate::ui::{format_bytes, format_duration};

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(area);

    render_traffic_header(f, state, ui, chunks[0]);
    render_main_body(f, state, ui, chunks[1]);
}

fn render_traffic_header(f: &mut Frame, state: &AppState, ui: &UiState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Download block
    let down_block = Block::default()
        .title(" 📥 下载流量 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    let down_inner = down_block.inner(chunks[0]);
    f.render_widget(down_block, chunks[0]);

    let down_total = format_bytes(state.download_total);
    let down_rate = ui
        .traffic_history
        .back()
        .map(|(_, d)| format_bytes(*d) + "/s")
        .unwrap_or_else(|| "0 B/s".to_string());

    let down_sparkline = Sparkline::default()
        .data(
            ui.traffic_history
                .iter()
                .map(|(_, d)| *d)
                .collect::<Vec<_>>(),
        )
        .max(
            ui.traffic_history
                .iter()
                .map(|(_, d)| *d)
                .max()
                .unwrap_or(1)
                .max(1),
        )
        .style(Style::default().fg(Color::Blue))
        .direction(ratatui::widgets::RenderDirection::LeftToRight);

    let down_text = Paragraph::new(vec![
        Line::from(Span::styled(
            down_total,
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            down_rate,
            Style::default().fg(Color::LightBlue),
        )),
    ]);

    let down_inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(down_inner);
    f.render_widget(down_text, down_inner_chunks[0]);
    f.render_widget(down_sparkline, down_inner_chunks[1]);

    // Upload block
    let up_block = Block::default()
        .title(" 📤 上传流量 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let up_inner = up_block.inner(chunks[1]);
    f.render_widget(up_block, chunks[1]);

    let up_total = format_bytes(state.upload_total);
    let up_rate = ui
        .traffic_history
        .back()
        .map(|(u, _)| format_bytes(*u) + "/s")
        .unwrap_or_else(|| "0 B/s".to_string());

    let up_sparkline = Sparkline::default()
        .data(
            ui.traffic_history
                .iter()
                .map(|(u, _)| *u)
                .collect::<Vec<_>>(),
        )
        .max(
            ui.traffic_history
                .iter()
                .map(|(u, _)| *u)
                .max()
                .unwrap_or(1)
                .max(1),
        )
        .style(Style::default().fg(Color::Green))
        .direction(ratatui::widgets::RenderDirection::LeftToRight);

    let up_text = Paragraph::new(vec![
        Line::from(Span::styled(
            up_total,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            up_rate,
            Style::default().fg(Color::LightGreen),
        )),
    ]);

    let up_inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(up_inner);
    f.render_widget(up_text, up_inner_chunks[0]);
    f.render_widget(up_sparkline, up_inner_chunks[1]);
}

fn render_main_body(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_info_panel(f, state, chunks[0]);
    render_connections_table(f, state, ui, chunks[1]);
}

fn render_info_panel(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title(" 系统信息 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![];

    if let Some(ref ver) = state.version {
        let meta_str = if ver.meta == Some(true) {
            "Meta"
        } else {
            "Clash"
        };
        lines.push(Line::from(vec![
            Span::styled("内核: ", Style::default().fg(Color::Gray)),
            Span::styled(meta_str, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("版本: ", Style::default().fg(Color::Gray)),
            Span::styled(&ver.version, Style::default().fg(Color::White)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("状态: ", Style::default().fg(Color::Gray)),
            Span::styled("未连接", Style::default().fg(Color::Red)),
        ]));
    }

    if let Some(ref mem) = state.memory {
        let mem_mb = mem.inuse as f64 / 1024.0 / 1024.0;
        let mem_pct = (mem_mb / 512.0).min(1.0); // assume 512MB max gauge
        lines.push(Line::from(vec![
            Span::styled("内存使用: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{:.1} MB", mem_mb),
                Style::default().fg(Color::White),
            ),
        ]));

        // Memory gauge
        let gauge = Gauge::default()
            .percent((mem_pct * 100.0) as u16)
            .label("")
            .style(Style::default().fg(if mem_pct > 0.8 {
                Color::Red
            } else {
                Color::Cyan
            }))
            .ratio(mem_pct);
        f.render_widget(gauge, {
            let mut r = inner;
            r.y += lines.len() as u16 + 2;
            r.height = 1;
            r
        });
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "代理入口",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    let proxy_config = state.proxy_config.as_ref().or(state.config.as_ref());
    if let Some(config) = proxy_config {
        let host = proxy_host(config);
        if let Some(port) = config.mixed_port {
            lines.push(key_value_line(
                "Mixed: ",
                format!("{}:{}", host, port),
                Color::Green,
            ));
            lines.push(key_value_line("用途: ", "系统/浏览器代理", Color::Gray));
        } else {
            lines.push(key_value_line("Mixed: ", "未启用", Color::Yellow));
        }
        if let Some(port) = config.port {
            lines.push(key_value_line(
                "HTTP: ",
                format!("{}:{}", host, port),
                Color::White,
            ));
        }
        if let Some(port) = config.socks_port {
            lines.push(key_value_line(
                "SOCKS: ",
                format!("{}:{}", host, port),
                Color::White,
            ));
        }
        if let Some(port) = config.redir_port {
            lines.push(key_value_line(
                "Redir: ",
                format!("{}:{}", host, port),
                Color::White,
            ));
        }
        if let Some(port) = config.tproxy_port {
            lines.push(key_value_line(
                "TProxy: ",
                format!("{}:{}", host, port),
                Color::White,
            ));
        }
        let lan = if config.allow_lan { "允许" } else { "关闭" };
        lines.push(key_value_line(
            "局域网: ",
            format!("{} {}", lan, bind_label(&config.bind_address)),
            if config.allow_lan {
                Color::Green
            } else {
                Color::Gray
            },
        ));
    } else {
        lines.push(key_value_line("Mixed: ", "等待配置加载", Color::Yellow));
    }
    if !state.api_url.is_empty() {
        lines.push(key_value_line(
            "控制 API: ",
            api_addr(&state.api_url),
            Color::Gray,
        ));
    }
    if !state.config_path.is_empty() {
        lines.push(key_value_line(
            "配置文件: ",
            compact_path(&state.config_path, inner.width.saturating_sub(12) as usize),
            Color::DarkGray,
        ));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("活跃连接: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}", state.connections.len()),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("代理节点: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}", state.all_proxies.len()),
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("规则数量: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{}", state.rules.len()),
            Style::default().fg(Color::White),
        ),
    ]));

    let info = Paragraph::new(lines).alignment(Alignment::Left);
    f.render_widget(info, inner);
}

fn key_value_line(label: &str, value: impl Into<String>, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(Color::Gray)),
        Span::styled(value.into(), Style::default().fg(color)),
    ])
}

fn proxy_host(config: &crate::state::Config) -> String {
    if config.allow_lan {
        let bind = config.bind_address.trim();
        if !bind.is_empty() && bind != "*" && bind != "0.0.0.0" {
            bind.to_string()
        } else {
            "127.0.0.1".to_string()
        }
    } else {
        "127.0.0.1".to_string()
    }
}

fn bind_label(bind_address: &str) -> String {
    let bind = bind_address.trim();
    if bind.is_empty() {
        String::new()
    } else {
        format!("bind={}", bind)
    }
}

fn api_addr(api_url: &str) -> String {
    api_url
        .strip_prefix("http://")
        .or_else(|| api_url.strip_prefix("https://"))
        .unwrap_or(api_url)
        .to_string()
}

fn compact_path(path: &str, max_width: usize) -> String {
    let char_count = path.chars().count();
    if max_width == 0 || char_count <= max_width {
        return path.to_string();
    }
    if max_width <= 3 {
        return "...".to_string();
    }

    let keep = max_width - 3;
    let suffix: String = path
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("...{}", suffix)
}

fn render_connections_table(f: &mut Frame, state: &AppState, _ui: &UiState, area: Rect) {
    let block = Block::default()
        .title(" 活跃连接 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.connections.is_empty() {
        let empty = Paragraph::new("暂无活跃连接")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    let header = Row::new(vec!["主机", "目标", "类型", "上行", "下行", "时间"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<_> = state
        .connections
        .iter()
        .take(inner.height.saturating_sub(2) as usize)
        .map(|conn| {
            let host = conn.metadata.host.clone().unwrap_or_else(|| {
                conn.metadata
                    .destination_ip
                    .clone()
                    .unwrap_or_else(|| "-".to_string())
            });
            let target = conn
                .chains
                .last()
                .cloned()
                .unwrap_or_else(|| "-".to_string());
            Row::new(vec![
                Cell::from(host).style(Style::default().fg(Color::White)),
                Cell::from(target).style(Style::default().fg(Color::Yellow)),
                Cell::from(conn.metadata.network.clone()).style(Style::default().fg(Color::Gray)),
                Cell::from(format_bytes(conn.upload)).style(Style::default().fg(Color::Green)),
                Cell::from(format_bytes(conn.download)).style(Style::default().fg(Color::Blue)),
                Cell::from(format_duration(&conn.start))
                    .style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);
}
