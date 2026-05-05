use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::context::UiState;
use crate::state::AppState;

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState, area: Rect) {
    let block = Block::default()
        .title(" 日志 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.logs.is_empty() {
        let empty = Paragraph::new("暂无日志")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    let log_lines: Vec<Line> = state
        .logs
        .iter()
        .skip(ui.log_scroll)
        .take(inner.height as usize)
        .map(|log| {
            let (level, color) = if log.contains("[ERROR]") || log.contains(" error ") {
                ("ERR", Color::Red)
            } else if log.contains("[WARN]") || log.contains(" warn ") {
                ("WRN", Color::Yellow)
            } else if log.contains("[INFO]") || log.contains(" info ") {
                ("INF", Color::Green)
            } else if log.contains("[DEBUG]") || log.contains(" debug ") {
                ("DBG", Color::Blue)
            } else {
                ("", Color::Gray)
            };

            if level.is_empty() {
                Line::from(Span::styled(log, Style::default().fg(Color::Gray)))
            } else {
                let parts: Vec<&str> = log.splitn(2, ' ').collect();
                let rest = parts.get(1).unwrap_or(&"");
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", level),
                        Style::default().fg(Color::Black).bg(color),
                    ),
                    Span::raw(" "),
                    Span::styled(*rest, Style::default().fg(Color::Gray)),
                ])
            }
        })
        .collect();

    let para = Paragraph::new(log_lines).wrap(Wrap { trim: false });
    f.render_widget(para, inner);

    let mut scrollbar_state = ScrollbarState::new(state.logs.len())
        .position(ui.log_scroll)
        .viewport_content_length(inner.height as usize);
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
