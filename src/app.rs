use crate::clash::{
    ClashClient, Config, Connection, Memory, Provider, Proxy, Rule, Traffic, Version,
};
use crate::config::ConfigManager;
use crate::core_manager::{CoreManager, create_core_manager, get_default_config_path};
use crate::event::{Event, EventHandler};
use crate::scheduler::{Scheduler, SchedulerEvent, format_duration_detailed};
use crate::ui::{Tab, render};
use anyhow::Result;
use ratatui::{
    Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub(crate) fn clear_terminal_startup_artifacts<B: Backend>(
    terminal: &mut Terminal<B>,
) -> Result<()> {
    terminal.clear()?;
    Ok(())
}

pub(crate) fn char_count(value: &str) -> usize {
    value.chars().count()
}

pub(crate) fn char_to_byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .map(|(index, _)| index)
        .nth(char_index)
        .unwrap_or(value.len())
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PopupMode {
    None,
    AddProvider,
    EditProvider,
    DeleteConfirm,
}

#[derive(Debug, Clone)]
pub struct ProviderUpdateInfo {
    pub is_updating: bool,
    pub last_update: Option<Instant>,
    pub next_update: Instant,
    pub last_error: Option<String>,
}

pub struct App {
    pub current_tab: Tab,
    pub running: bool,
    pub clash: Option<ClashClient>,
    pub core_manager: Option<Box<dyn CoreManager>>,
    pub event_handler: EventHandler,
    pub config_manager: ConfigManager,
    pub scheduler: Option<Arc<Scheduler>>,
    pub scheduler_rx: Option<mpsc::UnboundedReceiver<SchedulerEvent>>,

    // Data
    pub version: Option<Version>,
    pub config: Option<Config>,
    pub memory: Option<Memory>,
    pub proxies: Vec<Proxy>,
    pub all_proxies: HashMap<String, Proxy>,
    pub providers: Vec<Provider>,
    pub provider_update_info: HashMap<String, ProviderUpdateInfo>,
    pub missing_config_providers: HashSet<String>,
    pub last_provider_error: Option<String>,
    pub connections: Vec<Connection>,
    pub download_total: u64,
    pub upload_total: u64,
    pub rules: Vec<Rule>,
    pub traffic: Traffic,
    pub traffic_history: VecDeque<(u64, u64)>,
    pub logs: VecDeque<String>,

    // UI State
    pub _proxy_scroll: usize,
    pub _provider_scroll: usize,
    pub connection_scroll: usize,
    pub rule_scroll: usize,
    pub log_scroll: usize,
    pub selected_proxy_group: usize,
    pub selected_provider: usize,
    pub _selected_connection: usize,
    pub proxy_expanded: Vec<bool>,
    pub provider_expanded: Vec<bool>,

    // Selection states
    pub selected_tab_index: usize,

    // Modes
    pub modes: Vec<String>,
    pub current_mode_index: usize,

    // Popup and Input state
    pub popup_mode: PopupMode,
    pub input_mode: InputMode,
    pub input_fields: Vec<String>,
    pub input_field_index: usize,
    pub input_cursor: usize,
    pub delete_provider_name: Option<String>,
    pub error_message: Option<String>,
    pub success_message: Option<String>,

    // Auto update enabled
    pub auto_update_enabled: bool,

    // Core ready
    pub core_ready: bool,

    // Last update time
    pub last_update: Instant,
}

impl App {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::with_config(get_default_config_path())
    }

    pub fn with_config(config_path: PathBuf) -> Self {
        let event_handler = EventHandler::new(Duration::from_millis(100));
        let config_manager =
            ConfigManager::new(Some(config_path.clone())).expect("Failed to create config manager");

        // Available modes in Clash Meta
        let modes = vec![
            "Global".to_string(),
            "Rule".to_string(),
            "Direct".to_string(),
            "Reject".to_string(),
        ];

        Self {
            current_tab: Tab::Overview,
            running: true,
            clash: None,
            core_manager: None,
            event_handler,
            config_manager,
            scheduler: None,
            scheduler_rx: None,
            version: None,
            config: None,
            memory: None,
            proxies: Vec::new(),
            all_proxies: HashMap::new(),
            providers: Vec::new(),
            provider_update_info: HashMap::new(),
            missing_config_providers: HashSet::new(),
            last_provider_error: None,
            connections: Vec::new(),
            download_total: 0,
            upload_total: 0,
            rules: Vec::new(),
            traffic: Traffic::default(),
            traffic_history: VecDeque::with_capacity(60),
            logs: VecDeque::with_capacity(1000),
            _proxy_scroll: 0,
            _provider_scroll: 0,
            connection_scroll: 0,
            rule_scroll: 0,
            log_scroll: 0,
            selected_proxy_group: 0,
            selected_provider: 0,
            _selected_connection: 0,
            proxy_expanded: Vec::new(),
            provider_expanded: Vec::new(),
            selected_tab_index: 0,
            modes,
            current_mode_index: 1,
            popup_mode: PopupMode::None,
            input_mode: InputMode::Normal,
            input_fields: vec![String::new(), String::new(), String::from("86400")],
            input_field_index: 0,
            input_cursor: 0,
            delete_provider_name: None,
            error_message: None,
            success_message: None,
            auto_update_enabled: true,
            core_ready: false,
            last_update: Instant::now(),
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // 1. 尝试检测已有核心
        let config_path = self.config_manager.get_config_path().clone();
        let mut core_manager = create_core_manager(config_path.clone());

        if core_manager.detect_existing().await? {
            log::info!("使用已存在的核心");
            self.clash = Some(ClashClient::new(core_manager.api_url()));
            self.core_manager = Some(core_manager);
        } else {
            // 2. 启动内嵌核心
            log::info!("启动内嵌核心...");
            match core_manager.start().await {
                Ok(_) => {
                    self.clash = Some(ClashClient::new(core_manager.api_url()));
                    self.core_manager = Some(core_manager);

                    // 核心已启动，设置就绪状态
                    self.core_ready = true;
                }
                Err(e) => {
                    self.error_message = Some(format!("启动核心失败: {}", e));
                    return Err(e);
                }
            }
        }

        // 3. 初始化调度器
        if let Some(ref clash) = self.clash {
            let (scheduler_tx, scheduler_rx) = mpsc::unbounded_channel();
            let scheduler = Arc::new(Scheduler::new(
                clash.clone(),
                self.config_manager.clone(),
                scheduler_tx,
            ));

            scheduler.load_tasks().await?;

            // 启动调度器
            let scheduler_clone = scheduler.clone();
            tokio::spawn(async move {
                scheduler_clone.run().await;
            });

            self.scheduler = Some(scheduler);
            self.scheduler_rx = Some(scheduler_rx);
        }

        self.core_ready = true;
        Ok(())
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // 初始化
        if let Err(e) = self.initialize().await {
            log::error!("初始化失败: {}", e);
            // 继续运行，显示错误状态
        }

        // 初始化过程中核心可能向终端输出日志，首帧前强制清屏避免残留
        clear_terminal_startup_artifacts(terminal)?;

        self.last_update = Instant::now();

        while self.running {
            terminal.draw(|f| render(f, self))?;

            // 检查调度器事件
            if let Some(ref mut rx) = self.scheduler_rx
                && let Ok(event) = rx.try_recv()
            {
                self.handle_scheduler_event(event).await;
            }

            // 检查用户输入
            if let Some(event) = self.event_handler.next().await {
                match event {
                    Event::Tick => {
                        if self.last_update.elapsed() > Duration::from_secs(1) {
                            if self.core_ready {
                                let _ = self.refresh_data().await;
                                self.update_provider_info().await;
                            }
                            self.last_update = Instant::now();
                        }
                    }
                    Event::Key(key) => self.handle_key_event(key).await?,
                    Event::Mouse(_) => {}
                    Event::Resize(_, _) => {}
                }
            }
        }

        // 清理
        if let Some(ref mut cm) = self.core_manager {
            let _ = cm.stop().await;
        }

        Ok(())
    }

    async fn update_provider_info(&mut self) {
        if let Some(ref scheduler) = self.scheduler {
            let tasks = scheduler.get_tasks().await;
            for (name, task) in tasks {
                let info = ProviderUpdateInfo {
                    is_updating: task.is_updating,
                    last_update: task.last_update,
                    next_update: task.next_update,
                    last_error: task.last_error,
                };
                self.provider_update_info.insert(name, info);
            }
        }
    }

    fn add_log(&mut self, message: String) {
        const MAX_LOGS: usize = 500;

        self.logs.push_back(message);

        // 限制日志数量，防止内存无限增长
        if self.logs.len() > MAX_LOGS {
            let to_remove = self.logs.len() - MAX_LOGS;
            for _ in 0..to_remove {
                self.logs.pop_front();
            }
            // 调整 scroll 位置，防止跳动
            self.log_scroll = self.log_scroll.saturating_sub(to_remove);
        }
    }

    async fn handle_scheduler_event(&mut self, event: SchedulerEvent) {
        match event {
            SchedulerEvent::Started(name) => {
                self.add_log(format!(
                    "[{}] INFO: 开始更新订阅 '{}'...",
                    chrono::Local::now().format("%H:%M:%S"),
                    name
                ));
                if let Some(info) = self.provider_update_info.get_mut(&name) {
                    info.is_updating = true;
                }
            }
            SchedulerEvent::Completed(name, success, error) => {
                if success {
                    self.add_log(format!(
                        "[{}] INFO: 订阅 '{}' 更新成功",
                        chrono::Local::now().format("%H:%M:%S"),
                        name
                    ));
                    self.success_message = Some(format!("订阅 '{}' 自动更新成功", name));
                } else {
                    let err_msg = error.unwrap_or_else(|| "未知错误".to_string());
                    self.add_log(format!(
                        "[{}] ERROR: 订阅 '{}' 更新失败: {}",
                        chrono::Local::now().format("%H:%M:%S"),
                        name,
                        err_msg
                    ));
                    self.error_message = Some(format!(
                        "订阅 '{}' 自动更新失败: {}\n按 6 切到日志页可查看完整上下文。",
                        name, err_msg
                    ));
                }
                // 更新成功后刷新 providers 数据
                let _ = self.refresh_data().await;
                self.update_provider_info().await;
            }
            SchedulerEvent::Progress(name, msg) => {
                self.add_log(format!(
                    "[{}] INFO: 订阅 '{}' - {}",
                    chrono::Local::now().format("%H:%M:%S"),
                    name,
                    msg
                ));
            }
        }
    }

    async fn refresh_data(&mut self) -> Result<()> {
        let clash = match self.clash.clone() {
            Some(c) => c,
            None => return Ok(()),
        };

        // Fetch version on first run
        if self.version.is_none()
            && let Ok(v) = clash.get_version().await
        {
            self.version = Some(v);
        }

        // Fetch config
        if self.config.is_none()
            && let Ok(config) = clash.get_config().await
        {
            self.current_mode_index = self
                .modes
                .iter()
                .position(|m| m.to_lowercase() == config.mode.to_lowercase())
                .unwrap_or(1);
            self.config = Some(config);
        }

        // Fetch memory
        if let Ok(m) = clash.get_memory().await {
            self.memory = Some(m);
        }

        // Fetch proxies
        if let Ok(proxies) = clash.get_proxies().await {
            self.proxies = proxies;
            if self.proxy_expanded.len() != self.proxies.len() {
                self.proxy_expanded = vec![false; self.proxies.len()];
            }
        }

        // Fetch all proxies
        if let Ok(all) = clash.get_all_proxies().await {
            self.all_proxies = all;
        }

        // Fetch providers
        match clash.get_providers().await {
            Ok(providers) => {
                self.last_provider_error = None;
                self.providers = providers;
                let loaded_provider_names: HashSet<String> = self
                    .providers
                    .iter()
                    .map(|provider| provider.name.clone())
                    .collect();
                self.update_missing_config_providers(&loaded_provider_names);
            }
            Err(e) => {
                self.note_provider_error(format!("获取订阅列表失败: {}", e));
            }
        }

        // Fetch connections
        if let Ok(conn_resp) = clash.get_connections().await {
            self.connections = conn_resp.connections;
            self.download_total = conn_resp.download_total;
            self.upload_total = conn_resp.upload_total;
        }

        // Fetch rules
        if let Ok(rules) = clash.get_rules().await {
            self.rules = rules;
        }

        // Fetch traffic
        if let Ok(traffic) = clash.get_traffic().await {
            self.traffic = traffic.clone();
            self.traffic_history.push_back((traffic.up, traffic.down));
            if self.traffic_history.len() > 60 {
                self.traffic_history.pop_front();
            }
        }

        Ok(())
    }

    async fn reload_config_and_refresh(&mut self) -> Result<()> {
        if let Some(ref clash) = self.clash {
            clash.reload_config(true).await?;
        }

        self.refresh_data().await?;
        self.update_provider_info().await;

        if self.providers.is_empty() {
            self.selected_provider = 0;
        } else {
            self.selected_provider = self.selected_provider.min(self.providers.len() - 1);
        }

        Ok(())
    }

    fn resolve_provider_cache_path(&self, path: &str) -> PathBuf {
        let provider_path = PathBuf::from(path);
        if provider_path.is_absolute() {
            provider_path
        } else {
            self.config_manager
                .get_config_path()
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(provider_path)
        }
    }

    fn note_provider_error(&mut self, message: String) {
        if self.last_provider_error.as_deref() != Some(message.as_str()) {
            self.add_log(format!(
                "[{}] ERROR: {}",
                chrono::Local::now().format("%H:%M:%S"),
                message
            ));
            self.last_provider_error = Some(message);
        }
    }

    fn update_missing_config_providers(&mut self, loaded_provider_names: &HashSet<String>) {
        let configured = match self.config_manager.get_providers() {
            Ok(providers) => providers,
            Err(e) => {
                self.note_provider_error(format!("读取配置中的订阅失败: {}", e));
                return;
            }
        };

        let missing: HashSet<String> = configured
            .keys()
            .filter(|name| !loaded_provider_names.contains(*name))
            .cloned()
            .collect();

        if !missing.is_empty() {
            let mut missing_names: Vec<String> = missing.iter().cloned().collect();
            missing_names.sort_unstable();

            for name in &missing_names {
                if self.providers.iter().any(|provider| provider.name == *name) {
                    continue;
                }
                if let Some(cfg) = configured.get(name) {
                    self.providers.push(Provider {
                        name: name.clone(),
                        provider_type: cfg.provider_type.clone(),
                        vehicle_type: "pending".to_string(),
                        proxies: Vec::new(),
                        updated_at: None,
                        subscription_info: None,
                        path: cfg.path.clone(),
                        health_check: None,
                    });
                }
            }

            self.providers.sort_by(|a, b| a.name.cmp(&b.name));
        }

        if self.provider_expanded.len() != self.providers.len() {
            self.provider_expanded = vec![false; self.providers.len()];
        }

        if missing != self.missing_config_providers {
            if missing.is_empty() {
                if !self.missing_config_providers.is_empty() {
                    self.add_log(format!(
                        "[{}] INFO: 订阅加载已恢复正常",
                        chrono::Local::now().format("%H:%M:%S")
                    ));
                }
            } else {
                let mut names: Vec<String> = missing.iter().cloned().collect();
                names.sort_unstable();
                let joined = names.join(", ");
                self.add_log(format!(
                    "[{}] WARN: 以下订阅尚未加载到核心（已显示在列表，可删除）: {}",
                    chrono::Local::now().format("%H:%M:%S"),
                    joined
                ));
            }
        }

        self.missing_config_providers = missing;
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::{KeyCode, KeyModifiers};

        if self.input_mode == InputMode::Editing {
            self.handle_input_key(key).await;
            return Ok(());
        }

        // 处理 Ctrl+C 退出
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.running = false;
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.popup_mode != PopupMode::None {
                    self.close_popup();
                } else {
                    self.running = false;
                }
            }
            KeyCode::Char('1') => self.switch_tab(Tab::Overview),
            KeyCode::Char('2') => self.switch_tab(Tab::Proxies),
            KeyCode::Char('3') => self.switch_tab(Tab::Providers),
            KeyCode::Char('4') => self.switch_tab(Tab::Connections),
            KeyCode::Char('5') => self.switch_tab(Tab::Rules),
            KeyCode::Char('6') => self.switch_tab(Tab::Logs),
            KeyCode::Char('r') => match self.reload_config_and_refresh().await {
                Ok(_) => {
                    self.success_message = Some("配置重载成功".to_string());
                    self.add_log(format!(
                        "[{}] INFO: 手动重载配置成功",
                        chrono::Local::now().format("%H:%M:%S")
                    ));
                }
                Err(e) => {
                    self.error_message = Some(format!("配置重载失败: {}", e));
                    self.add_log(format!(
                        "[{}] ERROR: 手动重载配置失败: {}",
                        chrono::Local::now().format("%H:%M:%S"),
                        e
                    ));
                }
            },
            KeyCode::Char('m') => {
                self.current_mode_index = (self.current_mode_index + 1) % self.modes.len();
                let mode = self.modes[self.current_mode_index].clone();
                if let Some(ref mut config) = self.config {
                    config.mode = mode.to_lowercase();
                }
            }
            KeyCode::Char('A') => {
                self.auto_update_enabled = !self.auto_update_enabled;
                let status = if self.auto_update_enabled {
                    "开启"
                } else {
                    "关闭"
                };
                self.success_message = Some(format!("自动更新已{}", status));
            }
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            _ => {
                if !self.core_ready {
                    return Ok(());
                }
                match self.current_tab {
                    Tab::Proxies => {
                        self.handle_proxy_keys(key).await;
                    }
                    Tab::Providers => {
                        self.handle_provider_keys(key).await;
                    }
                    Tab::Connections => self.handle_connection_keys(key).await,
                    Tab::Rules => self.handle_rule_keys(key),
                    Tab::Logs => self.handle_log_keys(key),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn handle_input_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => self.close_popup(),
            KeyCode::Tab => {
                self.input_field_index = (self.input_field_index + 1) % self.input_fields.len();
                self.input_cursor = char_count(&self.input_fields[self.input_field_index]);
            }
            KeyCode::BackTab => {
                if self.input_field_index == 0 {
                    self.input_field_index = self.input_fields.len() - 1;
                } else {
                    self.input_field_index -= 1;
                }
                self.input_cursor = char_count(&self.input_fields[self.input_field_index]);
            }
            KeyCode::Enter => {
                if self.popup_mode == PopupMode::DeleteConfirm {
                    self.confirm_delete_provider().await;
                } else {
                    self.submit_form().await;
                }
            }
            KeyCode::Char(c) => {
                let idx = self.input_field_index;
                let field = &mut self.input_fields[idx];
                let cursor_byte = char_to_byte_index(field, self.input_cursor);
                field.insert(cursor_byte, c);
                self.input_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    let idx = self.input_field_index;
                    let field = &mut self.input_fields[idx];
                    let start = char_to_byte_index(field, self.input_cursor - 1);
                    let end = char_to_byte_index(field, self.input_cursor);
                    field.replace_range(start..end, "");
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < char_count(&self.input_fields[self.input_field_index]) {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Delete => {
                let idx = self.input_field_index;
                let field = &mut self.input_fields[idx];
                if self.input_cursor < char_count(field) {
                    let start = char_to_byte_index(field, self.input_cursor);
                    let end = char_to_byte_index(field, self.input_cursor + 1);
                    field.replace_range(start..end, "");
                }
            }
            _ => {}
        }
    }

    async fn submit_form(&mut self) {
        if self.clash.is_none() {
            return;
        }

        match self.popup_mode {
            PopupMode::AddProvider => {
                let name = self.input_fields[0].trim().to_string();
                let url = self.input_fields[1].trim().to_string();
                let interval: u64 = self.input_fields[2].parse().unwrap_or(86400);

                if name.is_empty() {
                    self.error_message = Some("订阅名称不能为空".to_string());
                    return;
                }
                if url.is_empty() || !url.starts_with("http") {
                    self.error_message = Some("请输入有效的 URL".to_string());
                    return;
                }

                match self
                    .config_manager
                    .add_provider(&name, &url, interval as u32, true)
                {
                    Ok(_) => {
                        if let Some(ref scheduler) = self.scheduler {
                            scheduler
                                .add_task(name.clone(), url.clone(), interval)
                                .await;
                        }

                        if let Err(e) = self.reload_config_and_refresh().await {
                            self.add_log(format!(
                                "[{}] WARN: 订阅 '{}' 已写入配置，但重载失败: {}",
                                chrono::Local::now().format("%H:%M:%S"),
                                name,
                                e
                            ));
                            self.error_message = Some(format!("订阅已添加，但重载失败: {}", e));
                        }

                        if let Some(index) = self
                            .providers
                            .iter()
                            .position(|provider| provider.name == name)
                        {
                            self.selected_provider = index;
                            self.add_log(format!(
                                "[{}] INFO: 订阅 '{}' 添加成功",
                                chrono::Local::now().format("%H:%M:%S"),
                                name
                            ));
                            self.success_message = Some(format!("订阅 '{}' 添加成功！", name));
                        } else {
                            self.add_log(format!(
                                "[{}] WARN: 订阅 '{}' 已添加到配置，但核心未加载",
                                chrono::Local::now().format("%H:%M:%S"),
                                name
                            ));
                            self.error_message = Some(format!(
                                "订阅 '{}' 已添加到配置，但核心未加载。请检查 URL 或按 r 查看重载错误",
                                name
                            ));
                        }

                        self.close_popup();
                    }
                    Err(e) => {
                        self.error_message = Some(format!("添加失败: {}", e));
                    }
                }
            }
            PopupMode::EditProvider => {
                if let Some(provider) = self.providers.get(self.selected_provider) {
                    let name = provider.name.clone();
                    let url = self.input_fields[1].trim();
                    let interval: u64 = self.input_fields[2].parse().unwrap_or(86400);

                    if url.is_empty() || !url.starts_with("http") {
                        self.error_message = Some("请输入有效的 URL".to_string());
                        return;
                    }

                    match self.config_manager.update_provider(
                        &name,
                        Some(url),
                        Some(interval as u32),
                    ) {
                        Ok(_) => {
                            if let Some(ref scheduler) = self.scheduler {
                                scheduler.update_task_interval(&name, interval).await;
                            }

                            if let Err(e) = self.reload_config_and_refresh().await {
                                self.add_log(format!(
                                    "[{}] WARN: 订阅 '{}' 更新后重载失败: {}",
                                    chrono::Local::now().format("%H:%M:%S"),
                                    name,
                                    e
                                ));
                                self.error_message = Some(format!("订阅已更新，但重载失败: {}", e));
                            }

                            if self.providers.iter().any(|provider| provider.name == name) {
                                self.success_message = Some(format!("订阅 '{}' 更新成功！", name));
                            } else {
                                self.error_message = Some(format!(
                                    "订阅 '{}' 更新后核心未加载，请按 r 查看重载错误",
                                    name
                                ));
                            }
                            self.close_popup();
                        }
                        Err(e) => {
                            self.error_message = Some(format!("更新失败: {}", e));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn close_popup(&mut self) {
        self.popup_mode = PopupMode::None;
        self.input_mode = InputMode::Normal;
        self.input_fields = vec![String::new(), String::new(), String::from("86400")];
        self.input_field_index = 0;
        self.input_cursor = 0;
        self.delete_provider_name = None;
    }

    pub fn open_add_provider(&mut self) {
        self.popup_mode = PopupMode::AddProvider;
        self.input_mode = InputMode::Editing;
        self.input_fields = vec![String::new(), String::new(), String::from("86400")];
        self.input_field_index = 0;
        self.input_cursor = 0;
        self.error_message = None;
        self.success_message = None;
    }

    pub fn open_edit_provider(&mut self) {
        if let Some(provider) = self.providers.get(self.selected_provider) {
            self.popup_mode = PopupMode::EditProvider;
            self.input_mode = InputMode::Editing;
            self.input_fields = vec![provider.name.clone(), String::new(), String::from("86400")];
            self.input_field_index = 1;
            self.input_cursor = 0;
            self.error_message = None;
            self.success_message = None;

            if let Ok(configs) = self.config_manager.get_providers()
                && let Some(cfg) = configs.get(&provider.name)
            {
                self.input_fields[1] = cfg.url.clone();
                if let Some(interval) = cfg.interval {
                    self.input_fields[2] = interval.to_string();
                }
            }
        }
    }

    pub fn open_delete_confirm(&mut self) {
        if let Some(provider) = self.providers.get(self.selected_provider) {
            self.popup_mode = PopupMode::DeleteConfirm;
            self.input_mode = InputMode::Editing;
            self.delete_provider_name = Some(provider.name.clone());
            self.error_message = None;
            self.success_message = None;
        } else {
            self.error_message = Some("当前没有可删除的订阅".to_string());
        }
    }

    pub async fn confirm_delete_provider(&mut self) {
        let Some(name) = self.delete_provider_name.clone().or_else(|| {
            self.providers
                .get(self.selected_provider)
                .map(|p| p.name.clone())
        }) else {
            self.error_message = Some("删除失败: 未找到目标订阅".to_string());
            return;
        };

        let provider_cache_path = self
            .config_manager
            .get_providers()
            .ok()
            .and_then(|configs| configs.get(&name).and_then(|cfg| cfg.path.clone()))
            .or_else(|| {
                self.providers
                    .iter()
                    .find(|provider| provider.name == name)
                    .and_then(|provider| provider.path.clone())
            })
            .map(|path| self.resolve_provider_cache_path(&path));

        match self.config_manager.remove_provider(&name) {
            Ok(_) => {
                if let Some(path) = provider_cache_path {
                    match tokio::fs::remove_file(&path).await {
                        Ok(_) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => {
                            self.add_log(format!(
                                "[{}] WARN: 删除订阅 '{}' 缓存文件失败 {:?}: {}",
                                chrono::Local::now().format("%H:%M:%S"),
                                name,
                                path,
                                e
                            ));
                        }
                    }
                }

                if let Some(ref scheduler) = self.scheduler {
                    scheduler.remove_task(&name).await;
                }

                self.providers.retain(|provider| provider.name != name);
                self.provider_expanded = vec![false; self.providers.len()];
                self.provider_update_info.remove(&name);
                self.missing_config_providers.remove(&name);
                if self.providers.is_empty() {
                    self.selected_provider = 0;
                } else {
                    self.selected_provider = self.selected_provider.min(self.providers.len() - 1);
                }

                let reload_result = self.reload_config_and_refresh().await;
                let still_exists = self.providers.iter().any(|provider| provider.name == name);

                if let Err(e) = reload_result {
                    self.add_log(format!(
                        "[{}] WARN: 订阅 '{}' 删除后重载失败: {}",
                        chrono::Local::now().format("%H:%M:%S"),
                        name,
                        e
                    ));
                    self.error_message = Some(format!("订阅已从配置删除，但重载失败: {}", e));
                } else if still_exists {
                    self.add_log(format!(
                        "[{}] WARN: 订阅 '{}' 删除后仍出现在核心列表中",
                        chrono::Local::now().format("%H:%M:%S"),
                        name
                    ));
                    self.error_message = Some(format!(
                        "订阅 '{}' 删除后核心仍有缓存，请按 r 重载并查看日志",
                        name
                    ));
                } else {
                    self.success_message = Some(format!("订阅 '{}' 已删除！", name));
                }

                self.close_popup();
            }
            Err(e) => {
                self.error_message = Some(format!("删除失败: {}", e));
            }
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        self.current_tab = tab;
        self.selected_tab_index = tab as usize;
        self.close_popup();
    }

    fn next_tab(&mut self) {
        self.selected_tab_index = (self.selected_tab_index + 1) % 6;
        self.current_tab = match self.selected_tab_index {
            0 => Tab::Overview,
            1 => Tab::Proxies,
            2 => Tab::Providers,
            3 => Tab::Connections,
            4 => Tab::Rules,
            _ => Tab::Logs,
        };
        self.close_popup();
    }

    fn prev_tab(&mut self) {
        if self.selected_tab_index == 0 {
            self.selected_tab_index = 5;
        } else {
            self.selected_tab_index -= 1;
        }
        self.current_tab = match self.selected_tab_index {
            0 => Tab::Overview,
            1 => Tab::Proxies,
            2 => Tab::Providers,
            3 => Tab::Connections,
            4 => Tab::Rules,
            _ => Tab::Logs,
        };
        self.close_popup();
    }

    async fn handle_proxy_keys(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let clash = match self.clash {
            Some(ref c) => c,
            None => return,
        };

        match key.code {
            KeyCode::Up => {
                if self.selected_proxy_group > 0 {
                    self.selected_proxy_group -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_proxy_group < self.proxies.len().saturating_sub(1) {
                    self.selected_proxy_group += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(expanded) = self.proxy_expanded.get_mut(self.selected_proxy_group) {
                    *expanded = !*expanded;
                }
            }
            KeyCode::Char('s') => {
                if let Some(proxy) = self.proxies.get(self.selected_proxy_group) {
                    let _ = clash.test_proxy_delay(&proxy.name, None, None).await;
                }
            }
            KeyCode::Char('S') => {
                for proxy in &self.proxies {
                    let _ = clash.test_proxy_delay(&proxy.name, None, None).await;
                }
            }
            KeyCode::Left | KeyCode::Right => {
                if let Some(proxy) = self.proxies.get(self.selected_proxy_group)
                    && let Some(ref all) = proxy.all
                    && let Some(current_idx) =
                        all.iter().position(|p| Some(p) == proxy.now.as_ref())
                {
                    let new_idx = if key.code == KeyCode::Left {
                        current_idx.saturating_sub(1)
                    } else {
                        (current_idx + 1).min(all.len() - 1)
                    };
                    if new_idx != current_idx {
                        let _ = clash.switch_proxy(&proxy.name, &all[new_idx]).await;
                    }
                }
            }
            _ => {}
        }
    }

    async fn handle_provider_keys(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let clash = match self.clash {
            Some(ref c) => c,
            None => return,
        };

        match key.code {
            KeyCode::Up => {
                if self.selected_provider > 0 {
                    self.selected_provider -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_provider < self.providers.len().saturating_sub(1) {
                    self.selected_provider += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(expanded) = self.provider_expanded.get_mut(self.selected_provider) {
                    *expanded = !*expanded;
                }
            }
            KeyCode::Char('u') => {
                if let Some(provider) = self.providers.get(self.selected_provider) {
                    let name = provider.name.clone();
                    if let Some(ref scheduler) = self.scheduler {
                        let scheduler = Arc::clone(scheduler);
                        let provider_name = name.clone();
                        std::mem::drop(tokio::spawn(async move {
                            let _ = scheduler.force_update(&provider_name).await;
                        }));
                        self.add_log(format!(
                            "[{}] INFO: 已触发订阅 '{}' 更新",
                            chrono::Local::now().format("%H:%M:%S"),
                            name
                        ));
                    }
                }
            }
            KeyCode::Char('h') => {
                if let Some(provider) = self.providers.get(self.selected_provider) {
                    let name = provider.name.clone();
                    let clash_client = clash.clone();
                    let provider_name = name.clone();
                    std::mem::drop(tokio::spawn(async move {
                        let _ = clash_client.health_check_provider(&provider_name).await;
                    }));
                    self.add_log(format!(
                        "[{}] INFO: 已触发订阅 '{}' 健康检查",
                        chrono::Local::now().format("%H:%M:%S"),
                        name
                    ));
                }
            }
            KeyCode::Char('a') => self.open_add_provider(),
            KeyCode::Char('e') => self.open_edit_provider(),
            KeyCode::Char('d') => self.open_delete_confirm(),
            KeyCode::Char('D') => self.open_delete_confirm(),
            KeyCode::Char('U') => {
                if let Some(ref scheduler) = self.scheduler {
                    let scheduler = Arc::clone(scheduler);
                    std::mem::drop(tokio::spawn(async move {
                        scheduler.update_all().await;
                    }));
                    self.add_log(format!(
                        "[{}] INFO: 已触发所有订阅更新",
                        chrono::Local::now().format("%H:%M:%S")
                    ));
                }
            }
            _ => {}
        }
    }

    async fn handle_connection_keys(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let clash = match self.clash {
            Some(ref c) => c,
            None => return,
        };

        match key.code {
            KeyCode::Up => {
                self.connection_scroll = self.connection_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let max = self.connections.len().saturating_sub(1);
                if self.connection_scroll < max {
                    self.connection_scroll += 1;
                }
            }
            KeyCode::Char('c') => {
                let _ = clash.close_all_connections().await;
            }
            KeyCode::Char('d') => {
                if let Some(conn) = self.connections.get(self.connection_scroll) {
                    let _ = clash.close_connection(&conn.id).await;
                }
            }
            _ => {}
        }
    }

    fn handle_rule_keys(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up => {
                self.rule_scroll = self.rule_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let max = self.rules.len().saturating_sub(1);
                if self.rule_scroll < max {
                    self.rule_scroll += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_log_keys(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let max = self.logs.len().saturating_sub(1);
                if self.log_scroll < max {
                    self.log_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.log_scroll = self.log_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let max = self.logs.len().saturating_sub(1);
                self.log_scroll = (self.log_scroll + 10).min(max);
            }
            KeyCode::Char('c') => {
                self.logs.clear();
            }
            _ => {}
        }
    }

    pub fn get_main_layout(&self, area: Rect) -> (Rect, Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);
        (chunks[0], chunks[1], chunks[2])
    }

    pub fn get_current_mode(&self) -> &str {
        self.modes
            .get(self.current_mode_index)
            .map(|s| s.as_str())
            .unwrap_or("Rule")
    }

    pub fn get_provider_update_info(&self, name: &str) -> Option<&ProviderUpdateInfo> {
        self.provider_update_info.get(name)
    }

    pub fn format_next_update(&self, name: &str) -> String {
        if let Some(info) = self.provider_update_info.get(name) {
            if info.is_updating {
                "更新中...".to_string()
            } else {
                let remaining = info.next_update.saturating_duration_since(Instant::now());
                format!("{}后", format_duration_detailed(remaining))
            }
        } else {
            "-".to_string()
        }
    }

    #[allow(dead_code)]
    pub fn core_state(&self) -> &str {
        if self.core_ready {
            "就绪"
        } else {
            "初始化中..."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{App, InputMode, PopupMode, clear_terminal_startup_artifacts};
    use crate::{clash::ClashClient, ui::Tab};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{
        Terminal,
        backend::{Backend, TestBackend},
        buffer::Cell,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn clear_terminal_startup_artifacts_should_remove_stale_cells() {
        let backend = TestBackend::new(20, 10);
        let mut terminal = Terminal::new(backend).expect("should create test terminal");

        let mut stale = Cell::default();
        stale.set_symbol("X");
        terminal
            .backend_mut()
            .draw([(0, 4, &stale)].into_iter())
            .expect("should inject stale terminal cell");

        clear_terminal_startup_artifacts(&mut terminal)
            .expect("should clear terminal before first frame");

        let cell = terminal.backend().buffer()[(0, 4)].symbol();
        assert_eq!(cell, " ", "stale startup content should be cleared");
    }

    fn test_app() -> App {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "clash-tui-input-test-{}-{}.yaml",
            std::process::id(),
            timestamp
        ));
        App::with_config(path)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn provider_form_should_accept_multiple_chinese_characters() {
        let mut app = test_app();
        app.open_add_provider();

        app.handle_input_key(key(KeyCode::Char('测'))).await;
        app.handle_input_key(key(KeyCode::Char('试'))).await;

        assert_eq!(app.input_fields[0], "测试");
        assert_eq!(app.input_cursor, 2);
    }

    #[tokio::test]
    async fn provider_form_backspace_should_delete_one_unicode_character() {
        let mut app = test_app();
        app.open_add_provider();

        app.handle_input_key(key(KeyCode::Char('订'))).await;
        app.handle_input_key(key(KeyCode::Char('阅'))).await;
        app.handle_input_key(key(KeyCode::Backspace)).await;

        assert_eq!(app.input_fields[0], "订");
        assert_eq!(app.input_cursor, 1);
    }

    #[tokio::test]
    async fn providers_tab_should_open_delete_confirm_with_lowercase_d() {
        let mut app = test_app();
        app.core_ready = true;
        app.current_tab = Tab::Providers;
        app.clash = Some(ClashClient::new("http://127.0.0.1:9090".to_string()));
        app.providers.push(crate::clash::Provider {
            name: "demo".to_string(),
            ..Default::default()
        });
        app.provider_expanded = vec![false];

        app.handle_key_event(key(KeyCode::Char('d')))
            .await
            .expect("key handler should not fail");

        assert_eq!(app.popup_mode, PopupMode::DeleteConfirm);
        assert_eq!(app.input_mode, InputMode::Editing);
    }

    #[tokio::test]
    async fn confirm_delete_provider_should_remove_provider_cache_file() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!(
            "clash-tui-delete-provider-test-{}-{}",
            std::process::id(),
            timestamp
        ));
        let config_path = base_dir.join("config.yaml");
        let cache_path = base_dir.join("providers").join("demo.yaml");

        fs::create_dir_all(cache_path.parent().expect("cache parent should exist"))
            .expect("should create cache directory");
        fs::write(&cache_path, "old-cache-body").expect("should create cache file");
        fs::write(
            &config_path,
            r#"
proxy-providers:
  demo:
    type: http
    url: https://example.com/sub
    interval: 86400
    path: ./providers/demo.yaml
"#,
        )
        .expect("should write test config");

        let mut app = App::with_config(config_path.clone());
        app.providers.push(crate::clash::Provider {
            name: "demo".to_string(),
            ..Default::default()
        });
        app.provider_expanded = vec![false];
        app.selected_provider = 0;

        app.confirm_delete_provider().await;

        let providers = app
            .config_manager
            .get_providers()
            .expect("should read updated providers");
        assert!(
            !providers.contains_key("demo"),
            "provider should be removed from config"
        );
        assert!(
            !cache_path.exists(),
            "provider cache file should be removed"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn confirm_delete_provider_should_ignore_missing_cache_file() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!(
            "clash-tui-delete-provider-missing-cache-test-{}-{}",
            std::process::id(),
            timestamp
        ));
        let config_path = base_dir.join("config.yaml");

        fs::create_dir_all(&base_dir).expect("should create base directory");
        fs::write(
            &config_path,
            r#"
proxy-providers:
  demo:
    type: http
    url: https://example.com/sub
    interval: 86400
    path: ./providers/demo.yaml
"#,
        )
        .expect("should write test config");

        let mut app = App::with_config(config_path.clone());
        app.providers.push(crate::clash::Provider {
            name: "demo".to_string(),
            ..Default::default()
        });
        app.provider_expanded = vec![false];
        app.selected_provider = 0;

        app.confirm_delete_provider().await;

        let providers = app
            .config_manager
            .get_providers()
            .expect("should read updated providers");
        assert!(
            !providers.contains_key("demo"),
            "provider should still be removed when cache file is missing"
        );

        let _ = fs::remove_dir_all(base_dir);
    }

    #[tokio::test]
    async fn confirm_delete_provider_should_use_locked_name_when_selection_changes() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!(
            "clash-tui-delete-provider-locked-name-test-{}-{}",
            std::process::id(),
            timestamp
        ));
        let config_path = base_dir.join("config.yaml");
        fs::create_dir_all(&base_dir).expect("should create base directory");
        fs::write(
            &config_path,
            r#"
proxy-providers:
  demo-a:
    type: http
    url: https://example.com/a
    interval: 86400
    path: ./providers/demo-a.yaml
  demo-b:
    type: http
    url: https://example.com/b
    interval: 86400
    path: ./providers/demo-b.yaml
"#,
        )
        .expect("should write test config");

        let mut app = App::with_config(config_path.clone());
        app.providers.push(crate::clash::Provider {
            name: "demo-a".to_string(),
            ..Default::default()
        });
        app.providers.push(crate::clash::Provider {
            name: "demo-b".to_string(),
            ..Default::default()
        });
        app.provider_expanded = vec![false, false];
        app.selected_provider = 0;

        app.open_delete_confirm();
        app.selected_provider = 1; // 模拟弹窗期间选择变化
        app.confirm_delete_provider().await;

        let providers = app
            .config_manager
            .get_providers()
            .expect("should read updated providers");
        assert!(
            !providers.contains_key("demo-a"),
            "locked provider should be deleted"
        );
        assert!(
            providers.contains_key("demo-b"),
            "other provider should remain"
        );

        let _ = fs::remove_dir_all(base_dir);
    }
}
