use crate::commands::Command;
use crate::context::AppContext;
use crate::ui::{Popup, Tab};

pub struct CommandExecutor;

impl CommandExecutor {
    pub fn execute(cmd: &Command, ctx: &mut AppContext) {
        match cmd {
            Command::Quit => ctx.running = false,
            Command::SwitchTab(tab) => switch_tab(ctx, *tab),
            Command::NextTab => next_tab(ctx),
            Command::PrevTab => prev_tab(ctx),
            Command::Refresh => {}
            Command::CycleMode => {}
            Command::ToggleAutoUpdate => {
                ctx.ui.auto_update = !ctx.ui.auto_update;
                let status = if ctx.ui.auto_update {
                    "开启"
                } else {
                    "关闭"
                };
                ctx.ui.success_message = Some(format!("自动更新已{}", status));
            }
            Command::ToggleSearch => {
                ctx.ui.search_active = true;
                ctx.ui.search_query.clear();
            }
            Command::CycleSort => cycle_sort(ctx),

            Command::MoveUp => move_up(ctx),
            Command::MoveDown => move_down(ctx),
            Command::PageUp => page_up(ctx),
            Command::PageDown => page_down(ctx),
            Command::Home => home(ctx),
            Command::End => end(ctx),

            Command::ProxySelect => {
                if let Some(proxy) = ctx.state.proxies.get(ctx.ui.proxy_selected) {
                    let all = proxy.all.clone().unwrap_or_default();
                    let selected = proxy
                        .now
                        .as_ref()
                        .and_then(|n| all.iter().position(|a| a == n))
                        .unwrap_or(0);
                    ctx.ui.popup = Popup::ProxyNodes {
                        proxy_name: proxy.name.clone(),
                        selected,
                    };
                }
            }
            Command::ProxyPrev => {}
            Command::ProxyNext => {}
            Command::ProxySpeedTest => {}
            Command::ProxySpeedTestAll => {}

            Command::ProviderToggleExpand => {
                let idx = ctx.ui.provider_selected;
                if let Some(expanded) = ctx.ui.provider_expanded.get_mut(idx) {
                    *expanded = !*expanded;
                }
            }
            Command::ProviderAdd => {
                ctx.ui.popup = Popup::AddProvider;
                ctx.ui.input_fields = vec![String::new(), String::new(), String::from("86400")];
                ctx.ui.input_field_index = 0;
                ctx.ui.input_cursor = 0;
                ctx.ui.error_message = None;
                ctx.ui.success_message = None;
            }
            Command::ProviderEdit => {
                if let Some(provider) = ctx.state.providers.get(ctx.ui.provider_selected) {
                    let name = provider.name.clone();
                    let mut fields = vec![name.clone(), String::new(), String::from("86400")];
                    if let Ok(configs) = ctx.config_manager.get_providers()
                        && let Some(cfg) = configs.get(&name)
                    {
                        fields[1] = cfg.url.clone();
                        if let Some(interval) = cfg.interval {
                            fields[2] = interval.to_string();
                        }
                    }
                    ctx.ui.popup = Popup::EditProvider;
                    ctx.ui.input_fields = fields;
                    ctx.ui.input_field_index = 1;
                    ctx.ui.input_cursor = 0;
                    ctx.ui.error_message = None;
                    ctx.ui.success_message = None;
                }
            }
            Command::ProviderDelete => {
                if ctx.state.providers.get(ctx.ui.provider_selected).is_some() {
                    ctx.ui.popup = Popup::DeleteConfirm;
                    ctx.ui.error_message = None;
                    ctx.ui.success_message = None;
                }
            }
            Command::ProviderUpdate => {}
            Command::ProviderUpdateAll => {}
            Command::ProviderHealthCheck => {}

            Command::ConnClose => {
                let idx = ctx.ui.connection_selected;
                if let Some(conn) = ctx.state.connections.get(idx) {
                    let client = ctx.client.clone();
                    let id = conn.id.clone();
                    tokio::spawn(async move {
                        let _ = client.close_connection(&id).await;
                    });
                }
            }
            Command::ConnCloseAll => {
                let client = ctx.client.clone();
                tokio::spawn(async move {
                    let _ = client.close_all_connections().await;
                });
            }

            Command::LogClear => {
                ctx.state.logs.clear();
                ctx.ui.log_scroll = 0;
            }

            Command::Confirm => {}
            Command::Cancel => {
                ctx.ui.popup = Popup::None;
                ctx.ui.input_fields = vec![String::new(), String::new(), String::from("86400")];
                ctx.ui.input_field_index = 0;
                ctx.ui.input_cursor = 0;
            }

            Command::FormNextField => {
                let len = ctx.ui.input_fields.len();
                ctx.ui.input_field_index = (ctx.ui.input_field_index + 1) % len;
                ctx.ui.input_cursor = ctx.ui.input_fields[ctx.ui.input_field_index]
                    .chars()
                    .count();
            }
            Command::FormPrevField => {
                let len = ctx.ui.input_fields.len();
                if ctx.ui.input_field_index == 0 {
                    ctx.ui.input_field_index = len - 1;
                } else {
                    ctx.ui.input_field_index -= 1;
                }
                ctx.ui.input_cursor = ctx.ui.input_fields[ctx.ui.input_field_index]
                    .chars()
                    .count();
            }
            Command::FormChar(c) => {
                let idx = ctx.ui.input_field_index;
                let field = &mut ctx.ui.input_fields[idx];
                let byte_idx = field
                    .char_indices()
                    .map(|(i, _)| i)
                    .nth(ctx.ui.input_cursor)
                    .unwrap_or(field.len());
                field.insert(byte_idx, *c);
                ctx.ui.input_cursor += 1;
            }
            Command::FormBackspace => {
                if ctx.ui.input_cursor > 0 {
                    let idx = ctx.ui.input_field_index;
                    let field = &mut ctx.ui.input_fields[idx];
                    let start = field
                        .char_indices()
                        .map(|(i, _)| i)
                        .nth(ctx.ui.input_cursor - 1)
                        .unwrap_or(0);
                    let end = field
                        .char_indices()
                        .map(|(i, _)| i)
                        .nth(ctx.ui.input_cursor)
                        .unwrap_or(field.len());
                    field.replace_range(start..end, "");
                    ctx.ui.input_cursor -= 1;
                }
            }
            Command::FormDelete => {
                let idx = ctx.ui.input_field_index;
                let field = &mut ctx.ui.input_fields[idx];
                let len = field.chars().count();
                if ctx.ui.input_cursor < len {
                    let start = field
                        .char_indices()
                        .map(|(i, _)| i)
                        .nth(ctx.ui.input_cursor)
                        .unwrap_or(field.len());
                    let end = field
                        .char_indices()
                        .map(|(i, _)| i)
                        .nth(ctx.ui.input_cursor + 1)
                        .unwrap_or(field.len());
                    field.replace_range(start..end, "");
                }
            }
            Command::FormLeft => {
                if ctx.ui.input_cursor > 0 {
                    ctx.ui.input_cursor -= 1;
                }
            }
            Command::FormRight => {
                let len = ctx.ui.input_fields[ctx.ui.input_field_index]
                    .chars()
                    .count();
                if ctx.ui.input_cursor < len {
                    ctx.ui.input_cursor += 1;
                }
            }
            Command::FormSubmit => {}

            Command::SearchChar(c) => {
                ctx.ui.search_query.push(*c);
            }
            Command::SearchBackspace => {
                ctx.ui.search_query.pop();
            }
            Command::SearchCancel => {
                ctx.ui.search_query.clear();
            }

            Command::Noop => {}
        }
    }
}

