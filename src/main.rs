mod app;
mod args;
mod clash;
mod config;
mod core_manager;
mod event;
mod instance;
mod scheduler;
mod ui;

use anyhow::Result;
use args::{Cli, Commands};
use core_manager::{create_core_manager, get_default_config_path};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// 清理7天前的旧日志文件
fn cleanup_old_logs(log_dir: &PathBuf) {
    let seven_days_ago = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(7 * 24 * 60 * 60))
        .unwrap_or_else(std::time::SystemTime::now);

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata()
                && let Ok(modified) = metadata.modified()
                && modified < seven_days_ago
                && metadata.is_file()
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// 初始化 clash-tui 的日志（使用 log + env_logger，与 clash-rs 的 tracing 不冲突）
///
/// 当 `tui_mode` 为 true 时，日志只写入文件，不输出到终端（避免污染 TUI 界面）
fn init_logging(log_level: &str, tui_mode: bool) {
    // 获取日志目录
    let log_dir = dirs::config_dir()
        .map(|d| d.join("clash-tui").join("logs"))
        .unwrap_or_else(|| PathBuf::from("logs"));

    let _ = std::fs::create_dir_all(&log_dir);
    cleanup_old_logs(&log_dir);

    // 构建日志文件路径
    let log_file_path = log_dir.join("clash-tui.log");

    // 使用 env_logger 初始化 log crate
    let env = env_logger::Env::default().default_filter_or(log_level);

    let mut builder = env_logger::Builder::from_env(env);
    builder.format_timestamp_secs();

    if tui_mode {
        // TUI 模式：日志写入文件，避免污染终端
        if let Ok(target) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
        {
            builder.target(env_logger::Target::Pipe(Box::new(target)));
        }
    }

    builder.init();

    if !tui_mode {
        log::info!("clash-tui 日志目录: {:?}", log_dir);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();

    // 判断是否是 TUI 模式（需要提前判断，因为初始化日志的方式不同）
    let is_tui_mode = cli.command.is_none() || matches!(cli.command, Some(Commands::Tui));

    // 初始化日志（TUI 模式下日志只写入文件，不输出到终端）
    init_logging(&cli.log_level, is_tui_mode);

    // 确定配置文件路径
    let config_path = cli.config.unwrap_or_else(get_default_config_path);

    // 创建实例管理器
    let instance_mgr = instance::InstanceManager::new(&config_path);

    match cli.command {
        None | Some(Commands::Tui) => {
            // 检查是否已有实例在运行
            if let Some(existing) = instance_mgr.check_existing() {
                println!("clash-tui 已在运行 (PID: {})", existing.pid);
                println!("模式: {:?}", existing.mode);
                println!("API: {}", existing.api_url);
                println!();

                // 尝试获取更详细的状态
                let running = instance::RunningInstance::new(existing);
                match running.get_status().await {
                    Ok(status) => {
                        println!("运行时间: {}", status.uptime);
                        if let Some(ver) = status.version {
                            println!("版本: {}", ver);
                        }
                        if let Some(mode) = status.mode_config {
                            println!("代理模式: {}", mode);
                        }
                        if let Some(traffic) = status.traffic {
                            println!(
                                "上传: {} | 下载: {}",
                                format_bytes(traffic.up),
                                format_bytes(traffic.down)
                            );
                        }
                    }
                    Err(e) => {
                        println!("获取详细状态失败: {}", e);
                    }
                }

                println!();
                println!("提示: 使用 'clash-tui quit' 完全退出后台进程");
                return Ok(());
            }

            // 注册当前实例
            instance_mgr.register(
                instance::InstanceMode::Tui,
                format!("http://127.0.0.1:{}", 9090),
            )?;

            // 启动 TUI 模式
            run_tui(config_path).await?;
        }
        Some(Commands::Daemon { pid_file: _ }) => {
            // 检查是否已有实例在运行
            if instance_mgr.check_existing().is_some() {
                println!("clash-tui 已在后台运行");
                return Ok(());
            }

            // 注册当前实例
            instance_mgr.register(
                instance::InstanceMode::Daemon,
                format!("http://127.0.0.1:{}", 9090),
            )?;

            // 启动静默模式
            run_daemon(config_path, None).await?;
        }
        Some(Commands::Stop) => {
            // 停止 clash-rs（保持 clash-tui 运行）
            stop_daemon(config_path).await?;
        }
        Some(Commands::Restart) => {
            // 重启 clash-rs
            restart_daemon(config_path).await?;
        }
        Some(Commands::Status) => {
            // 查看状态
            show_status(config_path).await?;
        }
        Some(Commands::Quit) => {
            // 完全退出（clash-tui + clash-rs）
            quit_all(config_path).await?;
        }
    }

    Ok(())
}

/// 格式化字节大小
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

/// TUI 模式
async fn run_tui(config_path: PathBuf) -> Result<()> {
    use crossterm::{
        cursor::{Hide, Show},
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        },
    };
    use ratatui::{Terminal, backend::CrosstermBackend};
    use std::io;

    // 先清除屏幕，避免残留内容（必须在重定向前执行）
    let mut stdout = io::stdout();
    execute!(stdout, Hide, Clear(ClearType::All), Clear(ClearType::Purge))?;

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::with_config(config_path);
    let res = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    )?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

