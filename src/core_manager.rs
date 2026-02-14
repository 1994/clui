use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

const API_PORT: u16 = 9090;
const API_HOST: &str = "127.0.0.1";

/// clash-rs 核心状态
#[derive(Debug, Clone, PartialEq)]
pub enum CoreState {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// 核心管理 trait
#[async_trait::async_trait]
pub trait CoreManager: Send + Sync {
    /// 获取 API 地址
    fn api_url(&self) -> String;

    /// 检测是否已有核心在运行
    async fn detect_existing(&mut self) -> Result<bool>;

    /// 启动核心
    async fn start(&mut self) -> Result<()>;

    /// 停止核心
    async fn stop(&mut self) -> Result<()>;

    /// 重启核心
    async fn restart(&mut self) -> Result<()> {
        self.stop().await?;
        sleep(Duration::from_millis(500)).await;
        self.start().await
    }

    /// 获取当前状态
    fn state(&self) -> CoreState;

    /// 是否是内嵌模式
    #[allow(dead_code)]
    fn is_embedded(&self) -> bool;

    /// 是否正在运行
    fn is_running(&self) -> bool {
        matches!(self.state(), CoreState::Running)
    }
}

/// 库模式 - 直接调用 clash-lib
pub struct EmbeddedCoreManager {
    config_path: PathBuf,
    state: CoreState,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EmbeddedCoreManager {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            state: CoreState::Stopped,
            shutdown_tx: None,
        }
    }

    fn ensure_core_log_file() -> Option<String> {
        let log_dir = dirs::config_dir()
            .map(|d| d.join("clash-tui").join("logs"))
            .unwrap_or_else(|| PathBuf::from("logs"));

        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            log::warn!("创建 clash 核心日志目录失败 {:?}: {}", log_dir, e);
            return None;
        }

        let log_path = log_dir.join("clash-core.log");
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(_) => Some(log_path.to_string_lossy().to_string()),
            Err(e) => {
                log::warn!("创建 clash 核心日志文件失败 {:?}: {}", log_path, e);
                None
            }
        }
    }
}

#[async_trait::async_trait]
impl CoreManager for EmbeddedCoreManager {
    fn api_url(&self) -> String {
        format!("http://{}:{}", API_HOST, API_PORT)
    }

    async fn detect_existing(&mut self) -> Result<bool> {
        // 检查是否已有 clash-rs 在运行
        if let Ok(resp) = reqwest::get(format!("{}/version", self.api_url())).await
            && resp.status().is_success()
        {
            self.state = CoreState::Running;
            log::info!("检测到已运行的 clash-rs");
            return Ok(true);
        }
        Ok(false)
    }

    async fn start(&mut self) -> Result<()> {
        if self.state == CoreState::Running {
            return Ok(());
        }

        self.state = CoreState::Starting;
        log::info!("启动 clash-rs (库模式)...");

        // 确保配置存在
        if !self.config_path.exists() {
            create_default_config(&self.config_path)?;
        }

        // 加载配置
        let config_str = tokio::fs::read_to_string(&self.config_path)
            .await
            .with_context(|| format!("读取配置失败: {:?}", self.config_path))?;

        // 在内嵌核心场景关闭核心 stdout 日志，避免污染 TUI。
        // 日志仍通过 log_file 落盘，便于排查问题。
        let mut config_value: serde_yaml::Value = serde_yaml::from_str(&config_str)
            .with_context(|| format!("解析配置失败: {:?}", self.config_path))?;
        if let Some(mapping) = config_value.as_mapping_mut() {
            mapping.insert(
                serde_yaml::Value::String("log-level".to_string()),
                serde_yaml::Value::String("off".to_string()),
            );
        }
        let config_str = serde_yaml::to_string(&config_value)
            .with_context(|| format!("序列化配置失败: {:?}", self.config_path))?;

        // 使用 clash-lib 的 API 启动
        let opts = clash::Options {
            config: clash::Config::Str(config_str),
            cwd: self
                .config_path
                .parent()
                .map(|p| p.to_string_lossy().to_string()),
            rt: Some(clash::TokioRuntime::MultiThread),
            log_file: Self::ensure_core_log_file(),
        };

        // 创建关闭信号通道
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        // 在新线程中启动 clash-rs (因为 start_scaffold 是阻塞的)
        std::thread::spawn(move || {
            if let Err(e) = clash::start_scaffold(opts) {
                log::error!("clash-rs 启动错误: {}", e);
            }
        });

        // 等待 API 就绪（最多 60 秒）
        log::info!("等待 clash-rs API 就绪...");
        for i in 0..120 {
            sleep(Duration::from_millis(500)).await;
            if let Ok(resp) = reqwest::get(format!("{}/version", self.api_url())).await
                && resp.status().is_success()
            {
                self.state = CoreState::Running;
                log::info!("clash-rs 启动成功 ({} 次重试)", i);
                return Ok(());
            }
            // 检查是否收到关闭信号
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
        }

        self.state = CoreState::Error("启动超时".to_string());
        Err(anyhow::anyhow!("clash-rs API 启动超时"))
    }

    async fn stop(&mut self) -> Result<()> {
        log::info!("停止 clash-rs (库模式)...");

        // 调用 clash-lib 的 shutdown 函数
        if clash::shutdown() {
            log::info!("已发送关闭信号");
        }

        // 发送关闭信号（用于中断启动等待）
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        self.state = CoreState::Stopped;
        log::info!("clash-rs 已停止");
        Ok(())
    }

    fn state(&self) -> CoreState {
        self.state.clone()
    }

    fn is_embedded(&self) -> bool {
        true
    }
}

impl Drop for EmbeddedCoreManager {
    fn drop(&mut self) {
        if self.state == CoreState::Running {
            // 调用 clash-lib 的 shutdown 函数
            clash::shutdown();
            if let Some(tx) = self.shutdown_tx.take() {
                let _ = tx.send(());
            }
        }
    }
}

/// 创建默认配置
fn create_default_config(config_path: &PathBuf) -> Result<()> {
    let default_config = r#"# clash-rs 默认配置
mixed-port: 7890
external-controller: 127.0.0.1:9090
log-level: info
mode: rule

proxy-providers: {}

proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT

rules:
  - MATCH,DIRECT
"#;

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(config_path, default_config)
        .with_context(|| format!("创建默认配置失败: {:?}", config_path))?;

    log::info!("创建默认配置: {:?}", config_path);
    Ok(())
}

/// 展开 ~ 为用户目录
#[allow(dead_code)]
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(stripped);
    }
    PathBuf::from(path)
}

/// 获取默认配置路径
pub fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("clash-tui").join("config.yaml"))
        .unwrap_or_else(|| PathBuf::from("config.yaml"))
}

/// 创建核心管理器（库模式）
pub fn create_core_manager(config_path: PathBuf) -> Box<dyn CoreManager> {
    Box::new(EmbeddedCoreManager::new(config_path))
}