fn switch_tab(ctx: &mut AppContext, tab: Tab) {
    ctx.ui.current_tab = tab;
    ctx.ui.tab_index = tab as usize;
    ctx.ui.popup = Popup::None;
    ctx.ui.search_query.clear();
}

fn next_tab(ctx: &mut AppContext) {
    ctx.ui.tab_index = (ctx.ui.tab_index + 1) % 6;
    ctx.ui.current_tab = Tab::from_index(ctx.ui.tab_index);
    ctx.ui.popup = Popup::None;
    ctx.ui.search_query.clear();
}

fn prev_tab(ctx: &mut AppContext) {
    if ctx.ui.tab_index == 0 {
        ctx.ui.tab_index = 5;
    } else {
        ctx.ui.tab_index -= 1;
    }
    ctx.ui.current_tab = Tab::from_index(ctx.ui.tab_index);
    ctx.ui.popup = Popup::None;
    ctx.ui.search_query.clear();
}

fn move_up(ctx: &mut AppContext) {
    if let Popup::ProxyNodes {
        ref proxy_name,
        selected,
    } = ctx.ui.popup
    {
        if selected > 0 {
            ctx.ui.popup = Popup::ProxyNodes {
                proxy_name: proxy_name.clone(),
                selected: selected - 1,
            };
        }
        return;
    }
    match ctx.ui.current_tab {
        Tab::Proxies => {
            ctx.ui.proxy_selected = ctx.ui.proxy_selected.saturating_sub(1);
        }
        Tab::Providers => {
            ctx.ui.provider_selected = ctx.ui.provider_selected.saturating_sub(1);
        }
        Tab::Connections => {
            ctx.ui.connection_selected = ctx.ui.connection_selected.saturating_sub(1);
        }
        Tab::Rules => {
            ctx.ui.rule_selected = ctx.ui.rule_selected.saturating_sub(1);
        }
        Tab::Logs => ctx.ui.log_scroll = ctx.ui.log_scroll.saturating_sub(1),
        _ => {}
    }
}

