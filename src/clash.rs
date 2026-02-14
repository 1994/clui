use anyhow::{Context, Result, anyhow};
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ClashClient {
    base_url: String,
}

impl ClashClient {
    fn build_subscription_request(client: &reqwest::Client, url: &str) -> Result<reqwest::Request> {
        client
            .post(url)
            .header(USER_AGENT, "clash")
            .build()
            .with_context(|| format!("构建订阅请求失败: {}", url))
    }

    /// 使用 clash 核心的 User-Agent 下载订阅内容
    pub async fn fetch_subscription(&self, url: &str) -> Result<String> {
        let client = reqwest::Client::new();
        let request = Self::build_subscription_request(&client, url)?;
        let response = client
            .execute(request)
            .await
            .context(format!("下载订阅失败: {}", url))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context(format!("读取订阅响应失败: {}", url))?;

        if !status.is_success() {
            return Err(anyhow!("下载订阅失败: HTTP {} - {}", status, body));
        }

        Ok(body)
    }
}

// ==================== Version & System ====================

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub version: String,
    #[allow(dead_code)]
    pub premium: bool,
    #[serde(rename = "Meta")]
    pub meta: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Memory {
    pub inuse: u64,
    #[serde(rename = "oslimit")]
    pub os_limit: u64,
}

// ==================== Proxy ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Proxy {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub now: Option<String>,
    pub all: Option<Vec<String>>,
    pub history: Vec<ProxyHistory>,
    // Meta extensions
    pub udp: Option<bool>,
    pub xudp: Option<bool>,
    pub tfo: Option<bool>,
    pub mptcp: Option<bool>,
    #[allow(dead_code)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyHistory {
    #[allow(dead_code)]
    pub time: String,
    pub delay: u32,
    #[serde(rename = "meanDelay")]
    #[allow(dead_code)]
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
    pub health_check: Option<HealthCheck>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SubscriptionInfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HealthCheck {
    pub enable: bool,
    pub url: String,
    pub interval: u32,
}

