use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Row, Table},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0)])
        .split(area);

    render_proxy_list(f, app, chunks[0]);
}

fn render_proxy_list(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["名称", "类型", "当前节点", "延迟", "特性"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .proxies
        .iter()
        .enumerate()
        .map(|(idx, proxy)| {
            let is_selected = idx == app.selected_proxy_group;
            let expanded = app.proxy_expanded.get(idx).copied().unwrap_or(false);

            let delay = proxy
                .history
                .last()
                .map(|h| {
                    let color = if h.delay < 200 {
                        Color::Green
                    } else if h.delay < 500 {
                        Color::Yellow
                    } else {
                        Color::Red
                    };
                    Cell::from(Span::styled(
                        format!("{} ms", h.delay),
                        Style::default().fg(color),
                    ))
                })
                .unwrap_or_else(|| Cell::from("-"));

            let current = proxy.now.as_deref().unwrap_or("-");
            let prefix = if expanded { "▼ " } else { "▶ " };

            // Build features string (Meta specific)
            let mut features = vec![];
            if proxy.udp.unwrap_or(false) {
                features.push("UDP");
            }
            if proxy.xudp.unwrap_or(false) {
                features.push("XUDP");
            }
            if proxy.tfo.unwrap_or(false) {
                features.push("TFO");
            }
            if proxy.mptcp.unwrap_or(false) {
                features.push("MPTCP");
            }
            let features_str = if features.is_empty() {
                "-".to_string()
            } else {
                features.join(", ")
            };

            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let cells = vec![
                Cell::from(format!("{}{}", prefix, proxy.name)),
                Cell::from(proxy.proxy_type.clone()),
                Cell::from(current.to_string()),
                delay,
                Cell::from(features_str),
            ];

            Row::new(cells).style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" 代理组 (按 s 测速, ←→切换) ")
            .borders(Borders::ALL),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);

    // Render expanded proxy details
    if let Some(proxy) = app.proxies.get(app.selected_proxy_group)
        && let Some(true) = app.proxy_expanded.get(app.selected_proxy_group).copied()
    {
        render_proxy_details(f, app, proxy, area);
    }
}

fn render_proxy_details(f: &mut Frame, app: &App, proxy: &crate::clash::Proxy, area: Rect) {
    if let Some(ref all) = proxy.all {
        let popup_area = centered_rect(60, 50, area);

        let block = Block::default()
            .title(format!(" {} - 可用节点 ", proxy.name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let items: Vec<ListItem> = all
            .iter()
            .map(|name| {
                let is_current = proxy.now.as_ref() == Some(name);

                // Try to find delay for this node
                let delay_info = app
                    .all_proxies
                    .get(name)
                    .and_then(|p| p.history.last())
                    .map(|h| {
                        let _color = if h.delay < 200 {
                            Color::Green
                        } else if h.delay < 500 {
                            Color::Yellow
                        } else {
                            Color::Red
                        };
                        format!(" ({}ms)", h.delay)
                    })
                    .unwrap_or_default();

                let style = if is_current {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let prefix = if is_current { "● " } else { "○ " };
                ListItem::new(format!("{}{}{}", prefix, name, delay_info)).style(style)
            })
            .collect();

        let list = List::new(items).block(block);

        f.render_widget(Clear, popup_area);
        f.render_widget(list, popup_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
