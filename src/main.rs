mod app;
mod args;
mod client;
mod commands;
mod config;
mod context;
mod core_manager;
mod event;
mod state;
mod tabs;
mod terminal;
mod ui;
mod updater;

use anyhow::Result;
use args::{Cli, Commands};
use std::path::PathBuf;

fn init_logging(log_level: &str, tui_mode: bool) {
    let log_dir = dirs::config_dir()
        .map(|d| d.join("clash-tui").join("logs"))
        .unwrap_or_else(|| PathBuf::from("logs"));
    let _ = std::fs::create_dir_all(&log_dir);

    let seven_days_ago = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(7 * 24 * 60 * 60))
        .unwrap_or_else(std::time::SystemTime::now);
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && let Ok(modified) = meta.modified()
                && modified < seven_days_ago
                && meta.is_file()
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    let log_file_path = log_dir.join("clash-tui.log");
    let default_filter = if log_level == "info" {
        "warn,clash_tui=info".to_string()
    } else {
        log_level.to_string()
    };
    let env = env_logger::Env::default().default_filter_or(default_filter);
    let mut builder = env_logger::Builder::from_env(env);
    builder.format_timestamp_secs();

    if tui_mode {
        let target = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .or_else(|_| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(std::env::temp_dir().join("clash-tui.log"))
            });
        match target {
            Ok(file) => {
                builder.target(env_logger::Target::Pipe(Box::new(file)));
            }
            Err(_) => {
                builder.target(env_logger::Target::Pipe(Box::new(std::io::sink())));
            }
        }
    }
    builder.init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse_args();
    let is_tui_mode = cli.command.is_none() || matches!(cli.command, Some(Commands::Tui));
    init_logging(&cli.log_level, is_tui_mode);
    terminal::setup_panic_hook();

    match cli.command {
        None | Some(Commands::Tui) => {
            run_tui(cli.config).await?;
        }
        Some(Commands::Daemon) => {
            run_daemon(cli.config).await?;
        }
        Some(Commands::Stop) => {
            let config_manager = config::ConfigManager::new(cli.config)?;
            let client = client::ClashClient::new(config_manager.get_api_url());
            client.shutdown_core().await?;
            println!("Clash core stopped");
        }
        Some(Commands::Restart) => {
            let config_manager = config::ConfigManager::new(cli.config)?;
            let client = client::ClashClient::new(config_manager.get_api_url());
            client.shutdown_core().await.ok();
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            println!("Clash core restart signal sent");
        }
        Some(Commands::Status) => {
            let config_manager = config::ConfigManager::new(cli.config)?;
            let client = client::ClashClient::new(config_manager.get_api_url());
            match client.get_version().await {
                Ok(v) => {
                    println!("Status: Running");
                    println!("Version: {}", v.version);
                    println!("API: {}", config_manager.get_api_url());
                }
                Err(e) => {
                    println!("Status: Not running or unreachable");
                    println!("Error: {}", e);
                    match core_manager::bundled_core_path() {
                        Ok(path) => println!("Bundled core: {}", path.display()),
                        Err(core_err) => println!("Bundled core: {}", core_err),
                    }
                }
            }
        }
        Some(Commands::Quit) => {
            let config_manager = config::ConfigManager::new(cli.config)?;
            let client = client::ClashClient::new(config_manager.get_api_url());
            client.shutdown_core().await?;
            println!("Clash core stopped");
        }
    }

    Ok(())
}

async fn run_tui(config_path: Option<PathBuf>) -> Result<()> {
    let mut guard = terminal::TerminalGuard::new()?;
    let mut app = app::App::new(config_path)?;
    guard.terminal().draw(|f| {
        crate::tabs::render(
            f,
            &state::AppState::default(),
            &mut context::UiState::default(),
        );
    })?;
    app.initialize().await?;
    guard.terminal().clear()?;
    app.run(guard.terminal()).await?;
    Ok(())
}

async fn run_daemon(config_path: Option<PathBuf>) -> Result<()> {
    use tokio::sync::mpsc;

    let config_manager = config::ConfigManager::new(config_path)?;
    let config_path = config_manager.config_path().clone();
    core_manager::ensure_config(&config_path)?;
    let _ = config_manager.repair_for_tui();
    let _ = config_manager.repair_provider_caches();

    let api_url = config_manager.get_api_url();
    let client = client::ClashClient::new(api_url.clone());
    if core_manager::is_core_running(&api_url).await {
        client
            .reload_config(&config_path.to_string_lossy())
            .await
            .ok();
    } else {
        core_manager::start_core(&config_path).await?;
        wait_for_core(&api_url).await?;
    }

    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let updater = updater::Updater::new(client, config_manager, event_tx);
    updater.load_tasks().await?;
    let updater = std::sync::Arc::new(updater);
    let updater_clone = updater.clone();

    tokio::spawn(async move {
        updater_clone.run().await;
    });

    println!("clash-tui daemon running...");
    println!("API: {}", api_url);

    while let Some(event) = event_rx.recv().await {
        match event {
            updater::UpdateEvent::Started(name) => {
                log::info!("[daemon] Updating provider '{}'", name);
            }
            updater::UpdateEvent::Completed(name, success, error) => {
                if success {
                    log::info!("[daemon] Provider '{}' updated successfully", name);
                } else {
                    log::error!("[daemon] Provider '{}' update failed: {:?}", name, error);
                }
            }
            updater::UpdateEvent::Progress(name, msg) => {
                log::info!("[daemon] Provider '{}' - {}", name, msg);
            }
        }
    }

    Ok(())
}

async fn wait_for_core(api_url: &str) -> Result<()> {
    for _ in 0..30 {
        if core_manager::is_core_running(api_url).await {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    anyhow::bail!("Mihomo core did not become ready at {}", api_url)
}
