use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "clash-tui")]
#[command(about = "Clash TUI - 纯 Rust 实现的 Clash 终端界面")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// 配置文件路径
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// 日志级别
    #[arg(short, long, global = true, default_value = "info")]
    pub log_level: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 启动 TUI 界面 (默认)
    Tui,
    /// 静默模式 - 仅启动 clash-rs 核心（后台运行）
    Daemon {
        /// 写入 PID 文件
        #[arg(short, long)]
        pid_file: Option<PathBuf>,
    },
    /// 停止 clash-rs 核心
    Stop,
    /// 重启 clash-rs 核心
    Restart,
    /// 查看 clash-rs 状态
    Status,
    /// 完全退出（停止 clash-tui 和 clash-rs 核心）
    Quit,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
