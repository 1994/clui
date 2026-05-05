use crate::client::ClashClient;
use crate::commands::{AppModeKind, Command, CommandExecutor, CommandRegistry};
use crate::config::{ClashConfig, ConfigManager};
use crate::context::{AppContext, UiState};
use crate::core_manager;
use crate::event::{Event, EventHandler};
use crate::state::{
    AppState, Config, ConnectionsResponse, Memory, Provider, Proxy, ProxyHistory, Rule, Traffic,
    Version,
};
use crate::ui::Popup;
use crate::updater::{UpdateEvent, Updater};
use anyhow::Result;
use crossterm::event::KeyEvent;
use futures::{StreamExt, stream};
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
    speed_test_tx: mpsc::UnboundedSender<SpeedTestResult>,
    speed_test_rx: mpsc::UnboundedReceiver<SpeedTestResult>,
    core_started: bool,
}

struct SpeedTestResult {
    proxy_name: String,
    result: Result<u32, String>,
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
        let client = ClashClient::new(api_url.clone());
        let event_handler = EventHandler::new(Duration::from_millis(100));

        let mut state = AppState {
            api_url: api_url.clone(),
            config_path: config_manager.config_path().display().to_string(),
            ..Default::default()
        };
        load_local_config_info(&config_manager, &mut state);

        let (speed_test_tx, speed_test_rx) = mpsc::unbounded_channel();

