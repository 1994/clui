use crate::clash::ClashClient;
use crate::config::ConfigManager;
use anyhow::{Context, Result, anyhow};
use reqwest::header::USER_AGENT;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tokio::time;

#[derive(Debug, Clone)]
pub struct UpdateTask {
    #[allow(dead_code)]
    pub provider_name: String,
    #[allow(dead_code)]
    pub url: String,
    pub interval: Duration,
    pub last_update: Option<Instant>,
    pub next_update: Instant,
    pub is_updating: bool,
    pub last_error: Option<String>,
}

impl UpdateTask {
    pub fn new(name: String, url: String, interval_secs: u64) -> Self {
        let interval = Duration::from_secs(interval_secs);
        Self {
            provider_name: name,
            url,
            interval,
            last_update: None,
            next_update: Instant::now() + interval,
            is_updating: false,
            last_error: None,
        }
    }

    pub fn should_update(&self) -> bool {
        !self.is_updating && Instant::now() >= self.next_update
    }

    pub fn mark_updating(&mut self) {
        self.is_updating = true;
    }

    pub fn mark_completed(&mut self, success: bool, error: Option<String>) {
        self.is_updating = false;
        self.last_update = Some(Instant::now());
        self.next_update = Instant::now() + self.interval;
        if !success {
            self.last_error = error;
        } else {
            self.last_error = None;
        }
    }

    #[allow(dead_code)]
    pub fn time_until_next(&self) -> Duration {
        let now = Instant::now();
        if self.next_update > now {
            self.next_update - now
        } else {
            Duration::from_secs(0)
        }
    }
}

#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    Started(String),
    Completed(String, bool, Option<String>), // name, success, error_message
    #[allow(dead_code)]
    Progress(String, String),
}

pub struct Scheduler {
    tasks: Arc<Mutex<HashMap<String, UpdateTask>>>,
    clash: Arc<ClashClient>,
    config_manager: Arc<ConfigManager>,
    event_tx: mpsc::UnboundedSender<SchedulerEvent>,
    running: Arc<Mutex<bool>>,
}

impl Scheduler {
    pub fn new(
        clash: ClashClient,
        config_manager: ConfigManager,
        event_tx: mpsc::UnboundedSender<SchedulerEvent>,
    ) -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            clash: Arc::new(clash),
            config_manager: Arc::new(config_manager),
            event_tx,
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn load_tasks(&self) -> Result<()> {
        let providers = self.config_manager.get_providers()?;
        let mut tasks = self.tasks.lock().await;

        for (name, config) in providers {
            let interval = config.interval.unwrap_or(86400) as u64;
            let task = UpdateTask::new(name.clone(), config.url, interval);
            tasks.insert(name, task);
        }

        Ok(())
    }

    pub async fn add_task(&self, name: String, url: String, interval_secs: u64) {
        let mut tasks = self.tasks.lock().await;
        let task = UpdateTask::new(name.clone(), url, interval_secs);
        tasks.insert(name, task);
    }

    pub async fn remove_task(&self, name: &str) {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(name);
    }

