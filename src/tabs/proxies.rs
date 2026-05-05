use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
};

use crate::context::UiState;
use crate::state::{AppState, Proxy};
use crate::ui::{Popup, delay_color, proxy_color};

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let block = Block::default()
        .title(" 代理组 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render popup FIRST (before any early returns)
    if let Popup::ProxyNodes {
        proxy_name,
        selected,
    } = &ui.popup
        && let Some(proxy) = state.proxies.iter().find(|p| p.name == *proxy_name)
    {
        render_proxy_nodes_popup(f, proxy, *selected, state, inner);
    }

    if state.proxies.is_empty() {
        let empty = Paragraph::new("暂无代理组数据")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    // Prepare visible items with filtering
    let items: Vec<_> = state
        .proxies
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if ui.search_query.is_empty() {
                return true;
            }
            p.name
                .to_lowercase()
                .contains(&ui.search_query.to_lowercase())
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new("没有匹配的代理组")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    ui.proxy_selected = ui.proxy_selected.min(items.len().saturating_sub(1));
    let visible_rows = inner.height.saturating_sub(1) as usize;
    let start = visible_start(ui.proxy_selected, visible_rows, items.len());

    let header = Row::new(vec!["名称", "类型", "当前节点", "延迟", "节点数"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<_> = items
        .iter()
        .skip(start)
        .take(visible_rows)
        .enumerate()
        .map(|(offset, (_idx, p))| {
            let row_index = start + offset;
            let is_selected = row_index == ui.proxy_selected;
            let delay = if p.history.is_empty() {
                "-".to_string()
            } else {
                format!("{}ms", p.history[0].delay)
            };
            let node_count = state
                .all_proxies
                .get(&p.name)
                .and_then(|ap| ap.all.as_ref())
                .map(|a| a.len().to_string())
                .unwrap_or_else(|| "-".to_string());

            let base_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(30, 30, 40))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let delay_style = if is_selected {
                base_style.fg(delay_color(p.history.first().map(|h| h.delay).unwrap_or(0)))
            } else {
                Style::default().fg(delay_color(p.history.first().map(|h| h.delay).unwrap_or(0)))
            };

            Row::new(vec![
                Cell::from(p.name.clone()).style(base_style.fg(Color::White)),
                Cell::from(p.proxy_type.clone()).style(base_style.fg(proxy_color(&p.proxy_type))),
                Cell::from(p.now.clone().unwrap_or_else(|| "-".to_string()))
                    .style(base_style.fg(Color::Yellow)),
                Cell::from(delay).style(delay_style),
                Cell::from(node_count).style(base_style.fg(Color::Gray)),
            ])
            .height(1)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Length(12),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .column_spacing(1)
    .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);

    f.render_widget(table, inner);

    // Render scrollbar
    let mut scrollbar_state = ScrollbarState::new(items.len())
        .position(ui.proxy_selected)
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

fn render_proxy_nodes_popup(
    f: &mut Frame,
    proxy: &Proxy,
    selected: usize,
    state: &AppState,
    _area: Rect,
) {
    let all_nodes = state
        .all_proxies
        .get(&proxy.name)
        .and_then(|p| p.all.as_ref())
        .cloned()
        .unwrap_or_default();

    if all_nodes.is_empty() {
        return;
    }

    let selected = selected.min(all_nodes.len().saturating_sub(1));
    let area = f.area();
    let popup_width = (area.width * 3 / 5).clamp(30, 60);
    let popup_height = (all_nodes.len() as u16 + 4).min(area.height - 4);

    let popup_area = Rect {
        x: (area.width - popup_width) / 2,
        y: (area.height - popup_height) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" {} ", proxy.name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let visible_rows = inner.height as usize;
    let start = visible_start(selected, visible_rows, all_nodes.len());

    let rows: Vec<_> = all_nodes
        .iter()
        .skip(start)
        .take(visible_rows)
        .enumerate()
        .map(|(offset, node)| {
            let row_index = start + offset;
            let is_selected = row_index == selected;
            let delay = state
                .all_proxies
                .get(node)
                .and_then(|p| p.history.first())
                .map(|h| h.delay)
                .unwrap_or(0);

            let is_current = proxy.now.as_ref().map(|n| n == node).unwrap_or(false);

            let prefix = if is_current { "● " } else { "  " };
            let node_text = format!("{}{}", prefix, node);

            let base_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(40, 40, 60))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let delay_text = if delay > 0 {
                format!("{}ms", delay)
            } else {
                "-".to_string()
            };

            Row::new(vec![
                Cell::from(node_text).style(base_style.fg(if is_current {
                    Color::Green
                } else {
                    Color::White
                })),
                Cell::from(delay_text).style(base_style.fg(delay_color(delay))),
            ])
        })
        .collect();

    let table =
        Table::new(rows, [Constraint::Percentage(70), Constraint::Length(10)]).column_spacing(1);

    f.render_widget(table, inner);

    // Popup scrollbar
    let mut scrollbar_state = ScrollbarState::new(all_nodes.len()).position(selected);
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