#[cfg(test)]
#[expect(
    clippy::items_after_test_module,
    reason = "command executor tests stay close to the form-editing commands they exercise"
)]
mod tests {
    use super::*;
    use crate::client::ClashClient;
    use crate::config::ConfigManager;
    use crate::context::{AppContext, UiState};
    use crate::state::AppState;

    fn test_context() -> AppContext {
        AppContext {
            state: AppState::default(),
            ui: UiState::default(),
            client: ClashClient::new("http://127.0.0.1:9090".to_string()),
            config_manager: ConfigManager::new(None).unwrap(),
            running: true,
            connected: false,
        }
    }

    #[test]
    fn test_provider_add_sets_up_form() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        assert!(matches!(ctx.ui.popup, Popup::AddProvider));
        assert_eq!(ctx.ui.input_fields.len(), 3);
        assert_eq!(ctx.ui.input_fields[0], "");
        assert_eq!(ctx.ui.input_fields[1], "");
        assert_eq!(ctx.ui.input_fields[2], "86400");
        assert_eq!(ctx.ui.input_field_index, 0);
        assert_eq!(ctx.ui.input_cursor, 0);
    }

    #[test]
    fn test_form_char_inserts_at_cursor() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        CommandExecutor::execute(&Command::FormChar('h'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('e'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('l'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('l'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('o'), &mut ctx);

        assert_eq!(ctx.ui.input_fields[0], "hello");
        assert_eq!(ctx.ui.input_cursor, 5);
    }

    #[test]
    fn test_form_char_inserts_in_middle() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        CommandExecutor::execute(&Command::FormChar('a'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('c'), &mut ctx);
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        CommandExecutor::execute(&Command::FormChar('b'), &mut ctx);

        assert_eq!(ctx.ui.input_fields[0], "abc");
        assert_eq!(ctx.ui.input_cursor, 2);
    }

    #[test]
    fn test_form_backspace_deletes() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        CommandExecutor::execute(&Command::FormChar('a'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('b'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('c'), &mut ctx);
        CommandExecutor::execute(&Command::FormBackspace, &mut ctx);

        assert_eq!(ctx.ui.input_fields[0], "ab");
        assert_eq!(ctx.ui.input_cursor, 2);
    }

    #[test]
    fn test_form_backspace_at_start_does_nothing() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        CommandExecutor::execute(&Command::FormBackspace, &mut ctx);

        assert_eq!(ctx.ui.input_fields[0], "");
        assert_eq!(ctx.ui.input_cursor, 0);
    }

    #[test]
    fn test_form_next_prev_field_cycles() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        assert_eq!(ctx.ui.input_field_index, 0);

        CommandExecutor::execute(&Command::FormNextField, &mut ctx);
        assert_eq!(ctx.ui.input_field_index, 1);

        CommandExecutor::execute(&Command::FormNextField, &mut ctx);
        assert_eq!(ctx.ui.input_field_index, 2);

        CommandExecutor::execute(&Command::FormNextField, &mut ctx);
        assert_eq!(ctx.ui.input_field_index, 0);

        CommandExecutor::execute(&Command::FormPrevField, &mut ctx);
        assert_eq!(ctx.ui.input_field_index, 2);
    }

    #[test]
    fn test_form_cursor_left_right() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        CommandExecutor::execute(&Command::FormChar('a'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('b'), &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 2);

        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 1);

        CommandExecutor::execute(&Command::FormRight, &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 2);

        // Can't go past end
        CommandExecutor::execute(&Command::FormRight, &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 2);

        // Can't go before start
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 0);
    }

    #[test]
    fn test_form_multibyte_characters() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        // Type "中文"
        CommandExecutor::execute(&Command::FormChar('中'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('文'), &mut ctx);

        assert_eq!(ctx.ui.input_fields[0], "中文");
        assert_eq!(ctx.ui.input_cursor, 2);

        // Move left and insert in middle
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 1);

        CommandExecutor::execute(&Command::FormChar('A'), &mut ctx);
        assert_eq!(ctx.ui.input_fields[0], "中A文");
        assert_eq!(ctx.ui.input_cursor, 2);

        // Backspace should delete "A"
        CommandExecutor::execute(&Command::FormBackspace, &mut ctx);
        assert_eq!(ctx.ui.input_fields[0], "中文");
        assert_eq!(ctx.ui.input_cursor, 1);
    }

    #[test]
    fn test_form_delete_removes_forward() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);

        CommandExecutor::execute(&Command::FormChar('a'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('b'), &mut ctx);
        CommandExecutor::execute(&Command::FormChar('c'), &mut ctx);

        // Move cursor to beginning
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        CommandExecutor::execute(&Command::FormLeft, &mut ctx);
        assert_eq!(ctx.ui.input_cursor, 0);

        // Delete forward
        CommandExecutor::execute(&Command::FormDelete, &mut ctx);
        assert_eq!(ctx.ui.input_fields[0], "bc");
        assert_eq!(ctx.ui.input_cursor, 0);
    }

    #[test]
    fn test_cancel_resets_form() {
        let mut ctx = test_context();
        CommandExecutor::execute(&Command::ProviderAdd, &mut ctx);
        CommandExecutor::execute(&Command::FormChar('x'), &mut ctx);

        CommandExecutor::execute(&Command::Cancel, &mut ctx);

        assert!(matches!(ctx.ui.popup, Popup::None));
        assert_eq!(ctx.ui.input_fields[0], "");
        assert_eq!(ctx.ui.input_field_index, 0);
        assert_eq!(ctx.ui.input_cursor, 0);
    }
}

