use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::app::{App, InputMode, PopupMode, char_to_byte_index};
use crate::ui::format_bytes;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    // Check if we need to render a popup
    match app.popup_mode {
        PopupMode::AddProvider | PopupMode::EditProvider => {
            render_provider_form(f, app, area);
        }
        PopupMode::DeleteConfirm => {
            render_delete_confirm(f, app, area);
        }
        PopupMode::None => {
            render_provider_list(f, app, area);
        }
    }
}

fn render_provider_list(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    // Render help text
    let auto_update_status = if app.auto_update_enabled {
        Span::styled("自动更新: 开启", Style::default().fg(Color::Green))
    } else {
        Span::styled("自动更新: 关闭", Style::default().fg(Color::Red))
    };

    let help_line = Line::from(vec![
        auto_update_status,
        Span::raw(" | a:新增 e:编辑 d/D:删除 u:立即更新 U:更新全部 h:健康检查 Enter:展开 ↑↓:选择"),
    ]);
    let help = Paragraph::new(help_line)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(help, chunks[0]);

    // Render success/error message if exists
    let (msg_area, table_area) = if app.success_message.is_some() || app.error_message.is_some() {
        let msg_text = app
            .success_message
            .as_deref()
            .or(app.error_message.as_deref())
            .unwrap_or_default();
        let msg_width = chunks[2].width.saturating_sub(4).max(1) as usize;
        let msg_lines = msg_text
            .lines()
            .map(|line| line.chars().count())
            .map(|count| count.div_ceil(msg_width))
            .sum::<usize>()
            .max(1);
        let msg_height = (msg_lines as u16 + 2).clamp(3, 8);

        let msg_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(msg_height), Constraint::Min(0)])
            .split(chunks[2]);

        if let Some(ref msg) = app.success_message {
            let msg_widget = Paragraph::new(msg.as_str())
                .style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Color::Green),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(msg_widget, msg_chunks[0]);
        } else if let Some(ref msg) = app.error_message {
            let msg_widget = Paragraph::new(msg.as_str())
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Color::Red),
                )
                .wrap(Wrap { trim: false });
            f.render_widget(msg_widget, msg_chunks[0]);
        }
        (Some(chunks[1]), msg_chunks[1])
    } else {
        (Some(chunks[1]), chunks[2])
    };

    // Render auto update toggle hint
    if let Some(area) = msg_area {
        let hint = Paragraph::new("按 A 切换自动更新")
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(hint, area);
    }

    render_provider_table(f, app, table_area);
}

