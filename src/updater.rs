use crate::client::ClashClient;
use crate::config::ConfigManager;
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};
use tokio::time;

#[derive(Debug, Clone)]
pub struct UpdateTask {
    #[allow(dead_code)]
    pub name: String,
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
            name: name.clone(),
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
}

#[derive(Debug, Clone)]
pub enum UpdateEvent {
    Started(String),
    Completed(String, bool, Option<String>),
    Progress(String, String),
}

#[derive(Debug, Clone)]
struct NormalizedSubscription {
    body: String,
    proxy_count: usize,
}

pub struct Updater {
    tasks: Arc<Mutex<HashMap<String, UpdateTask>>>,
    client: Arc<ClashClient>,
    config_manager: Arc<ConfigManager>,
    event_tx: mpsc::UnboundedSender<UpdateEvent>,
    running: Arc<Mutex<bool>>,
}

impl Updater {
    pub fn new(
        client: ClashClient,
        config_manager: ConfigManager,
        event_tx: mpsc::UnboundedSender<UpdateEvent>,
    ) -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            client: Arc::new(client),
            config_manager: Arc::new(config_manager),
            event_tx,
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn load_tasks(&self) -> Result<()> {
        let providers = self.config_manager.get_providers()?;
        let mut tasks = self.tasks.lock().await;
        for (name, cfg) in providers {
            let interval = cfg.interval.unwrap_or(86400) as u64;
            tasks.insert(name.clone(), UpdateTask::new(name, cfg.url, interval));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn add_task(&self, name: String, url: String, interval_secs: u64) {
        let mut tasks = self.tasks.lock().await;
        tasks.insert(name.clone(), UpdateTask::new(name, url, interval_secs));
    }

    #[allow(dead_code)]
    pub async fn remove_task(&self, name: &str) {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(name);
    }

    #[allow(dead_code)]
    pub async fn update_task_interval(&self, name: &str, interval_secs: u64) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(name) {
            task.interval = Duration::from_secs(interval_secs);
            task.next_update = Instant::now() + task.interval;
        }
    }

    #[allow(dead_code)]
    pub async fn get_tasks(&self) -> HashMap<String, UpdateTask> {
        self.tasks.lock().await.clone()
    }

    pub async fn run(&self) {
        *self.running.lock().await = true;
        let mut interval = time::interval(Duration::from_secs(10));
        while *self.running.lock().await {
            interval.tick().await;
            let names: Vec<String> = {
                let tasks = self.tasks.lock().await;
                tasks
                    .iter()
                    .filter(|(_, t)| t.should_update())
                    .map(|(n, _)| n.clone())
                    .collect()
            };
            for name in names {
                self.update_provider(name).await;
            }
        }
    }

    async fn update_provider(&self, name: String) {
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(task) = tasks.get_mut(&name) {
                task.is_updating = true;
            }
        }
        let _ = self.event_tx.send(UpdateEvent::Started(name.clone()));

        let result = self.download_and_reload(&name).await;

        let (success, error_msg) = match &result {
            Ok(report) => {
                let _ = self.event_tx.send(UpdateEvent::Progress(
                    name.clone(),
                    format!("验证通过：核心已加载 {} 个节点", report.proxy_count),
                ));
                (true, None)
            }
            Err(e) => {
                let msg = format!("{:#}", e);
                let _ = self
                    .event_tx
                    .send(UpdateEvent::Progress(name.clone(), msg.clone()));
                (false, Some(msg))
            }
        };

        {
            let mut tasks = self.tasks.lock().await;
            if let Some(task) = tasks.get_mut(&name) {
                task.is_updating = false;
                task.last_update = Some(Instant::now());
                task.next_update = Instant::now() + task.interval;
                task.last_error = error_msg.clone();
            }
        }

        let _ = self
            .event_tx
            .send(UpdateEvent::Completed(name, success, error_msg));
    }