fn move_down(ctx: &mut AppContext) {
    if let Popup::ProxyNodes {
        ref proxy_name,
        selected,
    } = ctx.ui.popup
    {
        let max = ctx
            .state
            .all_proxies
            .get(proxy_name)
            .and_then(|p| p.all.as_ref())
            .map(|a| a.len().saturating_sub(1))
            .unwrap_or(0);
        if selected < max {
            ctx.ui.popup = Popup::ProxyNodes {
                proxy_name: proxy_name.clone(),
                selected: selected + 1,
            };
        }
        return;
    }
    match ctx.ui.current_tab {
        Tab::Proxies => {
            let max = ctx.state.proxies.len().saturating_sub(1);
            if ctx.ui.proxy_selected < max {
                ctx.ui.proxy_selected += 1;
            }
        }
        Tab::Providers => {
            let max = ctx.state.providers.len().saturating_sub(1);
            if ctx.ui.provider_selected < max {
                ctx.ui.provider_selected += 1;
            }
        }
        Tab::Connections => {
            let max = ctx.state.connections.len().saturating_sub(1);
            if ctx.ui.connection_selected < max {
                ctx.ui.connection_selected += 1;
            }
        }
        Tab::Rules => {
            let max = ctx.state.rules.len().saturating_sub(1);
            if ctx.ui.rule_selected < max {
                ctx.ui.rule_selected += 1;
            }
        }
        Tab::Logs => {
            let max = ctx.state.logs.len().saturating_sub(1);
            if ctx.ui.log_scroll < max {
                ctx.ui.log_scroll += 1;
            }
        }
        _ => {}
    }
}

fn page_up(ctx: &mut AppContext) {
    if ctx.ui.current_tab == Tab::Logs {
        ctx.ui.log_scroll = ctx.ui.log_scroll.saturating_sub(10);
    }
}

