use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table,
    },
};

use crate::context::UiState;
use crate::state::AppState;
use crate::ui::rule_type_color;

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let block = Block::default()
        .title(" 规则列表 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.rules.is_empty() {
        let empty = Paragraph::new("暂无规则数据")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    let header = Row::new(vec!["#", "类型", "规则", "代理组"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let filtered: Vec<_> = state
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            if ui.search_query.is_empty() {
                return true;
            }
            r.payload
                .to_lowercase()
                .contains(&ui.search_query.to_lowercase())
                || r.rule_type
                    .to_lowercase()
                    .contains(&ui.search_query.to_lowercase())
                || r.proxy
                    .to_lowercase()
                    .contains(&ui.search_query.to_lowercase())
        })
        .collect();

    if filtered.is_empty() {
        let empty = Paragraph::new("没有匹配的规则")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    ui.rule_selected = ui.rule_selected.min(filtered.len().saturating_sub(1));
    let visible_rows = inner.height.saturating_sub(1) as usize;
    let start = visible_start(ui.rule_selected, visible_rows, filtered.len());

    let rows: Vec<_> = filtered
        .iter()
        .skip(start)
        .take(visible_rows)
        .enumerate()
        .map(|(offset, (_idx, r))| {
            let row_index = start + offset;
            let is_selected = row_index == ui.rule_selected;
            let base_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(30, 30, 40))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let type_color = rule_type_color(&r.rule_type);
            let type_badge = format!(" {} ", r.rule_type.to_uppercase());

            Row::new(vec![
                Cell::from(format!("{}", row_index + 1)).style(base_style.fg(Color::DarkGray)),
                Cell::from(Span::styled(
                    type_badge,
                    Style::default().fg(Color::Black).bg(type_color),
                )),
                Cell::from(r.payload.clone()).style(base_style.fg(Color::White)),
                Cell::from(r.proxy.clone()).style(base_style.fg(Color::Yellow)),
            ])
            .height(1)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Percentage(50),
            Constraint::Min(0),
        ],
    )
    .header(header)
    .column_spacing(1);

    f.render_widget(table, inner);

    let mut scrollbar_state = ScrollbarState::new(filtered.len())
        .position(ui.rule_selected)
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