    async fn download_and_reload(&self, name: &str) -> Result<NormalizedSubscription> {
        let providers = self.config_manager.get_providers()?;
        let provider = providers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("provider '{}' not found in config", name))?;
        let path = provider
            .path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("provider '{}' missing path", name))?;
        let target_path = self.config_manager.resolve_provider_path(path);

        log::info!("Downloading subscription '{}'", name);
        self.send_progress(name, "开始下载".to_string());
        let body = self.client.fetch_subscription(&provider.url).await?;
        log::info!("Subscription '{}' downloaded, size: {}", name, body.len());
        self.send_progress(name, format!("下载完成：{} bytes", body.len()));

        let normalized = normalize_subscription(&body)
            .with_context(|| format!("normalize subscription '{}'", name))?;
        self.send_progress(
            name,
            format!("解析完成：发现 {} 个节点", normalized.proxy_count),
        );

        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create provider dir: {:?}", parent))?;
        }
        tokio::fs::write(&target_path, &normalized.body)
            .await
            .with_context(|| format!("write provider cache: {:?}", target_path))?;
        self.send_progress(name, format!("已写入缓存：{}", display_path(&target_path)));

        log::info!("Reloading config after provider update '{}'", name);
        let config_path = self
            .config_manager
            .config_path()
            .to_string_lossy()
            .to_string();
        self.send_progress(name, "正在重载核心配置".to_string());
        self.client
            .reload_config(&config_path)
            .await
            .with_context(|| format!("reload config after updating '{}'", name))?;
        self.send_progress(name, "核心配置已重载，正在验证节点".to_string());

        let loaded_count = self.wait_for_loaded_provider(name).await?;
        if loaded_count == 0 {
            return Err(anyhow!("provider '{}' loaded with 0 proxies", name));
        }
        Ok(NormalizedSubscription {
            body: normalized.body,
            proxy_count: loaded_count,
        })
    }

    async fn wait_for_loaded_provider(&self, name: &str) -> Result<usize> {
        let mut last_seen = String::new();
        for _ in 0..10 {
            match self.client.get_provider_proxy_counts().await {
                Ok(counts) => {
                    let provider_names = counts
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ");
                    last_seen = if provider_names.is_empty() {
                        "核心返回的订阅列表为空".to_string()
                    } else {
                        format!("核心当前订阅：{}", provider_names)
                    };

                    if let Some(count) = counts.get(name) {
                        if *count > 0 {
                            return Ok(*count);
                        }
                        last_seen = format!("核心已看到订阅 '{}'，但节点数为 0", name);
                    }
                }
                Err(e) => {
                    last_seen = format!("查询核心订阅失败：{}", e);
                }
            }
            time::sleep(Duration::from_millis(500)).await;
        }

        Err(anyhow!(
            "provider '{}' was not usable after reload: {}",
            name,
            last_seen
        ))
    }

    fn send_progress(&self, name: &str, message: String) {
        let _ = self
            .event_tx
            .send(UpdateEvent::Progress(name.to_string(), message));
    }

    #[allow(dead_code)]
    pub async fn force_update(&self, name: &str) {
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(task) = tasks.get_mut(name) {
                task.next_update = Instant::now();
            }
        }
        self.update_provider(name.to_string()).await;
    }

    #[allow(dead_code)]
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