fn page_down(ctx: &mut AppContext) {
    if ctx.ui.current_tab == Tab::Logs {
        let max = ctx.state.logs.len().saturating_sub(1);
        ctx.ui.log_scroll = (ctx.ui.log_scroll + 10).min(max);
    }
}

fn home(ctx: &mut AppContext) {
    if let Popup::ProxyNodes { ref proxy_name, .. } = ctx.ui.popup {
        ctx.ui.popup = Popup::ProxyNodes {
            proxy_name: proxy_name.clone(),
            selected: 0,
        };
        return;
    }
    match ctx.ui.current_tab {
        Tab::Proxies => ctx.ui.proxy_selected = 0,
        Tab::Providers => ctx.ui.provider_selected = 0,
        Tab::Connections => ctx.ui.connection_selected = 0,
        Tab::Rules => ctx.ui.rule_selected = 0,
        Tab::Logs => ctx.ui.log_scroll = 0,
        _ => {}
    }
}

fn end(ctx: &mut AppContext) {
    if let Popup::ProxyNodes { ref proxy_name, .. } = ctx.ui.popup {
        let max = ctx
            .state
            .all_proxies
            .get(proxy_name)
            .and_then(|p| p.all.as_ref())
            .map(|a| a.len().saturating_sub(1))
            .unwrap_or(0);
        ctx.ui.popup = Popup::ProxyNodes {
            proxy_name: proxy_name.clone(),
            selected: max,
        };
        return;
    }
    match ctx.ui.current_tab {
        Tab::Proxies => ctx.ui.proxy_selected = ctx.state.proxies.len().saturating_sub(1),
        Tab::Providers => ctx.ui.provider_selected = ctx.state.providers.len().saturating_sub(1),
        Tab::Connections => {
            ctx.ui.connection_selected = ctx.state.connections.len().saturating_sub(1)
        }
        Tab::Rules => ctx.ui.rule_selected = ctx.state.rules.len().saturating_sub(1),
        Tab::Logs => ctx.ui.log_scroll = ctx.state.logs.len().saturating_sub(1),
        _ => {}
    }
}

fn cycle_sort(ctx: &mut AppContext) {
    match ctx.ui.current_tab {
        Tab::Proxies => {
            ctx.ui.proxy_sort_asc = !ctx.ui.proxy_sort_asc;
            let asc = ctx.ui.proxy_sort_asc;
            ctx.state.proxies.sort_by(|a, b| {
                let da = a.history.first().map(|h| h.delay).unwrap_or(u32::MAX);
                let db = b.history.first().map(|h| h.delay).unwrap_or(u32::MAX);
                if asc { da.cmp(&db) } else { db.cmp(&da) }
            });
            ctx.ui.success_message =
                Some(format!("代理按延迟排序 {}", if asc { "↑" } else { "↓" }));
        }
        Tab::Providers => {
            ctx.ui.provider_sort_asc = !ctx.ui.provider_sort_asc;
            let asc = ctx.ui.provider_sort_asc;
            ctx.state.providers.sort_by(|a, b| {
                if asc {
                    a.name.cmp(&b.name)
                } else {
                    b.name.cmp(&a.name)
                }
            });
            ctx.ui.success_message =
                Some(format!("订阅按名称排序 {}", if asc { "↑" } else { "↓" }));
        }
        Tab::Connections => {
            ctx.ui.connection_sort_asc = !ctx.ui.connection_sort_asc;
            let asc = ctx.ui.connection_sort_asc;
            ctx.state.connections.sort_by(|a, b| {
                if asc {
                    a.download.cmp(&b.download)
                } else {
                    b.download.cmp(&a.download)
                }
            });
            ctx.ui.success_message =
                Some(format!("连接按下载排序 {}", if asc { "↑" } else { "↓" }));
        }
        Tab::Rules => {
            ctx.ui.rule_sort_asc = !ctx.ui.rule_sort_asc;
            let asc = ctx.ui.rule_sort_asc;
            ctx.state.rules.sort_by(|a, b| {
                if asc {
                    a.rule_type.cmp(&b.rule_type)
                } else {
                    b.rule_type.cmp(&a.rule_type)
                }
            });
            ctx.ui.success_message =
                Some(format!("规则按类型排序 {}", if asc { "↑" } else { "↓" }));
        }
        _ => {}
    }
}
