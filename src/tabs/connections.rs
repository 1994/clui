use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
};

use crate::context::UiState;
use crate::state::AppState;
use crate::ui::{format_bytes, format_duration};

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let block = Block::default()
        .title(" 连接管理 ")
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

    let header = Row::new(vec!["主机", "目标", "类型", "进程", "上行", "下行", "时间"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let filtered: Vec<_> = state
        .connections
        .iter()
        .enumerate()
        .filter(|(_, conn)| {
            if ui.search_query.is_empty() {
                return true;
            }
            let host = conn
                .metadata
                .host
                .as_deref()
                .or(conn.metadata.destination_ip.as_deref())
                .unwrap_or("");
            host.to_lowercase()
                .contains(&ui.search_query.to_lowercase())
        })
        .collect();

    if filtered.is_empty() {
        let empty = Paragraph::new("没有匹配的连接")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    ui.connection_selected = ui.connection_selected.min(filtered.len().saturating_sub(1));
    let visible_rows = inner.height.saturating_sub(1) as usize;
    let start = visible_start(ui.connection_selected, visible_rows, filtered.len());

    let rows: Vec<_> = filtered
        .iter()
        .skip(start)
        .take(visible_rows)
        .enumerate()
        .map(|(offset, (_idx, conn))| {
            let row_index = start + offset;
            let is_selected = row_index == ui.connection_selected;
            let base_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(30, 30, 40))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

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
            let process = conn
                .metadata
                .process
                .clone()
                .unwrap_or_else(|| "-".to_string());

            Row::new(vec![
                Cell::from(host).style(base_style.fg(Color::White)),
                Cell::from(target).style(base_style.fg(Color::Yellow)),
                Cell::from(conn.metadata.network.clone()).style(base_style.fg(Color::Magenta)),
                Cell::from(process).style(base_style.fg(Color::Cyan)),
                Cell::from(format_bytes(conn.upload)).style(base_style.fg(Color::Green)),
                Cell::from(format_bytes(conn.download)).style(base_style.fg(Color::Blue)),
                Cell::from(format_duration(&conn.start)).style(base_style.fg(Color::DarkGray)),
            ])
            .height(1)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Length(6),
            Constraint::Percentage(15),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);

    let mut scrollbar_state = ScrollbarState::new(filtered.len())
        .position(ui.connection_selected)
        .viewport_content_length(inner.height.saturating_sub(2) as usize);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█"),
        inner,
        &mut scrollbar_state,
    );
}

fn visible_start(selected: usize, visible_rows: usize, total_rows: usize) -> usize {
    if visible_rows == 0 || total_rows <= visible_rows {
        return 0;
    }

    let half_page = visible_rows / 2;
    selected
        .saturating_sub(half_page)
        .min(total_rows.saturating_sub(visible_rows))
}