/// 静默模式 - 仅启动 clash-rs
async fn run_daemon(config_path: PathBuf, pid_file: Option<PathBuf>) -> Result<()> {
    log::info!("启动 clash-rs 静默模式...");

    // 检查是否已有实例在运行
    let mut manager = create_core_manager(config_path);
    if manager.detect_existing().await? {
        println!("clash-rs 已在运行");
        return Ok(());
    }

    // 启动 clash-rs
    if let Err(e) = manager.start().await {
        eprintln!("启动失败: {}", e);
        std::process::exit(1);
    }

    // 写入 PID 文件
    if let Some(pid_path) = pid_file {
        let pid = std::process::id();
        if let Err(e) = tokio::fs::write(&pid_path, pid.to_string()).await {
            log::warn!("写入 PID 文件失败: {}", e);
        } else {
            log::info!("PID 文件: {:?}", pid_path);
        }
    }

    println!("clash-rs 已启动");
    println!("API: {}", manager.api_url());

    // 保持运行，监控核心状态
    loop {
        sleep(Duration::from_secs(5)).await;

        if !manager.is_running() {
            log::error!("clash-rs 已停止，尝试重启...");
            if let Err(e) = manager.restart().await {
                log::error!("重启失败: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// 停止 clash-rs
async fn stop_daemon(_config_path: PathBuf) -> Result<()> {
    // 通过 API 发送停止命令
    let api_url = format!("http://127.0.0.1:{}", 9090);
    let client = clash::ClashClient::new(api_url);

    match client.shutdown_core().await {
        Ok(_) => println!("clash-rs 已停止"),
        Err(e) => {
            log::warn!("通过 API 停止失败: {}", e);
            println!("clash-rs 可能未运行或无法通过 API 停止");
        }
    }

    Ok(())
}

/// 重启 clash-rs
async fn restart_daemon(config_path: PathBuf) -> Result<()> {
    let mut manager = create_core_manager(config_path.clone());

    // 先尝试停止已运行的实例
    if manager.detect_existing().await? {
        println!("正在停止 clash-rs...");
        let client = clash::ClashClient::new(manager.api_url());
        let _ = client.shutdown_core().await;
        sleep(Duration::from_millis(500)).await;
    }

    // 启动新实例
    manager.start().await?;

    println!("clash-rs 已重启");
    println!("API: {}", manager.api_url());

    Ok(())
}

/// 显示状态
async fn show_status(config_path: PathBuf) -> Result<()> {
    let mut manager = create_core_manager(config_path);

    if manager.detect_existing().await? {
        let client = clash::ClashClient::new(manager.api_url());

        match client.get_version().await {
            Ok(version) => {
                println!("状态: 运行中");
                println!("版本: {}", version.version);
                println!("API: {}", manager.api_url());
            }
            Err(_) => {
                println!("状态: 运行中 (无法获取版本)");
                println!("API: {}", manager.api_url());
            }
        }
    } else {
        println!("状态: 未运行");
    }

    Ok(())
}

/// 完全退出（clash-tui + clash-rs）
async fn quit_all(config_path: PathBuf) -> Result<()> {
    let instance_mgr = instance::InstanceManager::new(&config_path);

    // 1. 检查是否有实例在运行
    let existing = match instance_mgr.check_existing() {
        Some(info) => info,
        None => {
            println!("clash-tui 未在运行");

            // 尝试停止 clash-rs（可能没有 clash-tui 但 clash-rs 在运行）
            let api_url = format!("http://127.0.0.1:{}", 9090);
            let client = clash::ClashClient::new(api_url);
            let _ = client.shutdown_core().await;
            return Ok(());
        }
    };

    println!("正在停止 clash-tui (PID: {})...", existing.pid);

    // 2. 先停止 clash-rs
    let api_url = format!("http://127.0.0.1:{}", 9090);
    let client = clash::ClashClient::new(api_url);
    match client.shutdown_core().await {
        Ok(_) => println!("clash-rs 已停止"),
        Err(e) => log::warn!("停止 clash-rs 失败: {}", e),
    }

    // 3. 发送终止信号给 clash-tui 进程
    #[cfg(unix)]
    {
        use std::process::Command;
        let result = Command::new("kill")
            .args(["-TERM", &existing.pid.to_string()])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                println!("已发送退出信号给 clash-tui");
            }
            _ => {
                eprintln!("发送退出信号失败，尝试强制终止...");
                let _ = Command::new("kill")
                    .args(["-9", &existing.pid.to_string()])
                    .output();
            }
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        let result = Command::new("taskkill")
            .args(&["/PID", &existing.pid.to_string(), "/F"])
            .output();

        match result {
            Ok(output) if output.status.success() => {
                println!("clash-tui 已终止");
            }
            Err(e) => {
                eprintln!("终止进程失败: {}", e);
            }
            _ => {}
        }
    }

    // 4. 清理实例文件
    instance_mgr.unregister();

    println!("clash-tui 已完全退出");
    Ok(())
}