fn normalize_subscription(body: &str) -> Result<NormalizedSubscription> {
    let yaml = serde_yaml::from_str::<serde_yaml::Value>(body).with_context(
        || "订阅内容不是 Clash YAML；请使用 Clash/Mihomo YAML 订阅或带 target=clash 的转换链接",
    )?;

    let proxies = match yaml {
        serde_yaml::Value::Mapping(mapping) => mapping
            .get(serde_yaml::Value::String("proxies".to_string()))
            .and_then(serde_yaml::Value::as_sequence)
            .cloned()
            .ok_or_else(|| {
                let keys = mapping
                    .iter()
                    .filter_map(|(key, _)| key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow!(
                    "订阅 YAML 缺少顶层 proxies 列表，现有字段：{}",
                    if keys.is_empty() { "<none>" } else { &keys }
                )
            })?,
        serde_yaml::Value::Sequence(sequence) => sequence,
        _ => {
            return Err(anyhow!(
                "订阅 YAML 格式不支持，必须是包含 proxies 的映射或节点列表"
            ));
        }
    };

    let proxy_count = proxies
        .iter()
        .filter(|proxy| {
            proxy
                .as_mapping()
                .and_then(|mapping| mapping.get(serde_yaml::Value::String("name".to_string())))
                .and_then(serde_yaml::Value::as_str)
                .is_some()
                && proxy
                    .as_mapping()
                    .and_then(|mapping| mapping.get(serde_yaml::Value::String("type".to_string())))
                    .and_then(serde_yaml::Value::as_str)
                    .is_some()
        })
        .count();
    if proxy_count == 0 {
        return Err(anyhow!(
            "订阅里没有可识别节点：每个节点至少需要 name 和 type"
        ));
    }

    let mut provider = serde_yaml::Mapping::new();
    provider.insert(
        serde_yaml::Value::String("proxies".to_string()),
        serde_yaml::Value::Sequence(proxies),
    );
    let body = serde_yaml::to_string(&serde_yaml::Value::Mapping(provider))
        .with_context(|| "serialize normalized provider yaml")?;

    Ok(NormalizedSubscription { body, proxy_count })
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_manager;
    use std::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn normalize_subscription_extracts_proxies_from_full_config() {
        let input = r#"
mixed-port: 7890
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
proxies:
  - name: node-a
    type: ss
    server: example.com
    port: 443
    cipher: aes-128-gcm
    password: pass
rules:
  - MATCH,Proxy
"#;

        let normalized = normalize_subscription(input).unwrap();

        assert_eq!(normalized.proxy_count, 1);
        let yaml: serde_yaml::Value = serde_yaml::from_str(&normalized.body).unwrap();
        assert!(yaml.get("proxies").is_some());
        assert!(yaml.get("proxy-groups").is_none());
        assert!(yaml.get("rules").is_none());
    }

    #[test]
    fn normalize_subscription_wraps_proxy_sequence() {
        let input = r#"
- name: node-a
  type: direct
- name: node-b
  type: reject
"#;

        let normalized = normalize_subscription(input).unwrap();

        assert_eq!(normalized.proxy_count, 2);
        let yaml: serde_yaml::Value = serde_yaml::from_str(&normalized.body).unwrap();
        assert_eq!(yaml.get("proxies").unwrap().as_sequence().unwrap().len(), 2);
    }

    #[test]
    fn normalize_subscription_rejects_yaml_without_proxies() {
        let input = r#"
proxy-groups:
  - name: Proxy
    type: select
"#;

        let error = normalize_subscription(input).unwrap_err().to_string();

        assert!(error.contains("缺少顶层 proxies"));
    }

    #[tokio::test]
    async fn update_should_load_nodes_into_running_core() {
        if core_manager::bundled_core_path().is_err() {
            eprintln!("skipping core smoke test: bundled mihomo core is not present");
            return;
        }

        let api_port = free_port();
        let mixed_port = free_port();
        let dir = std::env::temp_dir().join(format!("clash-tui-core-smoke-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");
        std::fs::write(
            &config_path,
            format!(
                r#"mixed-port: {mixed_port}
external-controller: 127.0.0.1:{api_port}
allow-lan: false
mode: rule
log-level: error
proxy-providers: {{}}
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
rules:
  - MATCH,Proxy
"#
            ),
        )
        .unwrap();

        let subscription = r#"
proxies:
  - name: local-ss-obfs
    type: ss
    server: 127.0.0.1
    port: 12345
    cipher: chacha20-ietf-poly1305
    password: test-password
    plugin: obfs
    plugin-opts:
      mode: http
      host: example.com
    udp: true
"#;
        let subscription_url = serve_subscription_once(subscription).await;
        let api_url = format!("http://127.0.0.1:{api_port}");

        core_manager::start_core(&config_path).await.unwrap();
        wait_for_core(&api_url).await;

        let manager = ConfigManager::new(Some(config_path.clone())).unwrap();
        manager
            .add_provider("local", &subscription_url, 3600)
            .unwrap();
        let client = ClashClient::new(api_url);
        let verify_client = client.clone();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let updater = Updater::new(client, manager.clone(), tx);

        let report = updater.download_and_reload("local").await.unwrap();

        assert_eq!(report.proxy_count, 1);
        let providers = verify_client.get_providers().await.unwrap();
        let provider = providers
            .iter()
            .find(|provider| provider.name == "local")
            .unwrap();
        assert_eq!(provider.proxies.len(), 1);

        let proxies = verify_client.get_proxies().await.unwrap();
        let proxy_group = proxies.iter().find(|proxy| proxy.name == "Proxy").unwrap();
        assert!(
            proxy_group
                .all
                .as_ref()
                .unwrap()
                .contains(&"local-ss-obfs".to_string())
        );
        let cache =
            std::fs::read_to_string(manager.resolve_provider_path("./providers/local.yaml"))
                .unwrap();
        assert!(cache.contains("proxies:"));
        assert!(!cache.contains("proxy-groups:"));
        let mut messages = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let UpdateEvent::Progress(_, message) = event {
                messages.push(message);
            }
        }
        assert!(messages.iter().any(|message| message.contains("下载完成")));
        assert!(messages.iter().any(|message| message.contains("解析完成")));
        assert!(
            messages
                .iter()
                .any(|message| message.contains("已写入缓存"))
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("核心配置已重载"))
        );

        core_manager::stop_core().await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn free_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    async fn serve_subscription_once(body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/yaml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{addr}/sub.yaml")
    }

    async fn wait_for_core(api_url: &str) {
        for _ in 0..40 {
            if core_manager::is_core_running(api_url).await {
                return;
            }
            time::sleep(Duration::from_millis(250)).await;
        }
        core_manager::stop_core().await;
        panic!("embedded core did not become ready at {api_url}");
    }
}
