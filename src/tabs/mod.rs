pub mod connections;
pub mod logs;
pub mod overview;
pub mod providers;
pub mod proxies;
pub mod rules;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
};

use crate::context::UiState;
use crate::state::AppState;
use crate::ui::Tab;

// ========== Main Render ==========

pub fn render(f: &mut Frame, state: &AppState, ui: &mut UiState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // compact header
            Constraint::Min(0),    // main
            Constraint::Length(1), // footer
        ])
        .split(area);

    render_header(f, state, ui, chunks[0]);

    let main_area = if ui.error_message.is_some() || ui.success_message.is_some() {
        let msg_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(chunks[1]);
        render_toast(f, ui, msg_chunks[0]);
        msg_chunks[1]
    } else {
        chunks[1]
    };

    match ui.current_tab {
        Tab::Overview => overview::render(f, state, ui, main_area),
        Tab::Proxies => proxies::render(f, state, ui, main_area),
        Tab::Providers => providers::render(f, state, ui, main_area),
        Tab::Connections => connections::render(f, state, ui, main_area),
        Tab::Rules => rules::render(f, state, ui, main_area),
        Tab::Logs => logs::render(f, state, ui, main_area),
    }

    render_footer(f, ui, chunks[2]);
}

// ========== Header ==========

fn render_header(f: &mut Frame, state: &AppState, ui: &UiState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(14),
            Constraint::Min(0),
            Constraint::Length(28),
        ])
        .split(area);

    // Logo
    let logo = Paragraph::new("⚡ ClashTUI").style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(logo, chunks[0]);

    // Tabs
    let titles: Vec<_> = [
        Tab::Overview,
        Tab::Proxies,
        Tab::Providers,
        Tab::Connections,
        Tab::Rules,
        Tab::Logs,
    ]
    .iter()
    .map(|t| Line::from(t.title()))
    .collect();

    let tabs = Tabs::new(titles)
        .select(ui.tab_index)
        .style(Style::default().fg(Color::Gray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .divider("");
    f.render_widget(tabs, chunks[1]);

    // Search indicator overlay
    if ui.search_active {
        let search_text = format!("/{}█", ui.search_query);
        let search_para = Paragraph::new(search_text)
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        f.render_widget(search_para, chunks[1]);
    }

    // Status badges
    let mut badges = vec![];
    if state.version.is_some() {
        let mode = ui
            .modes
            .get(ui.current_mode_index)
            .map(|s| s.as_str())
            .unwrap_or("Rule");
        let mode_color = match mode {
            "Global" => Color::Magenta,
            "Rule" => Color::Green,
            "Direct" => Color::Blue,
            "Reject" => Color::Red,
            _ => Color::Gray,
        };
        badges.push(Span::styled("●", Style::default().fg(Color::Green)));
        badges.push(Span::raw(" "));
        badges.push(Span::styled(
            format!(" {} ", mode),
            Style::default().fg(Color::Black).bg(mode_color),
        ));
    } else {
        badges.push(Span::styled("● 离线", Style::default().fg(Color::Red)));
    }

    if let Some(ref mem) = state.memory {
        badges.push(Span::raw(" "));
        let mem_mb = mem.inuse / 1024 / 1024;
        badges.push(Span::styled(
            format!(" {}MB ", mem_mb),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ));
    }

    let status = Paragraph::new(Line::from(badges)).alignment(Alignment::Right);
    f.render_widget(status, chunks[2]);
}

// ========== Footer ==========

fn render_footer(f: &mut Frame, ui: &UiState, area: Rect) {
    let shortcuts = match ui.current_tab {
        Tab::Overview => "q退出 r刷新 m模式 F1帮助",
        Tab::Proxies => "↑↓选择 Enter切换节点 s测速 F4排序 /搜索 F1帮助",
        Tab::Providers => "↑↓选择 a添加 e编辑 d删除 u更新 h检查 F4排序 /搜索 F1帮助",
        Tab::Connections => "↑↓选择 d关闭 c关闭全部 F9关闭 F4排序 /搜索 F1帮助",
        Tab::Rules => "↑↓选择 F4排序 /搜索 F1帮助",
        Tab::Logs => "↑↓滚动 jk滚动 PgUp/PgDn翻页 c清空 /搜索 F1帮助",
    };

    let footer = Paragraph::new(shortcuts)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(footer, area);
}

// ========== Toast ==========

fn render_toast(f: &mut Frame, ui: &UiState, area: Rect) {
    if let Some(ref msg) = ui.success_message {
        let widget = Paragraph::new(msg.as_str())
            .style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        f.render_widget(widget, area);
    } else if let Some(ref msg) = ui.error_message {
        let widget = Paragraph::new(msg.as_str())
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(widget, area);
    }
}

// ========== Help Popup ==========

pub fn render_help(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(75, 80, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" 按键帮助 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let text = Text::from(vec![
        Line::from(vec![Span::styled(
            "全局",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  q / Esc       退出"),
        Line::from("  1~6           切换标签页"),
        Line::from("  Tab           下一个标签"),
        Line::from("  r / F5        刷新数据"),
        Line::from("  m / F6        切换代理模式"),
        Line::from("  /             搜索过滤"),
        Line::from("  F1            显示/关闭帮助"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "代理",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑↓            选择代理组"),
        Line::from("  Enter/Space   展开节点选择"),
        Line::from("  s             测速当前组"),
        Line::from("  S             测速全部组"),
        Line::from("  ←→            切换节点（无需展开）"),
        Line::from("  F4            排序"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "订阅",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  a             添加订阅"),
        Line::from("  e             编辑订阅"),
        Line::from("  d             删除订阅"),
        Line::from("  u             更新当前订阅"),
        Line::from("  U             更新全部订阅"),
        Line::from("  h             健康检查"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "连接",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  d / F9        关闭选中连接"),
        Line::from("  c             关闭全部连接"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "日志",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j / ↑         向上滚动"),
        Line::from("  k / ↓         向下滚动"),
        Line::from("  c             清空日志"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "按任意键关闭",
            Style::default().fg(Color::Gray),
        )]),
    ]);

    let para = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, popup_area);
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
