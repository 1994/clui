use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ==================== Version & System ====================

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    #[serde(default)]
    pub version: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub premium: bool,
    #[serde(rename = "Meta", alias = "meta")]
    #[serde(default)]
    pub meta: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Memory {
    #[serde(default)]
    pub inuse: u64,
    #[allow(dead_code)]
    #[serde(rename = "oslimit")]
    #[serde(default)]
    pub os_limit: u64,
}

// ==================== Proxy ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Proxy {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub proxy_type: String,
    #[serde(default)]
    pub now: Option<String>,
    #[serde(default)]
    pub all: Option<Vec<String>>,
    #[serde(default)]
    pub history: Vec<ProxyHistory>,
    #[allow(dead_code)]
    #[serde(default)]
    pub udp: Option<bool>,
    #[allow(dead_code)]
    #[serde(default)]
    pub xudp: Option<bool>,
    #[allow(dead_code)]
    #[serde(default)]
    pub tfo: Option<bool>,
    #[allow(dead_code)]
    #[serde(default)]
    pub mptcp: Option<bool>,
    #[allow(dead_code)]
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProxyHistory {
    #[allow(dead_code)]
    #[serde(default)]
    pub time: String,
    #[serde(default)]
    pub delay: u32,
    #[serde(rename = "meanDelay")]
    #[allow(dead_code)]
    #[serde(default)]
    pub mean_delay: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxiesResponse {
    pub proxies: HashMap<String, Proxy>,
}

// ==================== Provider ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Provider {
    pub name: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub provider_type: String,
    pub vehicle_type: String,
    pub proxies: Vec<Proxy>,
    #[allow(dead_code)]
    pub updated_at: Option<String>,
    #[serde(rename = "subscriptionInfo")]
    pub subscription_info: Option<SubscriptionInfo>,
    #[allow(dead_code)]
    pub path: Option<String>,
    #[allow(dead_code)]
    pub health_check: Option<HealthCheck>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubscriptionInfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    #[allow(dead_code)]
    pub expire: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HealthCheck {
    #[allow(dead_code)]
    pub enable: bool,
    #[allow(dead_code)]
    pub url: String,
    #[allow(dead_code)]
    pub interval: u32,
}

// ==================== Connection ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConnectionsResponse {
    #[serde(rename = "downloadTotal", alias = "download_total", default)]
    pub download_total: u64,
    #[serde(rename = "uploadTotal", alias = "upload_total", default)]
    pub upload_total: u64,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[allow(dead_code)]
    #[serde(default)]
    pub memory: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Connection {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub metadata: ConnectionMetadata,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[allow(dead_code)]
    #[serde(default)]
    pub start: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub chains: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub rule: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub rule_payload: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub process_path: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub process: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub source_port: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConnectionMetadata {
    #[serde(default)]
    pub network: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    #[serde(default)]
    pub conn_type: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub source_ip: String,
    #[serde(default)]
    pub destination_ip: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub source_port: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub destination_port: String,
    #[serde(default)]
    pub host: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub dns_mode: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub process: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub process_path: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub inbound_ip: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub inbound_port: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub inbound_name: Option<String>,
}

// ==================== Rule ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RulesResponse {
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Rule {
    #[serde(rename = "type")]
    #[serde(default)]
    pub rule_type: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub proxy: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub size: Option<u32>,
}

// ==================== Config ====================

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(rename = "socks-port", alias = "socks_port", default)]
    pub socks_port: Option<u16>,
    #[serde(rename = "redir-port", alias = "redir_port", default)]
    pub redir_port: Option<u16>,
    #[serde(rename = "tproxy-port", alias = "tproxy_port", default)]
    pub tproxy_port: Option<u16>,
    #[serde(rename = "mixed-port", alias = "mixed_port", default)]
    pub mixed_port: Option<u16>,
    #[serde(default)]
    pub tun: Option<TunConfig>,
    #[serde(default)]
    pub mode: String,
    #[serde(rename = "log-level", alias = "log_level", default)]
    pub log_level: String,
    #[serde(rename = "allow-lan", alias = "allow_lan", default)]
    pub allow_lan: bool,
    #[serde(rename = "bind-address", alias = "bind_address", default)]
    pub bind_address: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TunConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub stack: String,
    #[serde(rename = "dns-hijack", alias = "dns_hijack", default)]
    pub dns_hijack: Vec<String>,
    #[serde(rename = "auto-route", alias = "auto_route", default)]
    pub auto_route: bool,
    #[serde(
        rename = "auto-detect-interface",
        alias = "auto_detect_interface",
        default
    )]
    pub auto_detect_interface: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigData {
    #[allow(dead_code)]
    #[serde(default)]
    pub path: String,
    #[serde(rename = "profile")]
    #[serde(default)]
    pub config: Config,
}

// ==================== Traffic ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
}

// ==================== App State ====================

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub version: Option<Version>,
    pub config: Option<Config>,
    pub proxy_config: Option<Config>,
    pub api_url: String,
    pub config_path: String,
    pub memory: Option<Memory>,
    pub proxies: Vec<Proxy>,
    pub all_proxies: HashMap<String, Proxy>,
    pub providers: Vec<Provider>,
    pub connections: Vec<Connection>,
    pub download_total: u64,
    pub upload_total: u64,
    pub rules: Vec<Rule>,
    pub traffic: Traffic,
    pub logs: Vec<String>,
}