    pub async fn update_task_interval(&self, name: &str, interval_secs: u64) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(name) {
            task.interval = Duration::from_secs(interval_secs);
            task.next_update = Instant::now() + task.interval;
        }
    }

    pub async fn get_tasks(&self) -> HashMap<String, UpdateTask> {
        self.tasks.lock().await.clone()
    }

    pub async fn run(&self) {
        *self.running.lock().await = true;
        let mut interval = time::interval(Duration::from_secs(10)); // Check every 10 seconds

        while *self.running.lock().await {
            interval.tick().await;

            let tasks_to_update = {
                let tasks = self.tasks.lock().await;
                tasks
                    .iter()
                    .filter(|(_, task)| task.should_update())
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>()
            };

            for name in tasks_to_update {
                self.update_provider(name).await;
            }
        }
    }

    async fn update_provider(&self, name: String) {
        // Mark as updating
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(task) = tasks.get_mut(&name) {
                task.mark_updating();
            }
        }

        let _ = self.event_tx.send(SchedulerEvent::Started(name.clone()));

        let result = match self.clash.update_provider(&name).await {
            Ok(_) => Ok(()),
            Err(api_err) => {
                match self.download_provider_snapshot_and_reload(&name).await {
                    Ok(_) => {
                        self.event_tx
                            .send(SchedulerEvent::Progress(
                                name.clone(),
                                "核心更新失败，已回退为直连下载并重载配置".to_string(),
                            ))
                            .ok();
                        Ok(())
                    }
                    Err(fallback_err) => Err(anyhow!(
                        "{}；回退下载也失败: {}",
                        api_err,
                        fallback_err
                    )),
                }
            }
        };

        let success = result.is_ok();
        let error_msg = result.err().map(|e| e.to_string());

        // Mark as completed
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(task) = tasks.get_mut(&name) {
                task.mark_completed(success, error_msg.clone());
            }
        }

        let _ = self
            .event_tx
            .send(SchedulerEvent::Completed(name, success, error_msg));
    }

    fn resolve_provider_path(&self, path: &str) -> PathBuf {
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

    async fn download_provider_snapshot_and_reload(&self, name: &str) -> Result<()> {
        let providers = self.config_manager.get_providers()?;
        let provider = providers
            .get(name)
            .ok_or_else(|| anyhow!("配置中不存在订阅 '{}'", name))?;

        let path = provider
            .path
            .as_deref()
            .ok_or_else(|| anyhow!("订阅 '{}' 缺少 path 配置", name))?;
        let target_path = self.resolve_provider_path(path);

        let response = reqwest::Client::new()
            .get(provider.url.as_str())
            .header(USER_AGENT, "clash-rs/0.0.0")
            .send()
            .await
            .with_context(|| format!("拉取订阅 '{}' 失败", name))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("读取订阅 '{}' 响应失败", name))?;

        if !status.is_success() {
            let detail = body.trim();
            if detail.is_empty() {
                return Err(anyhow!("拉取订阅 '{}' 失败: HTTP {}", name, status));
            }
            return Err(anyhow!(
                "拉取订阅 '{}' 失败: HTTP {} - {}",
                name,
                status,
                detail
            ));
        }

        if !body.contains("proxies:") {
            return Err(anyhow!(
                "拉取订阅 '{}' 失败: 响应不含 proxies 字段",
                name
            ));
        }

        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("创建订阅目录失败: {:?}", parent))?;
        }

        tokio::fs::write(&target_path, body)
            .await
            .with_context(|| format!("写入订阅缓存失败: {:?}", target_path))?;

        self.clash
            .reload_config(true)
            .await
            .with_context(|| format!("回退下载后重载配置失败: '{}'", name))?;

        Ok(())
    }

    pub async fn force_update(&self, name: &str) -> Result<()> {
        // Update next_update to now
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(task) = tasks.get_mut(name) {
                task.next_update = Instant::now();
            }
        }

        self.update_provider(name.to_string()).await;
        Ok(())
    }

    pub async fn update_all(&self) {
        let names: Vec<String> = {
            let tasks = self.tasks.lock().await;
            tasks.keys().cloned().collect()
        };

        for name in names {
            self.update_provider(name).await;
        }
    }

    #[allow(dead_code)]
    pub async fn stop(&self) {
        *self.running.lock().await = false;
    }
}

#[allow(dead_code)]
pub fn format_duration_short(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

pub fn format_duration_detailed(duration: Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    let mut parts = vec![];
    if days > 0 {
        parts.push(format!("{}天", days));
    }
    if hours > 0 {
        parts.push(format!("{}小时", hours));
    }
    if minutes > 0 && days == 0 {
        parts.push(format!("{}分", minutes));
    }
    if seconds > 0 && days == 0 && hours == 0 {
        parts.push(format!("{}秒", seconds));
    }

    if parts.is_empty() {
        "即将".to_string()
    } else {
        parts.join("")
    }
}