fn render_provider_table(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["名称", "类型", "节点数", "下次更新", "流量"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .providers
        .iter()
        .enumerate()
        .map(|(idx, provider)| {
            let is_selected = idx == app.selected_provider;
            let expanded = app.provider_expanded.get(idx).copied().unwrap_or(false);
            let is_pending = app.missing_config_providers.contains(&provider.name);

            // Get update info
            let update_info = app.get_provider_update_info(&provider.name);
            let next_update_str = app.format_next_update(&provider.name);

            let traffic_str = provider
                .subscription_info
                .as_ref()
                .map(|info| {
                    let used = info.upload + info.download;
                    let total = info.total;
                    if total > 0 {
                        format!(
                            "{}/{} {:.1}%",
                            format_bytes(used),
                            format_bytes(total),
                            (used as f64 / total as f64) * 100.0
                        )
                    } else {
                        format_bytes(used)
                    }
                })
                .unwrap_or_else(|| "-".to_string());

            let prefix = if expanded { "▼ " } else { "▶ " };

            // Show update status
            let name_with_status = if is_pending {
                format!("{} {} ⚠待加载", prefix, provider.name)
            } else if let Some(info) = update_info {
                if info.is_updating {
                    format!("{} {} 🔄", prefix, provider.name)
                } else if info.last_error.is_some() {
                    format!("{} {} ⚠", prefix, provider.name)
                } else {
                    format!("{}{}", prefix, provider.name)
                }
            } else {
                format!("{}{}", prefix, provider.name)
            };

            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if is_pending {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let cells = vec![
                Cell::from(name_with_status),
                Cell::from(provider.vehicle_type.clone()),
                Cell::from(provider.proxies.len().to_string()),
                Cell::from(next_update_str),
                Cell::from(traffic_str),
            ];

            Row::new(cells).style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" 代理订阅 (Providers) ")
            .borders(Borders::ALL),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);

    // Render expanded provider details
    if let Some(provider) = app.providers.get(app.selected_provider)
        && let Some(true) = app.provider_expanded.get(app.selected_provider).copied()
    {
        render_provider_details(f, app, provider, area);
    }
}

fn render_provider_details(
    f: &mut Frame,
    app: &App,
    provider: &crate::clash::Provider,
    area: Rect,
) {
    let popup_area = centered_rect(70, 60, area);

    let block = Block::default()
        .title(format!(" {} - 节点列表 ", provider.name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Show subscription info
    let mut items = vec![];

    // Show update status
    if app.missing_config_providers.contains(&provider.name) {
        items.push(
            ListItem::new("状态: ⚠ 尚未加载到核心（可直接删除/编辑后重试）")
                .style(Style::default().fg(Color::Yellow)),
        );
        items.push(ListItem::new(""));
    }

    if let Some(info) = app.get_provider_update_info(&provider.name) {
        let status_text = if info.is_updating {
            "状态: 🔄 更新中...".to_string()
        } else if let Some(ref err) = info.last_error {
            format!("状态: ⚠ 上次更新失败 - {}", err)
        } else if let Some(last) = info.last_update {
            let ago = last.elapsed().as_secs();
            let time_str = if ago < 60 {
                format!("{}秒前", ago)
            } else if ago < 3600 {
                format!("{}分钟前", ago / 60)
            } else {
                format!("{}小时前", ago / 3600)
            };
            format!("状态: ✅ 上次更新 {}", time_str)
        } else {
            "状态: ⏳ 等待首次更新".to_string()
        };

        let next_update = app.format_next_update(&provider.name);
        items.push(
            ListItem::new(format!("{} | 下次: {}", status_text, next_update))
                .style(Style::default().fg(Color::Yellow)),
        );
        items.push(ListItem::new(""));
    }

    if let Some(ref info) = provider.subscription_info {
        let used = info.upload + info.download;
        let usage_text = format!(
            "流量使用: ↑{} ↓{} / {} (剩余: {})",
            format_bytes(info.upload),
            format_bytes(info.download),
            format_bytes(info.total),
            format_bytes(info.total.saturating_sub(used))
        );

        let expire_text = if info.expire > 0 {
            let expire_date = chrono::DateTime::from_timestamp(info.expire as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            format!("过期时间: {}", expire_date)
        } else {
            "过期时间: 无限制".to_string()
        };

        items.push(ListItem::new(usage_text).style(Style::default().fg(Color::Green)));
        items.push(ListItem::new(expire_text).style(Style::default().fg(Color::Green)));
        items.push(ListItem::new(""));
    }

    // Show health check info
    if let Some(ref hc) = provider.health_check {
        items.push(
            ListItem::new(format!(
                "健康检查: {} | URL: {} | 间隔: {}s",
                if hc.enable { "开启" } else { "关闭" },
                hc.url,
                hc.interval
            ))
            .style(Style::default().fg(Color::Blue)),
        );
        items.push(ListItem::new(""));
    }

    // Show proxies
    for proxy in &provider.proxies {
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
                Span::styled(format!(" {}ms", h.delay), Style::default().fg(color))
            })
            .unwrap_or_else(|| Span::raw(" -"));

        let line = Line::from(vec![
            Span::raw(format!("{} ({})", proxy.name, proxy.proxy_type)),
            delay,
        ]);
        items.push(ListItem::new(line));
    }

    let list = List::new(items).block(block);

    f.render_widget(Clear, popup_area);
    f.render_widget(list, popup_area);
}

fn render_provider_form(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 50, area);

    let title = if app.popup_mode == PopupMode::AddProvider {
        " 新增订阅 "
    } else {
        " 编辑订阅 "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);

    // Clear background
    f.render_widget(Clear, popup_area);
    f.render_widget(block, popup_area);

    // Form layout
    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Name
            Constraint::Min(5),    // URL - 最小5行，长URL可自动换行
            Constraint::Length(3), // Interval
            Constraint::Length(2), // Help text
        ])
        .split(inner);

    let labels = ["名称:", "订阅 URL:", "更新间隔(秒):"];

    for (i, (chunk, label)) in form_chunks.iter().zip(labels.iter()).enumerate() {
        let is_active = i == app.input_field_index;
        let border_style = if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let input_block = Block::default()
            .title(format!("{} {}", if is_active { "►" } else { " " }, label))
            .borders(Borders::ALL)
            .border_style(border_style);

        let value = &app.input_fields[i];
        let display_value = if is_active && app.input_mode == InputMode::Editing {
            let mut s = value.clone();
            let cursor_byte = char_to_byte_index(&s, app.input_cursor);
            s.insert(cursor_byte, '│');
            s
        } else {
            value.clone()
        };

        let input = Paragraph::new(display_value)
            .block(input_block)
            .wrap(Wrap { trim: false });

        f.render_widget(input, *chunk);
    }

    // Help text
    let help = Paragraph::new("Tab:切换字段  Enter:确认  Esc:取消")
        .style(Style::default().fg(Color::Gray));
    f.render_widget(help, form_chunks[3]);

    // Error message
    if let Some(ref err) = app.error_message {
        let error_area = centered_rect(60, 20, area);
        let error_widget = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Color::Red)
                    .title(" 错误 "),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(Clear, error_area);
        f.render_widget(error_widget, error_area);
    }
}

fn render_delete_confirm(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 30, area);

    let provider_name = app
        .delete_provider_name
        .clone()
        .or_else(|| {
            app.providers
                .get(app.selected_provider)
                .map(|p| p.name.clone())
        })
        .unwrap_or_else(|| "未知".to_string());

    let text = format!(
        "确定要删除订阅 '{}' 吗?\n\n按 Enter 确认删除，Esc 取消",
        provider_name
    );

    let block = Block::default()
        .title(" 确认删除 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(Color::Yellow))
        .wrap(Wrap { trim: false })
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);

    // Error message
    if let Some(ref err) = app.error_message {
        let error_area = centered_rect(60, 20, area);
        let error_widget = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Color::Red)
                    .title(" 错误 "),
            );
        f.render_widget(Clear, error_area);
        f.render_widget(error_widget, error_area);
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
