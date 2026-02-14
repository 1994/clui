use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process;
use std::time::SystemTime;

/// 实例状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub pid: u32,
    pub start_time: SystemTime,
    pub api_url: String,
    pub mode: InstanceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstanceMode {
    Tui,    // TUI 交互模式
    Daemon, // 后台守护模式
}

/// 单例管理器
pub struct InstanceManager {
    pid_file: PathBuf,
    info_file: PathBuf,
}

impl InstanceManager {
    pub fn new(config_path: &Path) -> Self {
        let data_dir = dirs::data_dir()
            .map(|d| d.join("clash-tui"))
            .unwrap_or_else(|| PathBuf::from(".clash-tui"));

        let _ = std::fs::create_dir_all(&data_dir);

        // 使用配置文件的哈希或路径来区分不同配置的运行实例
        let config_hash = format!(
            "{:x}",
            md5::compute(config_path.to_string_lossy().as_bytes())
        );

        Self {
            pid_file: data_dir.join(format!("clash-tui-{}.pid", config_hash)),
            info_file: data_dir.join(format!("clash-tui-{}.json", config_hash)),
        }
    }

    /// 检查是否已有实例在运行
    pub fn check_existing(&self) -> Option<InstanceInfo> {
        // 1. 检查 PID 文件
        let pid = match std::fs::read_to_string(&self.pid_file) {
            Ok(content) => content.trim().parse::<u32>().ok()?,
            Err(_) => return None,
        };

        // 2. 检查进程是否存在
        if !self.process_exists(pid) {
            // 清理残留文件
            let _ = std::fs::remove_file(&self.pid_file);
            let _ = std::fs::remove_file(&self.info_file);
            return None;
        }

        // 3. 读取实例信息
        match std::fs::read_to_string(&self.info_file) {
            Ok(content) => serde_json::from_str(&content).ok(),
            Err(_) => None,
        }
    }

    /// 注册当前实例
    pub fn register(&self, mode: InstanceMode, api_url: String) -> Result<()> {
        let info = InstanceInfo {
            pid: process::id(),
            start_time: SystemTime::now(),
            api_url,
            mode,
        };

        // 写入 PID 文件
        std::fs::write(&self.pid_file, info.pid.to_string())
            .with_context(|| format!("写入 PID 文件失败: {:?}", self.pid_file))?;

        // 写入信息文件
        let json = serde_json::to_string(&info)?;
        std::fs::write(&self.info_file, json)
            .with_context(|| format!("写入实例信息失败: {:?}", self.info_file))?;

        Ok(())
    }

    /// 注销实例（清理文件）
    pub fn unregister(&self) {
        let _ = std::fs::remove_file(&self.pid_file);
        let _ = std::fs::remove_file(&self.info_file);
    }

    /// 检查进程是否存在
    #[cfg(unix)]
    fn process_exists(&self, pid: u32) -> bool {
        use std::process::Command;
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    fn process_exists(&self, pid: u32) -> bool {
        use std::process::Command;
        Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }

    /// 获取 PID 文件路径
    #[allow(dead_code)]
    pub fn pid_file(&self) -> &PathBuf {
        &self.pid_file
    }
}

impl Drop for InstanceManager {
    fn drop(&mut self) {
        // 如果是当前进程注册的，清理文件
        if let Ok(content) = std::fs::read_to_string(&self.pid_file)
            && let Ok(pid) = content.trim().parse::<u32>()
            && pid == process::id()
        {
            self.unregister();
        }
    }
}

/// 连接到已运行实例的状态
#[derive(Debug)]
pub struct RunningInstance {
    pub info: InstanceInfo,
    client: crate::clash::ClashClient,
}

impl RunningInstance {
    pub fn new(info: InstanceInfo) -> Self {
        let client = crate::clash::ClashClient::new(info.api_url.clone());
        Self { info, client }
    }

    /// 获取状态摘要
    pub async fn get_status(&self) -> Result<InstanceStatus> {
        let version = self.client.get_version().await.ok();
        let config = self.client.get_config().await.ok();
        let traffic = self.client.get_traffic().await.ok();

        Ok(InstanceStatus {
            pid: self.info.pid,
            mode: format!("{:?}", self.info.mode),
            version: version.map(|v| v.version),
            mode_config: config.map(|c| c.mode),
            uptime: SystemTime::now()
                .duration_since(self.info.start_time)
                .map(|d| format!("{:?}", d))
                .unwrap_or_else(|_| "unknown".to_string()),
            traffic,
        })
    }

    /// 请求退出
    #[allow(dead_code)]
    pub async fn request_quit(&self) -> Result<()> {
        // 通过 API 发送停止命令
        self.client.shutdown_core().await
    }
}

#[derive(Debug)]
pub struct InstanceStatus {
    #[allow(dead_code)]
    pub pid: u32,
    #[allow(dead_code)]
    pub mode: String,
    pub version: Option<String>,
    pub mode_config: Option<String>,
    pub uptime: String,
    pub traffic: Option<crate::clash::Traffic>,
}
