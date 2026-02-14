use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(1)
        .constraints([ratatui::layout::Constraint::Min(0)])
        .split(area);

    render_logs_list(f, app, chunks[0]);
}

fn render_logs_list(f: &mut Frame, app: &App, area: Rect) {
    let visible_rows = (area.height as usize).saturating_sub(2);
    let total_logs = app.logs.len();

    // 确保 scroll 位置有效（防止日志被清空后 scroll 超出范围）
    let start = app.log_scroll.min(total_logs.saturating_sub(1));

    // Collect logs in reverse order (newest first)
    let log_lines: Vec<Line> = app
        .logs
        .iter()
        .rev()
        .skip(start)
        .take(visible_rows)
        .map(|log| {
            // Parse log level and apply color
            let color = if log.contains("ERROR") {
                Color::Red
            } else if log.contains("WARN") {
                Color::Yellow
            } else if log.contains("DEBUG") {
                Color::Gray
            } else {
                Color::White
            };

            Line::from(Span::styled(log.clone(), Style::default().fg(color)))
        })
        .collect();

    let logs_widget = Paragraph::new(log_lines)
        .block(
            Block::default()
                .title(format!(
                    " 日志 ({}/{}) ",
                    total_logs.saturating_sub(start),
                    total_logs
                ))
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(logs_widget, area);

    // Render scrollbar
    if total_logs > visible_rows {
        let scrollbar_state = ScrollbarState::new(total_logs)
            .position(start)
            .content_length(total_logs)
            .viewport_content_length(visible_rows);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state.clone());
    }
}
