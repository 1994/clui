use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Overview = 0,
    Proxies = 1,
    Providers = 2,
    Connections = 3,
    Rules = 4,
    Logs = 5,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "概览",
            Tab::Proxies => "代理",
            Tab::Providers => "订阅",
            Tab::Connections => "连接",
            Tab::Rules => "规则",
            Tab::Logs => "日志",
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Tab::Overview,
            1 => Tab::Proxies,
            2 => Tab::Providers,
            3 => Tab::Connections,
            4 => Tab::Rules,
            _ => Tab::Logs,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Popup {
    None,
    AddProvider,
    EditProvider,
    DeleteConfirm,
    ProxyNodes { proxy_name: String, selected: usize },
}

// ========== Format Helpers ==========

pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(1024.0).min(units.len() as f64 - 1.0) as usize;
    let value = bytes as f64 / 1024f64.powi(i as i32);
    if i == 0 {
        format!("{} {}", bytes, units[i])
    } else {
        format!("{:.2} {}", value, units[i])
    }
}

pub fn format_duration(s: &str) -> String {
    if s.is_empty() {
        return "-".to_string();
    }
    if let Ok(ts) = s.parse::<i64>() {
        let now = chrono::Local::now().timestamp();
        let diff = now - ts;
        if diff < 60 {
            "刚刚".to_string()
        } else if diff < 3600 {
            format!("{}分钟前", diff / 60)
        } else if diff < 86400 {
            format!("{}小时前", diff / 3600)
        } else {
            format!("{}天前", diff / 86400)
        }
    } else {
        s.to_string()
    }
}

// ========== Color Helpers ==========

pub fn delay_color(delay: u32) -> Color {
    match delay {
        0 => Color::Gray,
        1..=200 => Color::Green,
        201..=500 => Color::Yellow,
        501..=1000 => Color::Rgb(255, 140, 0), // dark orange
        _ => Color::Red,
    }
}

pub fn rule_type_color(type_name: &str) -> Color {
    match type_name.to_lowercase().as_str() {
        "domain" | "domain-suffix" | "domain-keyword" => Color::Cyan,
        "ip-cidr" | "ip-cidr6" | "geoip" => Color::Blue,
        "src-ip-cidr" => Color::Magenta,
        "dst-port" | "src-port" => Color::Yellow,
        "process-name" | "process-path" => Color::Green,
        "match" | "final" => Color::Red,
        _ => Color::Gray,
    }
}

pub fn proxy_color(type_name: &str) -> Color {
    match type_name.to_lowercase().as_str() {
        "selector" => Color::Cyan,
        "url-test" => Color::Green,
        "fallback" => Color::Yellow,
        "load-balance" => Color::Magenta,
        "relay" => Color::Blue,
        "direct" => Color::Gray,
        "reject" => Color::Red,
        _ => Color::White,
    }
}