// ==================== Connection ====================

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionsResponse {
    pub download_total: u64,
    pub upload_total: u64,
    pub connections: Vec<Connection>,
    #[allow(dead_code)]
    pub memory: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Connection {
    pub id: String,
    pub metadata: ConnectionMetadata,
    pub upload: u64,
    pub download: u64,
    #[allow(dead_code)]
    pub start: String,
    #[allow(dead_code)]
    pub chains: Vec<String>,
    #[allow(dead_code)]
    pub rule: String,
    #[allow(dead_code)]
    pub rule_payload: String,
    // Meta extensions
    #[allow(dead_code)]
    pub process_path: Option<String>,
    #[allow(dead_code)]
    pub process: Option<String>,
    #[allow(dead_code)]
    pub source_port: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConnectionMetadata {
    pub network: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub conn_type: String,
    #[allow(dead_code)]
    pub source_ip: String,
    pub destination_ip: Option<String>,
    #[allow(dead_code)]
    pub source_port: String,
    pub destination_port: String,
    pub host: Option<String>,
    #[allow(dead_code)]
    pub dns_mode: Option<String>,
    #[allow(dead_code)]
    pub process: Option<String>,
    #[allow(dead_code)]
    pub process_path: Option<String>,
    #[allow(dead_code)]
    pub inbound_ip: Option<String>,
    #[allow(dead_code)]
    pub inbound_port: Option<String>,
    #[allow(dead_code)]
    pub inbound_name: Option<String>,
}

// ==================== Rule ====================

#[derive(Debug, Clone, Deserialize)]
pub struct RulesResponse {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Rule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub size: Option<u32>,
}

// ==================== Config ====================

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    pub port: Option<u16>,
    pub socks_port: Option<u16>,
    pub redir_port: Option<u16>,
    pub tproxy_port: Option<u16>,
    pub mixed_port: Option<u16>,
    pub tun: Option<TunConfig>,
    pub mode: String,
    pub log_level: String,
    pub allow_lan: bool,
    pub bind_address: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TunConfig {
    pub enable: bool,
    pub device: String,
    pub stack: String,
    pub dns_hijack: Vec<String>,
    pub auto_route: bool,
    pub auto_detect_interface: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigData {
    #[allow(dead_code)]
    pub path: String,
    #[serde(rename = "profile")]
    pub config: Config,
}

// ==================== Traffic ====================

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
}

// ==================== Group ====================

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct Group {
    pub name: String,
    #[serde(rename = "type")]
    pub group_type: String,
    pub now: String,
    pub all: Vec<String>,
}

// ==================== Experimental ====================

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct Cache {
    pub key: String,
    pub value: String,
    pub expire: u64,
}

impl ClashClient {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    async fn ensure_success_response(response: reqwest::Response, action: &str) -> Result<()> {
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }

        let body = response.text().await.unwrap_or_default();
        let detail = body.trim();
        if detail.is_empty() {
            Err(anyhow!("{} 失败: HTTP {}", action, status))
        } else {
            Err(anyhow!("{} 失败: HTTP {} - {}", action, status, detail))
        }
    }

    fn parse_u64_or_str(value: &serde_json::Value) -> Option<u64> {
        if let Some(v) = value.as_u64() {
            return Some(v);
        }
        value.as_str().and_then(|v| v.parse::<u64>().ok())
    }

    fn parse_u32_or_str(value: &serde_json::Value) -> Option<u32> {
        Self::parse_u64_or_str(value).and_then(|v| u32::try_from(v).ok())
    }

    fn parse_subscription_info(value: Option<&serde_json::Value>) -> Option<SubscriptionInfo> {
        let value = value?;
        let object = value.as_object()?;
        let read = |keys: &[&str]| {
            keys.iter()
                .find_map(|key| object.get(*key))
                .and_then(Self::parse_u64_or_str)
        };

        let upload = read(&["upload", "Upload"])?;
        let download = read(&["download", "Download"])?;
        let total = read(&["total", "Total"])?;
        let expire = read(&["expire", "Expire"]).unwrap_or(0);

        Some(SubscriptionInfo {
            upload,
            download,
            total,
            expire,
        })
    }

    fn parse_health_check(value: Option<&serde_json::Value>) -> Option<HealthCheck> {
        let value = value?;
        let object = value.as_object()?;

        let enable = object
            .get("enable")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let url = object
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let interval = object
            .get("interval")
            .and_then(Self::parse_u32_or_str)
            .unwrap_or(0);

        Some(HealthCheck {
            enable,
            url,
            interval,
        })
    }

    fn parse_provider(name: &str, value: &serde_json::Value) -> Provider {
        let provider_name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(name)
            .to_string();
        let provider_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let vehicle_type = value
            .get("vehicleType")
            .or_else(|| value.get("vehicle_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let proxies = value
            .get("proxies")
            .cloned()
            .and_then(|proxies| serde_json::from_value::<Vec<Proxy>>(proxies).ok())
            .unwrap_or_default();
        let updated_at = value
            .get("updatedAt")
            .or_else(|| value.get("updated_at"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let path = value
            .get("path")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let subscription_info = Self::parse_subscription_info(
            value
                .get("subscriptionInfo")
                .or_else(|| value.get("subscription_info")),
        );
        let health_check = Self::parse_health_check(
            value
                .get("healthCheck")
                .or_else(|| value.get("health_check")),
        );

        Provider {
            name: provider_name,
            provider_type,
            vehicle_type,
            proxies,
            updated_at,
            subscription_info,
            path,
            health_check,
        }
    }

    fn providers_base_url(&self) -> Result<reqwest::Url> {
        let base = format!("{}/providers/proxies", self.base_url.trim_end_matches('/'));
        reqwest::Url::parse(&base).map_err(|e| anyhow!("构造订阅 API 地址失败: {}", e))
    }

    fn provider_endpoint_url(&self, name: &str) -> Result<reqwest::Url> {
        let mut url = self.providers_base_url()?;
        url.path_segments_mut()
            .map_err(|_| anyhow!("构造订阅 API 地址失败: base URL 不支持路径拼接"))?
            .push(name);
        Ok(url)
    }

    fn provider_healthcheck_endpoint_url(&self, name: &str) -> Result<reqwest::Url> {
        let mut url = self.provider_endpoint_url(name)?;
        url.path_segments_mut()
            .map_err(|_| anyhow!("构造订阅健康检查 API 地址失败: base URL 不支持路径拼接"))?
            .push("healthcheck");
        Ok(url)
    }

    // ==================== Version ====================

    pub async fn get_version(&self) -> Result<Version> {
        let url = format!("{}/version", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let version = resp.json::<Version>().await?;
        Ok(version)
    }

    // ==================== Memory (Meta Only) ====================

    pub async fn get_memory(&self) -> Result<Memory> {
        let url = format!("{}/memory", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let memory = resp.json::<Memory>().await?;
        Ok(memory)
    }

    // ==================== Proxies ====================

    pub async fn get_proxies(&self) -> Result<Vec<Proxy>> {
        let url = format!("{}/proxies", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let data = resp.json::<ProxiesResponse>().await?;

        let mut proxies: Vec<Proxy> = data
            .proxies
            .into_iter()
            .filter(|(_, p)| p.all.is_some())
            .map(|(_, p)| p)
            .collect();

        proxies.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(proxies)
    }

    pub async fn get_all_proxies(&self) -> Result<HashMap<String, Proxy>> {
        let url = format!("{}/proxies", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let data = resp.json::<ProxiesResponse>().await?;
        Ok(data.proxies)
    }

    #[allow(dead_code)]
    pub async fn get_proxy(&self, name: &str) -> Result<Proxy> {
        let url = format!("{}/proxies/{}", self.base_url, name);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let proxy = resp.json::<Proxy>().await?;
        Ok(proxy)
    }

    pub async fn test_proxy_delay(
        &self,
        name: &str,
        url: Option<&str>,
        timeout: Option<u32>,
    ) -> Result<u32> {
        let endpoint = format!("{}/proxies/{}/delay", self.base_url, name);
        let client = reqwest::Client::new();

        let test_url = url.unwrap_or("http://www.gstatic.com/generate_204");
        let test_timeout = timeout.unwrap_or(5000);

        let resp = client
            .get(&endpoint)
            .query(&[
                ("timeout", test_timeout.to_string()),
                ("url", test_url.to_string()),
            ])
            .send()
            .await?;
        let result: serde_json::Value = resp.json().await?;
        Ok(result["delay"].as_u64().unwrap_or(0) as u32)
    }

    #[allow(dead_code)]
    pub async fn test_all_proxy_delay(
        &self,
        url: Option<&str>,
        timeout: Option<u32>,
    ) -> Result<HashMap<String, u32>> {
        let _endpoint = format!("{}/group", self.base_url);
        let _client = reqwest::Client::new();

        let test_url = url.unwrap_or("http://www.gstatic.com/generate_204");
        let test_timeout = timeout.unwrap_or(5000);

        let proxies = self.get_proxies().await?;
        let mut delays = HashMap::new();

        for proxy in proxies {
            if let Ok(delay) = self
                .test_proxy_delay(&proxy.name, Some(test_url), Some(test_timeout))
                .await
            {
                delays.insert(proxy.name, delay);
            }
        }

        Ok(delays)
    }

    pub async fn switch_proxy(&self, selector: &str, proxy: &str) -> Result<()> {
        let url = format!("{}/proxies/{}", self.base_url, selector);
        let body = serde_json::json!({ "name": proxy });
        let client = reqwest::Client::new();
        client.put(&url).json(&body).send().await?;
        Ok(())
    }

    // ==================== Providers ====================

    pub async fn get_providers(&self) -> Result<Vec<Provider>> {
        let url = format!("{}/providers/proxies", self.base_url);
        let response: reqwest::Response = reqwest::get(&url).await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let detail = body.trim();
            if detail.is_empty() {
                return Err(anyhow!("获取订阅列表失败: HTTP {}", status));
            }
            return Err(anyhow!("获取订阅列表失败: HTTP {} - {}", status, detail));
        }

        let body_preview: String = body.chars().take(240).collect();
        let data: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            anyhow!(
                "获取订阅列表失败: 解析响应失败: {} | 响应片段: {}",
                e,
                body_preview
            )
        })?;
        let providers_obj = data
            .get("providers")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| anyhow!("获取订阅列表失败: 响应缺少 providers 字段"))?;

        let mut providers: Vec<Provider> = providers_obj
            .iter()
            .map(|(name, value)| Self::parse_provider(name, value))
            .collect();

        providers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(providers)
    }

    #[allow(dead_code)]
    pub async fn get_provider(&self, name: &str) -> Result<Provider> {
        let url = self.provider_endpoint_url(name)?;
        let resp: reqwest::Response = reqwest::get(url).await?;
        let provider = resp.json::<Provider>().await?;
        Ok(provider)
    }

    pub async fn health_check_provider(&self, name: &str) -> Result<()> {
        let url = self.provider_healthcheck_endpoint_url(name)?;
        let client = reqwest::Client::new();
        let response = client.get(url).send().await?;
        Self::ensure_success_response(response, &format!("健康检查订阅 '{}'", name)).await
    }

    #[allow(dead_code)]
    pub async fn test_proxy_in_provider(&self, provider: &str, proxy: &str) -> Result<u32> {
        let mut url = self.provider_endpoint_url(provider)?;
        {
            let mut segments = url.path_segments_mut().map_err(|_| {
                anyhow!("构造订阅节点健康检查 API 地址失败: base URL 不支持路径拼接")
            })?;
            segments.push(proxy);
            segments.push("healthcheck");
        }
        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .query(&[
                ("timeout", "5000"),
                ("url", "http://www.gstatic.com/generate_204"),
            ])
            .send()
            .await?;
        let result: serde_json::Value = resp.json().await?;
        Ok(result["delay"].as_u64().unwrap_or(0) as u32)
    }

    // ==================== Connections ====================

    pub async fn get_connections(&self) -> Result<ConnectionsResponse> {
        let url = format!("{}/connections", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let data = resp.json::<ConnectionsResponse>().await?;
        Ok(data)
    }

    pub async fn close_connection(&self, id: &str) -> Result<()> {
        let url = format!("{}/connections/{}", self.base_url, id);
        let client = reqwest::Client::new();
        client.delete(&url).send().await?;
        Ok(())
    }

    pub async fn close_all_connections(&self) -> Result<()> {
        let url = format!("{}/connections", self.base_url);
        let client = reqwest::Client::new();
        client.delete(&url).send().await?;
        Ok(())
    }

    // ==================== Rules ====================

    pub async fn get_rules(&self) -> Result<Vec<Rule>> {
        let url = format!("{}/rules", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let data = resp.json::<RulesResponse>().await?;
        Ok(data.rules)
    }

    // ==================== Traffic ====================

    pub async fn get_traffic(&self) -> Result<Traffic> {
        let url = format!("{}/traffic", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let traffic = resp.json::<Traffic>().await?;
        Ok(traffic)
    }

    // ==================== Config ====================

    pub async fn get_config(&self) -> Result<Config> {
        let url = format!("{}/configs", self.base_url);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let data = resp.json::<ConfigData>().await?;
        Ok(data.config)
    }

    #[allow(dead_code)]
    pub async fn update_config(&self, config: &Config) -> Result<()> {
        let url = format!("{}/configs", self.base_url);
        let client = reqwest::Client::new();
        client.patch(&url).json(config).send().await?;
        Ok(())
    }

    pub async fn reload_config(&self, force: bool) -> Result<()> {
        let url = format!("{}/configs?force={}", self.base_url, force);
        let client = reqwest::Client::new();
        let response = client.put(&url).json(&serde_json::json!({})).send().await?;
        Self::ensure_success_response(response, "重载配置").await
    }

    #[allow(dead_code)]
    pub async fn change_mode(&self, mode: &str) -> Result<()> {
        let url = format!("{}/configs", self.base_url);
        let body = serde_json::json!({ "mode": mode });
        let client = reqwest::Client::new();
        client.patch(&url).json(&body).send().await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn change_log_level(&self, level: &str) -> Result<()> {
        let url = format!("{}/configs", self.base_url);
        let body = serde_json::json!({ "log-level": level });
        let client = reqwest::Client::new();
        client.patch(&url).json(&body).send().await?;
        Ok(())
    }

    // ==================== Cache ====================

    #[allow(dead_code)]
    pub async fn clear_fake_ip_cache(&self) -> Result<()> {
        let url = format!("{}/cache/fakeip", self.base_url);
        let client = reqwest::Client::new();
        client.post(&url).send().await?;
        Ok(())
    }

    // ==================== System ====================

    pub async fn shutdown_core(&self) -> Result<()> {
        let url = format!("{}/stop", self.base_url);
        let client = reqwest::Client::new();
        let resp = client.post(&url).send().await;
        // clash-rs 收到 stop 后会直接退出，连接可能提前关闭
        match resp {
            Ok(_) | Err(_) => Ok(()), // 忽略错误，因为服务可能已经停止
        }
    }

    // ==================== DNS ====================

    #[allow(dead_code)]
    pub async fn query_dns(&self, domain: &str) -> Result<Vec<String>> {
        let url = format!("{}/dns/query?name={}", self.base_url, domain);
        let resp: reqwest::Response = reqwest::get(&url).await?;
        let result: serde_json::Value = resp.json().await?;

        let ips: Vec<String> = result["Answer"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v["data"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(ips)
    }
}

#[cfg(test)]
mod tests {
    use super::ClashClient;
    use reqwest::header::USER_AGENT;

    #[test]
    fn provider_endpoint_should_percent_encode_reserved_chars_in_name() {
        let client = ClashClient::new("http://127.0.0.1:9090".to_string());
        let url = client
            .provider_endpoint_url("sub?token=abc&flag=true")
            .expect("provider endpoint should be valid");

        assert_eq!(url.query(), None, "provider name should stay in path");
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/providers/proxies/sub%3Ftoken=abc&flag=true"
        );
    }

    #[test]
    fn provider_healthcheck_endpoint_should_append_healthcheck_path() {
        let client = ClashClient::new("http://127.0.0.1:9090".to_string());
        let url = client
            .provider_healthcheck_endpoint_url("a/b")
            .expect("healthcheck endpoint should be valid");

        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/providers/proxies/a%2Fb/healthcheck"
        );
    }

    #[test]
    fn build_subscription_request_should_use_post_and_clash_user_agent() {
        let client = reqwest::Client::new();
        let request = ClashClient::build_subscription_request(&client, "https://example.com/sub")
            .expect("request should be built");

        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|v| v.to_str().ok()),
            Some("clash")
        );
    }
}
