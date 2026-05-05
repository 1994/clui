use crate::client::ClashClient;
use crate::commands::{AppModeKind, Command, CommandExecutor, CommandRegistry};
use crate::config::ConfigManager;
use crate::context::{AppContext, UiState};
use crate::core_manager;
use crate::event::{Event, EventHandler};
use crate::state::{
    AppState, Config, ConnectionsResponse, Memory, Provider, Proxy, Rule, Traffic, Version,
};
use crate::ui::Popup;
use crate::updater::{UpdateEvent, Updater};
use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{Terminal, backend::Backend};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::interval;

pub struct App {
    ctx: AppContext,
    registry: CommandRegistry,
    event_handler: EventHandler,
    updater_rx: Option<mpsc::UnboundedReceiver<UpdateEvent>>,
    updater: Option<Arc<Updater>>,
    last_refresh: Instant,
    refresh_handle: Option<JoinHandle<RefreshResult>>,
    core_started: bool,
}

struct RefreshResult {
    version: Result<Version, String>,
    data: Option<RefreshData>,
}

struct RefreshData {
    config: Result<Config, String>,
    memory: Result<Memory, String>,
    proxies: Result<Vec<Proxy>, String>,
    all_proxies: Result<std::collections::HashMap<String, Proxy>, String>,
    providers: Result<Vec<Provider>, String>,
    connections: Result<ConnectionsResponse, String>,
    rules: Result<Vec<Rule>, String>,
}

impl App {
    pub fn new(config_path: Option<std::path::PathBuf>) -> Result<Self> {
        let config_manager = ConfigManager::new(config_path)?;
        let api_url = config_manager.get_api_url();
        let client = ClashClient::new(api_url);
        let event_handler = EventHandler::new(Duration::from_millis(100));

        let ctx = AppContext {
            state: AppState::default(),
            ui: UiState::default(),
            client,
            config_manager,
            running: true,
            connected: false,
        };

        Ok(Self {
            ctx,
            registry: CommandRegistry::new(),
            event_handler,
            updater_rx: None,
            updater: None,
            last_refresh: Instant::now(),
            refresh_handle: None,
            core_started: false,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let config_path = self.ctx.config_manager.config_path().clone();
        self.ctx
            .config_manager
            .migrate_legacy_local_config_if_needed()?;
        core_manager::ensure_config(&config_path)?;
        let mut config_repaired = match self.ctx.config_manager.repair_for_tui() {
            Ok(repaired) => repaired,
            Err(e) => {
                self.ctx.ui.error_message = Some(format!("配置自动修复失败: {}", e));
                false
            }
        };
        match self.ctx.config_manager.repair_provider_caches() {
            Ok(count) if count > 0 => {
                config_repaired = true;
                self.add_log(format!("[{}] 已修复 {} 个订阅缓存", now(), count));
            }
            Ok(_) => {}
            Err(e) => {
                self.ctx.ui.error_message = Some(format!("订阅缓存自动修复失败: {}", e));
            }
        }
        self.merge_config_providers_into_state();

        let api_url = self.ctx.config_manager.get_api_url();
        self.ctx.client = ClashClient::new(api_url.clone());

        // Check if an existing core is already running (user may have started one manually)
        if core_manager::is_core_running(&api_url).await {
            self.ctx.connected = true;
            self.ctx.ui.success_message = Some("已连接到 Clash/Mihomo".to_string());
            let config_path = self
                .ctx
                .config_manager
                .config_path()
                .to_string_lossy()
                .to_string();
            if let Err(e) = self.ctx.client.reload_config(&config_path).await {
                self.ctx.ui.error_message = Some(format!("核心已连接但配置重载失败: {}", e));
            } else if config_repaired {
                self.ctx.ui.success_message = Some("配置已自动修复并重载".to_string());
            }
        } else {
            log::info!("Starting bundled Mihomo core...");
            match core_manager::start_core(&config_path).await {
                Ok(_) => {
                    self.core_started = true;
                    // Keep startup responsive; the periodic refresh will attach if the
                    // core finishes booting after the first screen is interactive.
                    for _ in 0..5 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        if core_manager::is_core_running(&api_url).await {
                            self.ctx.connected = true;
                            self.ctx.ui.success_message = Some("Mihomo 核心启动成功".to_string());
                            self.ctx.ui.error_message = None;
                            break;
                        }
                    }
                    if !self.ctx.connected {
                        self.ctx.ui.error_message = Some("Mihomo 核心启动超时".to_string());
                    }
                }
                Err(e) => {
                    self.ctx.ui.error_message = Some(format!("启动 Mihomo 核心失败: {}", e));
                    log::error!("Mihomo core failed to start: {}", e);
                }
            }
        }

        if self.ctx.connected {
            if let Ok(v) = self.ctx.client.get_version().await {
                self.ctx.state.version = Some(v);
            }

            self.start_updater().await;
        }

        Ok(())
    }

    async fn start_updater(&mut self) {
        if self.updater.is_some() {
            return;
        }

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let updater = Updater::new(
            self.ctx.client.clone(),
            self.ctx.config_manager.clone(),
            event_tx,
        );
        if let Err(e) = updater.load_tasks().await {
            log::warn!("加载订阅任务失败: {}", e);
        }
        let updater = Arc::new(updater);
        let updater_clone = updater.clone();
        tokio::spawn(async move {
            updater_clone.run().await;
        });
        self.updater = Some(updater);
        self.updater_rx = Some(event_rx);
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut tick = interval(Duration::from_secs(1));

        while self.ctx.running {
            self.apply_finished_refresh().await;

            terminal.draw(|f| {
                crate::tabs::render(f, &self.ctx.state, &mut self.ctx.ui);
                if self.ctx.ui.help_active {
                    crate::tabs::render_help(f, f.area());
                }
            })?;

            tokio::select! {
                _ = tick.tick() => {
                    if self.last_refresh.elapsed() >= Duration::from_secs(1) {
                        self.start_refresh();
                        self.last_refresh = Instant::now();
                    }
                }
                Some(event) = self.event_handler.next() => {
                    self.handle_event(event).await?;
                }
                Some(event) = async {
                    if let Some(ref mut rx) = self.updater_rx {
                        rx.recv().await
                    } else {
                        futures::future::pending().await
                    }
                } => {
                    self.handle_update_event(event).await;
                }
            }
        }

        if let Some(handle) = self.refresh_handle.take() {
            handle.abort();
        }
        if self.core_started {
            core_manager::stop_core().await;
        }

        Ok(())
    }

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Tick => {}
            Event::Key(key) => self.handle_key(key).await?,
            Event::Mouse(_mouse) => {}
            Event::Resize(_, _) => {}
        }
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // F1 toggle help
        if key.code == crossterm::event::KeyCode::F(1) {
            self.ctx.ui.help_active = !self.ctx.ui.help_active;
            return Ok(());
        }