        let ctx = AppContext {
            state,
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
            speed_test_tx,
            speed_test_rx,
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
        load_local_config_info(&self.ctx.config_manager, &mut self.ctx.state);

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
            self.expire_toast();

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
                Some(result) = self.speed_test_rx.recv() => {
                    self.handle_speed_test_result(result);
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
                    self.cycle_mode();
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
                        if let Some((group, node)) = self
                            .ctx
                            .state
                            .proxies
                            .iter()
                            .find(|p| &p.name == proxy_name)
                            .and_then(|proxy| {
                                proxy
                                    .all
                                    .as_ref()
                                    .and_then(|all| all.get(selected))
                                    .map(|node| (proxy.name.clone(), node.clone()))
                            })
                        {
                            self.switch_proxy_to(&group, &node);
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
                Command::ProxyPrev => {
                    self.switch_selected_proxy_by(-1);
                }
                Command::ProxyNext => {
                    self.switch_selected_proxy_by(1);
                }
                Command::ProxySpeedTest => {
                    self.start_selected_speed_test();
                }
                Command::ProxySpeedTestAll => {
                    self.start_all_speed_tests();
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
        self.ctx.ui.refresh_in_progress = true;

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
                self.ctx.ui.refresh_in_progress = false;
                self.apply_refresh_result(result);
                self.ctx.ui.last_refresh_at = Some(Instant::now());
                if self.ctx.ui.success_message.as_deref() == Some("正在刷新") {
                    self.ctx.ui.success_message = None;
                }
                if self.ctx.connected && self.updater.is_none() {
                    self.start_updater().await;
                }
            }
            Err(e) => {
                self.ctx.ui.refresh_in_progress = false;
                log::warn!("refresh task failed: {}", e);
            }
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

    fn cycle_mode(&mut self) {
        self.ctx.ui.current_mode_index =
            (self.ctx.ui.current_mode_index + 1) % self.ctx.ui.modes.len();
        let mode = self.ctx.ui.modes[self.ctx.ui.current_mode_index].clone();
        if let Some(ref mut config) = self.ctx.state.config {
            config.mode = mode.to_lowercase();
        }
        let client = self.ctx.client.clone();
        let mode_for_api = mode.to_lowercase();
        tokio::spawn(async move {
            if let Err(e) = client.change_mode(&mode_for_api).await {
                log::warn!("切换代理模式失败: {}", e);
            }
        });
        self.ctx.ui.success_message = Some(format!("正在切换为 {}", mode));
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
            load_local_config_info(&self.ctx.config_manager, &mut self.ctx.state);
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
                load_local_config_info(&self.ctx.config_manager, &mut self.ctx.state);

                if let Some(updater) = self.updater.as_ref() {
                    updater
                        .add_task(name.clone(), url.clone(), interval as u64)
                        .await;
                    let updater = updater.clone();
                    let name_for_update = name.clone();
                    self.ctx.ui.updating_providers.insert(name.clone());
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
                    load_local_config_info(&self.ctx.config_manager, &mut self.ctx.state);

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
            self.ctx.ui.updating_providers.insert(name.clone());
            tokio::spawn(async move {
                updater.force_update(&name_for_update).await;
            });
            self.ctx.ui.success_message = Some(format!("订阅 '{}' 正在更新", name));
            return;
        }
        self.ctx.ui.updating_providers.insert(name.clone());
        match self.ctx.client.update_provider_proxy(&name).await {
            Ok(()) => {
                self.ctx.ui.updating_providers.remove(&name);
                self.ctx.ui.success_message = Some(format!("订阅 '{}' 更新完成", name));
                self.start_refresh();
            }
            Err(e) => {
                self.ctx.ui.updating_providers.remove(&name);
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
            self.ctx.ui.updating_providers.insert(name.clone());
            if let Err(e) = self.ctx.client.update_provider_proxy(&name).await {
                self.ctx.ui.updating_providers.remove(&name);
                self.ctx.ui.error_message = Some(format!("订阅 '{}' 更新失败: {}", name, e));
                return;
            }
            self.ctx.ui.updating_providers.remove(&name);
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
                self.ctx.ui.updating_providers.insert(name.clone());
                log::info!("provider update started: {}", name);
                self.add_log(format!("[{}] 开始更新订阅 '{}'...", now(), name));
            }
            UpdateEvent::Completed(name, success, error) => {
                self.ctx.ui.updating_providers.remove(&name);
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
                self.ctx.ui.updating_providers.insert(name.clone());
                log::info!("provider update progress: {} - {}", name, msg);
                self.add_log(format!("[{}] 订阅 '{}' - {}", now(), name, msg));
            }
        }
    }

    fn switch_selected_proxy_by(&mut self, delta: isize) {
        let Some(proxy) = self.ctx.state.proxies.get(self.ctx.ui.proxy_selected) else {
            self.ctx.ui.error_message = Some("没有可切换的代理组".to_string());
            return;
        };

        let Some(all) = proxy.all.as_ref().filter(|all| !all.is_empty()) else {
            self.ctx.ui.error_message = Some("当前代理组没有可选节点".to_string());
            return;
        };

        let current_idx = proxy
            .now
            .as_ref()
            .and_then(|now| all.iter().position(|node| node == now))
            .unwrap_or(0);
        let next_idx = if delta < 0 {
            current_idx.saturating_sub(1)
        } else {
            (current_idx + 1).min(all.len() - 1)
        };

        if next_idx == current_idx {
            return;
        }

        let group = proxy.name.clone();
        let node = all[next_idx].clone();
        self.switch_proxy_to(&group, &node);
    }

    fn switch_proxy_to(&mut self, group: &str, node: &str) {
        if let Some(proxy) = self
            .ctx
            .state
            .proxies
            .iter_mut()
            .find(|proxy| proxy.name == group)
        {
            proxy.now = Some(node.to_string());
        }
        if let Some(proxy) = self.ctx.state.all_proxies.get_mut(group) {
            proxy.now = Some(node.to_string());
        }

        let client = self.ctx.client.clone();
        let group = group.to_string();
        let node = node.to_string();
        let message = format!("已切换到 {}", node);
        tokio::spawn(async move {
            if let Err(e) = client.switch_proxy(&group, &node).await {
                log::warn!("切换代理节点失败: {}", e);
            }
        });
        self.ctx.ui.success_message = Some(message);
        self.start_refresh();
    }

    fn start_selected_speed_test(&mut self) {
        let Some(name) = self
            .ctx
            .state
            .proxies
            .get(self.ctx.ui.proxy_selected)
            .map(|proxy| proxy.name.clone())
        else {
            self.ctx.ui.error_message = Some("没有可测速的代理组".to_string());
            return;
        };

        self.start_speed_test(name);
    }

    fn start_all_speed_tests(&mut self) {
        let names: Vec<String> = self
            .ctx
            .state
            .proxies
            .iter()
            .map(|proxy| proxy.name.clone())
            .collect();
        if names.is_empty() {
            self.ctx.ui.error_message = Some("没有可测速的代理组".to_string());
            return;
        }

        for name in &names {
            self.ctx.ui.testing_proxies.insert(name.clone());
        }
        self.ctx.ui.success_message = Some(format!("正在测速 {} 个代理组", names.len()));

        let client = self.ctx.client.clone();
        let tx = self.speed_test_tx.clone();
        tokio::spawn(async move {
            stream::iter(names)
                .for_each_concurrent(8, |name| {
                    let client = client.clone();
                    let tx = tx.clone();
                    async move {
                        let result = client
                            .test_proxy_delay(&name, None, Some(3000))
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx.send(SpeedTestResult {
                            proxy_name: name,
                            result,
                        });
                    }
                })
                .await;
        });
    }

    fn start_speed_test(&mut self, name: String) {
        self.ctx.ui.testing_proxies.insert(name.clone());
        self.ctx.ui.success_message = Some(format!("正在测速 {}", name));

        let client = self.ctx.client.clone();
        let tx = self.speed_test_tx.clone();
        tokio::spawn(async move {
            let result = client
                .test_proxy_delay(&name, None, Some(3000))
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(SpeedTestResult {
                proxy_name: name,
                result,
            });
        });
    }

    fn handle_speed_test_result(&mut self, result: SpeedTestResult) {
        self.ctx.ui.testing_proxies.remove(&result.proxy_name);
        match result.result {
            Ok(delay) => {
                update_proxy_delay(&mut self.ctx.state.proxies, &result.proxy_name, delay);
                if let Some(proxy) = self.ctx.state.all_proxies.get_mut(&result.proxy_name) {
                    set_proxy_delay(proxy, delay);
                }
                if self.ctx.ui.testing_proxies.is_empty() {
                    self.ctx.ui.success_message = Some("测速完成".to_string());
                }
            }
            Err(e) => {
                log::warn!("测速失败 {}: {}", result.proxy_name, e);
                if self.ctx.ui.testing_proxies.is_empty() {
                    self.ctx.ui.error_message = Some(format!("测速失败: {}", e));
                }
            }
        }
    }

    fn expire_toast(&mut self) {
        let current = self
            .ctx
            .ui
            .error_message
            .as_ref()
            .or(self.ctx.ui.success_message.as_ref())
            .cloned();

        let Some(current) = current else {
            self.ctx.ui.toast_text = None;
            self.ctx.ui.toast_set_at = None;
            return;
        };

        if self.ctx.ui.toast_text.as_deref() != Some(current.as_str()) {
            self.ctx.ui.toast_text = Some(current);
            self.ctx.ui.toast_set_at = Some(Instant::now());
            return;
        }

        if self
            .ctx
            .ui
            .toast_set_at
            .is_some_and(|set_at| set_at.elapsed() > Duration::from_secs(5))
        {
            self.ctx.ui.success_message = None;
            self.ctx.ui.error_message = None;
            self.ctx.ui.toast_text = None;
            self.ctx.ui.toast_set_at = None;
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

fn update_proxy_delay(proxies: &mut [Proxy], name: &str, delay: u32) {
    if let Some(proxy) = proxies.iter_mut().find(|proxy| proxy.name == name) {
        set_proxy_delay(proxy, delay);
    }
}

fn set_proxy_delay(proxy: &mut Proxy, delay: u32) {
    let history = ProxyHistory {
        time: chrono::Local::now().to_rfc3339(),
        delay,
        mean_delay: None,
    };
    if let Some(first) = proxy.history.first_mut() {
        *first = history;
    } else {
        proxy.history.push(history);
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

fn local_config_to_runtime_config(config: ClashConfig) -> Config {
    Config {
        port: config.port,
        socks_port: config.socks_port,
        redir_port: config.redir_port,
        tproxy_port: config.tproxy_port,
        mixed_port: config.mixed_port,
        mode: config.mode.unwrap_or_else(|| "rule".to_string()),
        allow_lan: config.allow_lan.unwrap_or(false),
        bind_address: config.bind_address.unwrap_or_default(),
        log_level: config.log_level.unwrap_or_default(),
        ..Default::default()
    }
}

fn load_local_config_info(config_manager: &ConfigManager, state: &mut AppState) {
    state.api_url = config_manager.get_api_url();
    state.config_path = config_manager.config_path().display().to_string();
    if let Ok(config) = config_manager.load() {
        let config = local_config_to_runtime_config(config);
        state.proxy_config = Some(config.clone());
        if state.config.is_none() {
            state.config = Some(config);
        }
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
