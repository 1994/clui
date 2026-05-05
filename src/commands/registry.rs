use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

use crate::commands::Command;
use crate::ui::Tab;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppModeKind {
    Normal,
    Search,
    Form,
    Popup,
    ProxyNodes,
}

pub struct CommandRegistry {
    global: HashMap<KeyCode, Command>,
    by_tab: HashMap<Tab, HashMap<KeyCode, Command>>,
    popup: HashMap<KeyCode, Command>,
    proxy_nodes: HashMap<KeyCode, Command>,
    form: HashMap<KeyCode, Command>,
    search: HashMap<KeyCode, Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut global = HashMap::new();
        global.insert(KeyCode::Char('q'), Command::Quit);
        global.insert(KeyCode::Esc, Command::Quit);
        global.insert(KeyCode::Char('1'), Command::SwitchTab(Tab::Overview));
        global.insert(KeyCode::Char('2'), Command::SwitchTab(Tab::Proxies));
        global.insert(KeyCode::Char('3'), Command::SwitchTab(Tab::Providers));
        global.insert(KeyCode::Char('4'), Command::SwitchTab(Tab::Connections));
        global.insert(KeyCode::Char('5'), Command::SwitchTab(Tab::Rules));
        global.insert(KeyCode::Char('6'), Command::SwitchTab(Tab::Logs));
        global.insert(KeyCode::Char('r'), Command::Refresh);
        global.insert(KeyCode::Char('R'), Command::Refresh);
        global.insert(KeyCode::Char('m'), Command::CycleMode);
        global.insert(KeyCode::Char('A'), Command::ToggleAutoUpdate);
        global.insert(KeyCode::Tab, Command::NextTab);
        global.insert(KeyCode::BackTab, Command::PrevTab);
        global.insert(KeyCode::Char('/'), Command::ToggleSearch);
        global.insert(KeyCode::F(4), Command::CycleSort);
        global.insert(KeyCode::F(5), Command::Refresh);
        global.insert(KeyCode::F(6), Command::CycleMode);
        global.insert(KeyCode::F(10), Command::Quit);

        let mut by_tab: HashMap<Tab, HashMap<KeyCode, Command>> = HashMap::new();

        let mut proxies = HashMap::new();
        proxies.insert(KeyCode::Up, Command::MoveUp);
        proxies.insert(KeyCode::Down, Command::MoveDown);
        proxies.insert(KeyCode::Home, Command::Home);
        proxies.insert(KeyCode::End, Command::End);
        proxies.insert(KeyCode::Enter, Command::ProxySelect);
        proxies.insert(KeyCode::Char(' '), Command::ProxySelect);
        proxies.insert(KeyCode::Char('s'), Command::ProxySpeedTest);
        proxies.insert(KeyCode::Char('S'), Command::ProxySpeedTestAll);
        by_tab.insert(Tab::Proxies, proxies);

        let mut providers = HashMap::new();
        providers.insert(KeyCode::Up, Command::MoveUp);
        providers.insert(KeyCode::Down, Command::MoveDown);
        providers.insert(KeyCode::Home, Command::Home);
        providers.insert(KeyCode::End, Command::End);
        providers.insert(KeyCode::Enter, Command::ProviderToggleExpand);
        providers.insert(KeyCode::Char(' '), Command::ProviderToggleExpand);
        providers.insert(KeyCode::Char('a'), Command::ProviderAdd);
        providers.insert(KeyCode::Char('e'), Command::ProviderEdit);
        providers.insert(KeyCode::Char('d'), Command::ProviderDelete);
        providers.insert(KeyCode::Char('D'), Command::ProviderDelete);
        providers.insert(KeyCode::Char('u'), Command::ProviderUpdate);
        providers.insert(KeyCode::Char('U'), Command::ProviderUpdateAll);
        providers.insert(KeyCode::Char('h'), Command::ProviderHealthCheck);
        by_tab.insert(Tab::Providers, providers);

        let mut connections = HashMap::new();
        connections.insert(KeyCode::Up, Command::MoveUp);
        connections.insert(KeyCode::Down, Command::MoveDown);
        connections.insert(KeyCode::Home, Command::Home);
        connections.insert(KeyCode::End, Command::End);
        connections.insert(KeyCode::Char('c'), Command::ConnCloseAll);
        connections.insert(KeyCode::Char('C'), Command::ConnCloseAll);
        connections.insert(KeyCode::Char('d'), Command::ConnClose);
        connections.insert(KeyCode::Char('D'), Command::ConnClose);
        connections.insert(KeyCode::F(9), Command::ConnClose);
        by_tab.insert(Tab::Connections, connections);