        if self.ctx.ui.help_active {
            self.ctx.ui.help_active = false;
            return Ok(());
        }

        let mode = self.current_mode();
        let cmds = self.registry.resolve(key, self.ctx.ui.current_tab, mode);

        for cmd in &cmds {
            CommandExecutor::execute(cmd, &mut self.ctx);
        }

        self.handle_side_effects(&cmds).await;

        Ok(())
    }

    async fn handle_side_effects(&mut self, cmds: &[Command]) {
        for cmd in cmds {
            match cmd {
                Command::Refresh => {
                    self.start_refresh();
                    self.ctx.ui.success_message = Some("正在刷新".to_string());
                }
                Command::CycleMode => {
                    self.cycle_mode().await;
                }
                Command::ToggleSearch => {
                    self.ctx.ui.search_active = true;
                    self.ctx.ui.search_query.clear();
                }
                Command::Confirm => match self.ctx.ui.popup {
                    Popup::DeleteConfirm => self.confirm_delete_provider().await,
                    Popup::ProxyNodes {
                        ref proxy_name,
                        selected,
                    } => {
                        if let Some(proxy) = self
                            .ctx
                            .state
                            .proxies
                            .iter()
                            .find(|p| &p.name == proxy_name)
                            && let Some(ref all) = proxy.all
                            && let Some(node) = all.get(selected)
                        {
                            let _ = self.ctx.client.switch_proxy(&proxy.name, node).await;
                        }
                        self.ctx.ui.popup = Popup::None;
                    }
                    _ => {}
                },
                Command::Cancel => {
                    self.ctx.ui.popup = Popup::None;
                    self.ctx.ui.search_active = false;
                    self.ctx.ui.search_query.clear();
                }
                Command::FormSubmit => {
                    self.submit_form().await;
                }
                Command::SearchCancel => {
                    self.ctx.ui.search_active = false;
                    self.ctx.ui.search_query.clear();
                }
                Command::ProviderUpdate => {
                    self.update_selected_provider().await;
                }
                Command::ProviderUpdateAll => {
                    self.update_all_providers().await;
                }
                Command::ProviderHealthCheck => {
                    self.health_check_selected_provider().await;
                }
                _ => {}
            }
        }
    }

    fn current_mode(&self) -> AppModeKind {
        if self.ctx.ui.search_active {
            return AppModeKind::Search;
        }
        match self.ctx.ui.popup {
            Popup::None => AppModeKind::Normal,
            Popup::AddProvider | Popup::EditProvider => AppModeKind::Form,
            Popup::DeleteConfirm => AppModeKind::Popup,
            Popup::ProxyNodes { .. } => AppModeKind::ProxyNodes,
        }
    }

    fn merge_config_providers_into_state(&mut self) {
        let Ok(config_providers) = self.ctx.config_manager.get_providers() else {
            return;
        };

        for (name, cfg) in config_providers {
            if self
                .ctx
                .state
                .providers
                .iter()
                .any(|provider| provider.name == name)
            {
                continue;
            }

            self.ctx.state.providers.push(Provider {
                name,
                provider_type: "Proxy".to_string(),
                vehicle_type: cfg.provider_type,
                path: cfg.path,
                ..Default::default()
            });
        }

        self.ctx.state.providers.sort_by(|a, b| a.name.cmp(&b.name));
        if self.ctx.state.providers.is_empty() {
            self.ctx.ui.provider_selected = 0;
        } else {
            self.ctx.ui.provider_selected = self
                .ctx
                .ui
                .provider_selected
                .min(self.ctx.state.providers.len() - 1);
        }
        if self.ctx.ui.provider_expanded.len() != self.ctx.state.providers.len() {
            self.ctx.ui.provider_expanded = vec![false; self.ctx.state.providers.len()];
        }
    }

    fn start_refresh(&mut self) {
        self.merge_config_providers_into_state();

        if self.refresh_handle.is_some() {
            return;
        }

        let client = self.ctx.client.clone();
        let connected = self.ctx.connected;
        self.refresh_handle = Some(tokio::spawn(async move {
            fetch_refresh_result(client, connected).await
        }));
    }

    async fn apply_finished_refresh(&mut self) {
        let Some(handle) = self.refresh_handle.as_ref() else {
            return;
        };
        if !handle.is_finished() {
            return;
        }

        let handle = self
            .refresh_handle
            .take()
            .expect("refresh handle exists after is_finished check");
        match handle.await {
            Ok(result) => {
                self.apply_refresh_result(result);
                if self.ctx.connected && self.updater.is_none() {
                    self.start_updater().await;
                }
            }
            Err(e) => log::warn!("refresh task failed: {}", e),
        }
    }

    fn apply_refresh_result(&mut self, result: RefreshResult) {
        let Some(data) = (match result.version {
            Ok(v) => {
                if !self.ctx.connected {
                    log::info!("refresh: core connected");
                }
                self.ctx.state.version = Some(v);
                self.ctx.connected = true;
                self.ctx.ui.error_message = None;
                result.data
            }
            Err(err) => {
                log::warn!("refresh failed: version: {}", err);
                if self.ctx.connected {
                    self.ctx.ui.error_message = Some(format!("与核心断开连接: {}", err));
                }
                self.ctx.connected = false;
                return;
            }
        }) else {
            return;
        };

        if let Ok(c) = data.config {
            if let Some(idx) = self
                .ctx
                .ui
                .modes
                .iter()
                .position(|m| m.to_lowercase() == c.mode.to_lowercase())
            {
                self.ctx.ui.current_mode_index = idx;
            }
            self.ctx.state.config = Some(c);
        } else if let Err(e) = data.config {
            log::warn!("refresh config failed: {}", e);
        }
        if let Ok(m) = data.memory {
            self.ctx.state.memory = Some(m);
        } else if let Err(e) = data.memory {
            log::warn!("refresh memory failed: {}", e);
        }
        if let Ok(p) = data.proxies {
            log::info!("refresh proxies ok: groups={}", p.len());
            self.ctx.state.proxies = p;
            if self.ctx.ui.proxy_expanded.len() != self.ctx.state.proxies.len() {
                self.ctx.ui.proxy_expanded = vec![false; self.ctx.state.proxies.len()];
            }
        } else if let Err(e) = data.proxies {
            log::warn!("refresh proxies failed: {}", e);
            self.ctx.ui.error_message = Some(format!("代理组刷新失败: {}", e));
        }
        if let Ok(a) = data.all_proxies {
            log::info!("refresh all_proxies ok: total={}", a.len());
            self.ctx.state.all_proxies = a;
        } else if let Err(e) = data.all_proxies {
            log::warn!("refresh all_proxies failed: {}", e);
        }
        if let Ok(p) = data.providers {
            let summary = p
                .iter()
                .map(|provider| format!("{}:{}", provider.name, provider.proxies.len()))
                .collect::<Vec<_>>()
                .join(", ");
            log::info!("refresh providers ok: count={} [{}]", p.len(), summary);
            self.ctx.state.providers = p;
        } else if let Err(e) = data.providers {
            log::warn!("refresh providers failed: {}", e);
            self.ctx.ui.error_message = Some(format!("订阅刷新失败: {}", e));
        }
        self.merge_config_providers_into_state();
        if let Ok(c) = data.connections {
            let previous_down = self.ctx.state.download_total;
            let previous_up = self.ctx.state.upload_total;
            self.ctx.state.connections = c.connections;
            self.ctx.state.download_total = c.download_total;
            self.ctx.state.upload_total = c.upload_total;

            let traffic = Traffic {
                up: c.upload_total.saturating_sub(previous_up),
                down: c.download_total.saturating_sub(previous_down),
            };
            self.ctx.state.traffic = traffic.clone();
            self.ctx
                .ui
                .traffic_history
                .push_back((traffic.up, traffic.down));
            if self.ctx.ui.traffic_history.len() > 60 {
                self.ctx.ui.traffic_history.pop_front();
            }
        } else if let Err(e) = data.connections {
            log::warn!("refresh connections failed: {}", e);
        }
        if let Ok(r) = data.rules {
            self.ctx.state.rules = r;
        } else if let Err(e) = data.rules {
            log::warn!("refresh rules failed: {}", e);
        }
    }

    async fn cycle_mode(&mut self) {
        self.ctx.ui.current_mode_index =
            (self.ctx.ui.current_mode_index + 1) % self.ctx.ui.modes.len();
        let mode = self.ctx.ui.modes[self.ctx.ui.current_mode_index].clone();
        if let Some(ref mut config) = self.ctx.state.config {
            config.mode = mode.to_lowercase();
        }
        let _ = self.ctx.client.change_mode(&mode.to_lowercase()).await;
        self.ctx.ui.success_message = Some(format!("模式切换为: {}", mode));
    }

    async fn confirm_delete_provider(&mut self) {
        if let Some(provider) = self.ctx.state.providers.get(self.ctx.ui.provider_selected) {
            let name = provider.name.clone();
            let cache_path = self
                .ctx
                .config_manager
                .get_providers()
                .ok()
                .and_then(|configs| configs.get(&name).and_then(|cfg| cfg.path.clone()))
                .or_else(|| provider.path.clone())
                .map(|p| self.ctx.config_manager.resolve_provider_path(&p));

            if let Err(e) = self.ctx.config_manager.remove_provider(&name) {
                self.ctx.ui.error_message = Some(format!("删除失败: {}", e));
                return;
            }
            if let Some(updater) = self.updater.as_ref() {
                updater.remove_task(&name).await;
            }

            if let Some(path) = cache_path {
                let _ = tokio::fs::remove_file(&path).await;
            }

            self.ctx.state.providers.retain(|p| p.name != name);
            self.ctx.ui.provider_expanded = vec![false; self.ctx.state.providers.len()];
            if self.ctx.state.providers.is_empty() {
                self.ctx.ui.provider_selected = 0;
            } else {
                self.ctx.ui.provider_selected = self
                    .ctx
                    .ui
                    .provider_selected
                    .min(self.ctx.state.providers.len() - 1);
            }

            let config_path = self
                .ctx
                .config_manager
                .config_path()
                .to_string_lossy()
                .to_string();
            if let Err(e) = self.ctx.client.reload_config(&config_path).await {
                self.ctx.ui.error_message = Some(format!("配置已删除但核心重载失败: {}", e));
            } else {
                self.ctx.ui.error_message = None;
            }
            self.ctx.ui.success_message = Some(format!("订阅 '{}' 已删除", name));
            self.ctx.ui.popup = Popup::None;
        }
    }

    async fn submit_form(&mut self) {
        match self.ctx.ui.popup {
            Popup::AddProvider => {
                let name = self.ctx.ui.input_fields[0].trim().to_string();
                let url = self.ctx.ui.input_fields[1].trim().to_string();
                let interval: u32 = self.ctx.ui.input_fields[2].parse().unwrap_or(86400);

                if name.is_empty() {
                    self.ctx.ui.error_message = Some("订阅名称不能为空".to_string());
                    return;
                }
                if !url.starts_with("http") {
                    self.ctx.ui.error_message = Some("请输入有效的 URL".to_string());
                    return;
                }

                if let Err(e) = self.ctx.config_manager.add_provider(&name, &url, interval) {
                    self.ctx.ui.error_message = Some(format!("添加失败: {}", e));
                    return;
                }

                if let Some(updater) = self.updater.as_ref() {
                    updater
                        .add_task(name.clone(), url.clone(), interval as u64)
                        .await;
                    let updater = updater.clone();
                    let name_for_update = name.clone();
                    tokio::spawn(async move {
                        updater.force_update(&name_for_update).await;
                    });
                } else {
                    let config_path = self
                        .ctx
                        .config_manager
                        .config_path()
                        .to_string_lossy()
                        .to_string();
                    if let Err(e) = self.ctx.client.reload_config(&config_path).await {
                        self.ctx.ui.error_message =
                            Some(format!("配置已保存但核心重载失败: {}", e));
                        self.ctx.ui.popup = Popup::None;
                        return;
                    }
                }
                self.start_refresh();
                self.ctx.ui.success_message = Some(format!("订阅 '{}' 已保存，正在更新", name));
                self.ctx.ui.popup = Popup::None;
            }
            Popup::EditProvider => {
                if let Some(provider) = self.ctx.state.providers.get(self.ctx.ui.provider_selected)
                {
                    let name = provider.name.clone();
                    let url = self.ctx.ui.input_fields[1].trim();
                    let interval: u32 = self.ctx.ui.input_fields[2].parse().unwrap_or(86400);

                    if !url.starts_with("http") {
                        self.ctx.ui.error_message = Some("请输入有效的 URL".to_string());
                        return;
                    }

                    if let Err(e) =
                        self.ctx
                            .config_manager
                            .update_provider(&name, Some(url), Some(interval))
                    {
                        self.ctx.ui.error_message = Some(format!("更新失败: {}", e));
                        return;
                    }

                    if let Some(updater) = self.updater.as_ref() {
                        updater.update_task_interval(&name, interval as u64).await;
                    }

                    let config_path = self
                        .ctx
                        .config_manager
                        .config_path()
                        .to_string_lossy()
                        .to_string();
                    if let Err(e) = self.ctx.client.reload_config(&config_path).await {
                        self.ctx.ui.error_message =
                            Some(format!("配置已更新但核心重载失败: {}", e));
                        self.ctx.ui.popup = Popup::None;
                        return;
                    }
                    self.start_refresh();
                    self.ctx.ui.success_message = Some(format!("订阅 '{}' 更新成功", name));
                    self.ctx.ui.popup = Popup::None;
                }
            }
            _ => {}
        }
    }

    async fn update_selected_provider(&mut self) {
        let Some(provider) = self.ctx.state.providers.get(self.ctx.ui.provider_selected) else {
            self.ctx.ui.error_message = Some("没有可更新的订阅".to_string());
            return;
        };
        let name = provider.name.clone();
        if let Some(updater) = self.updater.as_ref() {
            let updater = updater.clone();
            let name_for_update = name.clone();
            tokio::spawn(async move {
                updater.force_update(&name_for_update).await;
            });
            self.ctx.ui.success_message = Some(format!("订阅 '{}' 正在更新", name));
            return;
        }
        match self.ctx.client.update_provider_proxy(&name).await {
            Ok(()) => {
                self.ctx.ui.success_message = Some(format!("订阅 '{}' 更新完成", name));
                self.start_refresh();
            }
            Err(e) => {
                self.ctx.ui.error_message = Some(format!("订阅 '{}' 更新失败: {}", name, e));
            }
        }
    }

    async fn update_all_providers(&mut self) {
        if self.ctx.state.providers.is_empty() {
            self.ctx.ui.error_message = Some("没有可更新的订阅".to_string());
            return;
        }
        if let Some(updater) = self.updater.as_ref() {
            let updater = updater.clone();
            tokio::spawn(async move {
                updater.update_all().await;
            });
            self.ctx.ui.success_message = Some("所有订阅正在更新".to_string());
            return;
        }
        let names: Vec<String> = self
            .ctx
            .state
            .providers
            .iter()
            .map(|provider| provider.name.clone())
            .collect();
        for name in names {
            if let Err(e) = self.ctx.client.update_provider_proxy(&name).await {
                self.ctx.ui.error_message = Some(format!("订阅 '{}' 更新失败: {}", name, e));
                return;
            }
        }
        self.ctx.ui.success_message = Some("所有订阅更新完成".to_string());
        self.start_refresh();
    }

    async fn health_check_selected_provider(&mut self) {
        let Some(provider) = self.ctx.state.providers.get(self.ctx.ui.provider_selected) else {
            self.ctx.ui.error_message = Some("没有可检查的订阅".to_string());
            return;
        };
        let name = provider.name.clone();
        match self.ctx.client.health_check_provider(&name).await {
            Ok(()) => {
                self.ctx.ui.success_message = Some(format!("订阅 '{}' 健康检查已开始", name));
                self.start_refresh();
            }
            Err(e) => {
                self.ctx.ui.error_message = Some(format!("订阅 '{}' 健康检查失败: {}", name, e));
            }
        }
    }

    async fn handle_update_event(&mut self, event: UpdateEvent) {
        match event {
            UpdateEvent::Started(name) => {
                log::info!("provider update started: {}", name);
                self.add_log(format!("[{}] 开始更新订阅 '{}'...", now(), name));
            }
            UpdateEvent::Completed(name, success, error) => {
                if success {
                    log::info!("provider update completed: {} success", name);
                    self.add_log(format!("[{}] 订阅 '{}' 更新成功", now(), name));
                    self.ctx.ui.success_message = Some(format!("订阅 '{}' 更新成功", name));
                } else {
                    let err = error.unwrap_or_else(|| "未知错误".to_string());
                    log::warn!("provider update completed: {} failed: {}", name, err);
                    self.add_log(format!("[{}] 订阅 '{}' 更新失败: {}", now(), name, err));
                    self.ctx.ui.error_message = Some(format!("订阅 '{}' 更新失败: {}", name, err));
                }
                self.start_refresh();
            }
            UpdateEvent::Progress(name, msg) => {
                log::info!("provider update progress: {} - {}", name, msg);
                self.add_log(format!("[{}] 订阅 '{}' - {}", now(), name, msg));
            }
        }
    }

    fn add_log(&mut self, message: String) {
        const MAX_LOGS: usize = 500;
        const MAX_LOG_CHARS: usize = 400;
        self.ctx
            .state
            .logs
            .push(truncate_chars(&message, MAX_LOG_CHARS));
        if self.ctx.state.logs.len() > MAX_LOGS {
            self.ctx.state.logs.remove(0);
        }
    }
}

async fn fetch_refresh_result(client: ClashClient, connected: bool) -> RefreshResult {
    if !connected {
        return RefreshResult {
            version: client.get_version().await.map_err(|e| e.to_string()),
            data: None,
        };
    }

    let (version, config, memory, proxies, all_proxies, providers, connections, rules) = tokio::join!(
        client.get_version(),
        client.get_config(),
        client.get_memory(),
        client.get_proxies(),
        client.get_all_proxies(),
        client.get_providers(),
        client.get_connections(),
        client.get_rules(),
    );

    RefreshResult {
        version: version.map_err(|e| e.to_string()),
        data: Some(RefreshData {
            config: config.map_err(|e| e.to_string()),
            memory: memory.map_err(|e| e.to_string()),
            proxies: proxies.map_err(|e| e.to_string()),
            all_proxies: all_proxies.map_err(|e| e.to_string()),
            providers: providers.map_err(|e| e.to_string()),
            connections: connections.map_err(|e| e.to_string()),
            rules: rules.map_err(|e| e.to_string()),
        }),
    }
}

fn now() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}
