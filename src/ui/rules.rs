use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(1)
        .constraints([ratatui::layout::Constraint::Min(0)])
        .split(area);

    render_rules_table(f, app, chunks[0]);
}

fn render_rules_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["类型", "匹配对象", "代理", "计数"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let visible_rows = (area.height as usize).saturating_sub(5);
    let start = app.rule_scroll;

    let rows: Vec<Row> = app
        .rules
        .iter()
        .skip(start)
        .take(visible_rows)
        .enumerate()
        .map(|(idx, rule)| {
            let actual_idx = start + idx;
            let is_selected = actual_idx == app.rule_scroll;

            let rule_type_color = match rule.rule_type.as_str() {
                "DOMAIN" => Color::Cyan,
                "DOMAIN-SUFFIX" => Color::LightCyan,
                "DOMAIN-KEYWORD" => Color::LightBlue,
                "IP-CIDR" | "IP-CIDR6" => Color::Yellow,
                "GEOIP" => Color::LightYellow,
                "SRC-IP-CIDR" => Color::Magenta,
                "DST-PORT" => Color::Blue,
                "SRC-PORT" => Color::LightBlue,
                "PROCESS-NAME" => Color::Green,
                "MATCH" | "FINAL" => Color::Red,
                _ => Color::Gray,
            };

            let proxy_color = if rule.proxy == "DIRECT" {
                Color::Green
            } else if rule.proxy == "REJECT" {
                Color::Red
            } else if rule.proxy == "GLOBAL" {
                Color::Magenta
            } else {
                Color::Blue
            };

            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            let size_str = rule
                .size
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string());

            Row::new(vec![
                Cell::from(Span::styled(
                    rule.rule_type.clone(),
                    Style::default().fg(rule_type_color),
                )),
                Cell::from(rule.payload.clone()),
                Cell::from(Span::styled(
                    rule.proxy.clone(),
                    Style::default()
                        .fg(proxy_color)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(size_str),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            ratatui::layout::Constraint::Percentage(20),
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(25),
            ratatui::layout::Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(" 规则列表 ({} total) ", app.rules.len()))
            .borders(Borders::ALL),
    );

    f.render_widget(table, area);

    // Render scrollbar
    if app.rules.len() > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(app.rules.len())
            .position(app.rule_scroll)
            .content_length(app.rules.len())
            .viewport_content_length(visible_rows);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