        let mut rules = HashMap::new();
        rules.insert(KeyCode::Up, Command::MoveUp);
        rules.insert(KeyCode::Down, Command::MoveDown);
        rules.insert(KeyCode::Home, Command::Home);
        rules.insert(KeyCode::End, Command::End);
        by_tab.insert(Tab::Rules, rules);

        let mut logs = HashMap::new();
        logs.insert(KeyCode::Up, Command::MoveUp);
        logs.insert(KeyCode::Down, Command::MoveDown);
        logs.insert(KeyCode::Char('k'), Command::MoveUp);
        logs.insert(KeyCode::Char('j'), Command::MoveDown);
        logs.insert(KeyCode::PageUp, Command::PageUp);
        logs.insert(KeyCode::PageDown, Command::PageDown);
        logs.insert(KeyCode::Char('c'), Command::LogClear);
        logs.insert(KeyCode::Char('C'), Command::LogClear);
        by_tab.insert(Tab::Logs, logs);

        by_tab.insert(Tab::Overview, HashMap::new());

        let mut popup = HashMap::new();
        popup.insert(KeyCode::Enter, Command::Confirm);
        popup.insert(KeyCode::Esc, Command::Cancel);
        popup.insert(KeyCode::Char('q'), Command::Cancel);

        let mut proxy_nodes = HashMap::new();
        proxy_nodes.insert(KeyCode::Up, Command::MoveUp);
        proxy_nodes.insert(KeyCode::Down, Command::MoveDown);
        proxy_nodes.insert(KeyCode::Home, Command::Home);
        proxy_nodes.insert(KeyCode::End, Command::End);
        proxy_nodes.insert(KeyCode::Enter, Command::Confirm);
        proxy_nodes.insert(KeyCode::Esc, Command::Cancel);
        proxy_nodes.insert(KeyCode::Char(' '), Command::Confirm);
        proxy_nodes.insert(KeyCode::Char('q'), Command::Cancel);

        let mut form = HashMap::new();
        form.insert(KeyCode::Esc, Command::Cancel);
        form.insert(KeyCode::Tab, Command::FormNextField);
        form.insert(KeyCode::BackTab, Command::FormPrevField);
        form.insert(KeyCode::Enter, Command::FormSubmit);
        form.insert(KeyCode::Left, Command::FormLeft);
        form.insert(KeyCode::Right, Command::FormRight);
        form.insert(KeyCode::Backspace, Command::FormBackspace);
        form.insert(KeyCode::Delete, Command::FormDelete);

        let mut search = HashMap::new();
        search.insert(KeyCode::Esc, Command::SearchCancel);
        search.insert(KeyCode::Enter, Command::SearchCancel);
        search.insert(KeyCode::Backspace, Command::SearchBackspace);

        Self {
            global,
            by_tab,
            popup,
            proxy_nodes,
            form,
            search,
        }
    }

    pub fn resolve(&self, key: KeyEvent, tab: Tab, mode: AppModeKind) -> Vec<Command> {
        let mut cmds = vec![];

        // Ctrl+C always quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            cmds.push(Command::Quit);
            return cmds;
        }

        match mode {
            AppModeKind::Search => {
                if let Some(cmd) = self.search.get(&key.code).cloned() {
                    cmds.push(cmd);
                } else if let KeyCode::Char(c) = key.code {
                    cmds.push(Command::SearchChar(c));
                }
            }
            AppModeKind::Form => {
                if let Some(cmd) = self.form.get(&key.code).cloned() {
                    cmds.push(cmd);
                } else if let KeyCode::Char(c) = key.code {
                    cmds.push(Command::FormChar(c));
                }
            }
            AppModeKind::Popup => {
                if let Some(cmd) = self.popup.get(&key.code).cloned() {
                    cmds.push(cmd);
                }
            }
            AppModeKind::ProxyNodes => {
                if let Some(cmd) = self.proxy_nodes.get(&key.code).cloned() {
                    cmds.push(cmd);
                }
            }
            AppModeKind::Normal => {
                if let Some(cmd) = self.global.get(&key.code).cloned() {
                    cmds.push(cmd);
                } else if let Some(tab_cmds) = self.by_tab.get(&tab)
                    && let Some(cmd) = tab_cmds.get(&key.code).cloned()
                {
                    cmds.push(cmd);
                }
            }
        }

        cmds
    }
}
