use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "clash-tui")]
#[command(about = "A htop-like TUI for Clash/Mihomo proxy")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Config file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Log level
    #[arg(short, long, global = true, default_value = "info")]
    pub log_level: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start TUI (default)
    Tui,
    /// Daemon mode - auto update subscriptions in background
    Daemon,
    /// Stop clash core via API
    Stop,
    /// Send restart signal to clash core
    Restart,
    /// Show clash core status
    Status,
    /// Stop clash core via API
    Quit,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
