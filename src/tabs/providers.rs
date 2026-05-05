use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
};

use crate::context::UiState;
use crate::state::AppState;
use crate::ui::{Popup, format_bytes, format_duration};

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let block = Block::default()
        .title(" 订阅管理 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let provider_items: Vec<_> = state
        .providers
        .iter()
        .filter(|p| p.subscription_info.is_some() || !p.vehicle_type.is_empty())
        .collect();

    if provider_items.is_empty() {
        let empty = Paragraph::new("暂无订阅，按 a 添加")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
    } else {
        ui.provider_selected = ui
            .provider_selected
            .min(provider_items.len().saturating_sub(1));
        let visible_rows = inner.height.saturating_sub(1) as usize;
        let start = visible_start(ui.provider_selected, visible_rows, provider_items.len());

        let header = Row::new(vec!["名称", "类型", "状态", "节点数", "更新时间", "流量"])
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .height(1);

        let rows: Vec<_> = provider_items
            .iter()
            .skip(start)
            .take(visible_rows)
            .enumerate()
            .map(|(offset, p)| {
                let row_index = start + offset;
                let is_selected = row_index == ui.provider_selected;
                let base_style = if is_selected {
                    Style::default()
                        .bg(Color::Rgb(30, 30, 40))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let (status_text, status_color) = if ui.updating_providers.contains(&p.name) {
                    ("更新中".to_string(), Color::Cyan)
                } else if !p.proxies.is_empty() {
                    ("正常".to_string(), Color::Green)
                } else if p.updated_at.is_none() || p.updated_at.as_ref().unwrap().is_empty() {
                    ("待更新".to_string(), Color::Yellow)
                } else {
                    ("无节点".to_string(), Color::Red)
                };

                let traffic = p
                    .subscription_info
                    .as_ref()
                    .map(|info| {
                        let used = info.upload + info.download;
                        let total = info.total;
                        format!("{} / {}", format_bytes(used), format_bytes(total))
                    })
                    .unwrap_or_else(|| "-".to_string());

                Row::new(vec![
                    Cell::from(p.name.clone()).style(base_style.fg(Color::White)),
                    Cell::from(p.vehicle_type.clone()).style(base_style.fg(Color::Magenta)),
                    Cell::from(status_text).style(base_style.fg(status_color)),
                    Cell::from(format!("{}", p.proxies.len())).style(base_style.fg(Color::Gray)),
                    Cell::from(format_duration(p.updated_at.as_deref().unwrap_or("")))
                        .style(base_style.fg(Color::DarkGray)),
                    Cell::from(traffic).style(base_style.fg(Color::Cyan)),
                ])
                .height(1)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(25),
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Min(0),
            ],
        )
        .header(header)
        .column_spacing(1);

        f.render_widget(table, inner);

        let mut scrollbar_state = ScrollbarState::new(provider_items.len())
            .position(ui.provider_selected)
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

    match &ui.popup {
        Popup::AddProvider => render_form_popup(
            f,
            "添加订阅",
            &ui.input_fields,
            ui.input_field_index,
            ui.input_cursor,
            area,
        ),
        Popup::EditProvider => render_form_popup(
            f,
            "编辑订阅",
            &ui.input_fields,
            ui.input_field_index,
            ui.input_cursor,
            area,
        ),
        Popup::DeleteConfirm => render_confirm_popup(f, "确认删除？", area),
        _ => {}
    }
}

fn render_form_popup(
    f: &mut Frame,
    title: &str,
    fields: &[String],
    field_idx: usize,
    cursor: usize,
    area: Rect,
) {
    let popup_area = centered_rect(60, 55, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let labels = ["名称:", "URL:", "更新间隔(秒):"];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    for (i, (label, value)) in labels.iter().zip(fields.iter()).enumerate() {
        let is_active = i == field_idx;

        // Label
        let label_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let label_area = Rect {
            x: chunks[i].x,
            y: chunks[i].y,
            width: chunks[i].width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(format!("{} ", label)).style(label_style),
            label_area,
        );

        // Input area: 2 lines (bottom border + 1 line of text)
        let mut input_area = chunks[i];
        input_area.y += 1;
        input_area.height = 2;

        let input_block = if is_active {
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Rgb(25, 25, 35)))
        } else {
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray))
        };

        let mut text = value.clone();
        if is_active {
            // cursor is char-count; find correct byte index for insert
            let byte_idx = text
                .char_indices()
                .map(|(i, _)| i)
                .nth(cursor)
                .unwrap_or(text.len());
            text.insert(byte_idx, '|');
        }

        let text_style = if is_active {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        let para = Paragraph::new(text).block(input_block).style(text_style);
        f.render_widget(para, input_area);
    }

    let hint = Paragraph::new("Tab/↑↓切换字段  Enter确认  Esc取消")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, chunks[3]);
}

fn render_confirm_popup(f: &mut Frame, message: &str, area: Rect) {
    let popup_area = centered_rect(40, 20, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" 确认 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let msg = Paragraph::new(message)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(msg, chunks[0]);

    let hint = Paragraph::new("Enter确认  Esc取消")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);
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

fn visible_start(selected: usize, visible_rows: usize, total_rows: usize) -> usize {
    if visible_rows == 0 || total_rows <= visible_rows {
        return 0;
    }

    let half_page = visible_rows / 2;
    selected
        .saturating_sub(half_page)
        .min(total_rows.saturating_sub(visible_rows))
}
