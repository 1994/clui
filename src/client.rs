use crate::state::*;
use anyhow::{Context, Result, anyhow};
use reqwest::header::USER_AGENT;
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {action} failed with HTTP {status} - {detail}")]
    Api {
        action: String,
        status: reqwest::StatusCode,
        detail: String,
    },
    #[error("Parse error: {0}")]
    Parse(String),
    #[allow(dead_code)]
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub struct ClashClient {
    client: reqwest::Client,
    base_url: String,
}

impl ClashClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    async fn ensure_success(response: reqwest::Response, action: &str) -> Result<(), ApiError> {
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let detail = response.text().await.unwrap_or_default();
        Err(ApiError::Api {
            action: action.to_string(),
            status,
            detail: detail.trim().to_string(),
        })
    }

    // ==================== Version ====================

    pub async fn get_version(&self) -> Result<Version, ApiError> {
        let data = self.get_json("/version", "get version").await?;
        serde_json::from_value(data)
            .map_err(|e| ApiError::Parse(format!("get version response: {e}")))
    }

    // ==================== Memory ====================

    pub async fn get_memory(&self) -> Result<Memory, ApiError> {
        let data = self.get_json("/memory", "get memory").await?;
        serde_json::from_value(data)
            .map_err(|e| ApiError::Parse(format!("get memory response: {e}")))
    }

    // ==================== Proxies ====================

    pub async fn get_proxies(&self) -> Result<Vec<Proxy>, ApiError> {
        let data = self.get_json("/proxies", "get proxies").await?;
        let data = Self::parse_proxies_response(data)?;
        let mut proxies: Vec<Proxy> = data
            .proxies
            .into_iter()
            .filter(|(_, p)| p.all.is_some())
            .map(|(_, p)| p)
            .collect();
        proxies.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(proxies)
    }

    pub async fn get_all_proxies(&self) -> Result<HashMap<String, Proxy>, ApiError> {
        let data = self.get_json("/proxies", "get all proxies").await?;
        let data = Self::parse_proxies_response(data)?;
        Ok(data.proxies)
    }

    pub async fn test_proxy_delay(
        &self,
        name: &str,
        url: Option<&str>,
        timeout: Option<u32>,
    ) -> Result<u32, ApiError> {
        let endpoint = self.url(&format!("/proxies/{}/delay", name));
        let test_url = url.unwrap_or("http://www.gstatic.com/generate_204");
        let test_timeout = timeout.unwrap_or(5000);
        let resp = self
            .client
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

    pub async fn switch_proxy(&self, selector: &str, proxy: &str) -> Result<(), ApiError> {
        let url = self.url(&format!("/proxies/{}", selector));
        let body = serde_json::json!({ "name": proxy });
        let resp = self.client.put(&url).json(&body).send().await?;
        Self::ensure_success(resp, "switch proxy").await
    }

    // ==================== Providers ====================

    pub async fn get_providers(&self) -> Result<Vec<Provider>, ApiError> {
        let data = self.get_json("/providers/proxies", "get providers").await?;
        let providers_obj = data
            .get("providers")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ApiError::Parse("missing providers field".to_string()))?;
        let mut providers: Vec<Provider> = providers_obj
            .iter()
            .map(|(name, value)| Self::parse_provider(name, value))
            .collect();
        providers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(providers)
    }

    async fn get_json(&self, path: &str, action: &str) -> Result<serde_json::Value, ApiError> {
        let resp = self.client.get(self.url(path)).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(ApiError::Api {
                action: action.to_string(),
                status,
                detail: body.trim().to_string(),
            });
        }
        serde_json::from_str(&body).map_err(|e| ApiError::Parse(format!("{action} response: {e}")))
    }

    fn parse_proxies_response(value: serde_json::Value) -> Result<ProxiesResponse, ApiError> {
        let proxies_obj = value
            .get("proxies")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ApiError::Parse("missing proxies field".to_string()))?;

        let proxies = proxies_obj
            .iter()
            .map(|(name, value)| (name.clone(), Self::parse_proxy(name, value)))
            .collect();

        Ok(ProxiesResponse { proxies })
    }

    fn parse_proxy(name: &str, value: &serde_json::Value) -> Proxy {
        let proxy_name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string();
        let proxy_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let now = value
            .get("now")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let all = value.get("all").and_then(|v| v.as_array()).map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        });
        let history = value
            .get("history")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Proxy {
            name: proxy_name,
            proxy_type,
            now,
            all,
            history,
            udp: value.get("udp").and_then(|v| v.as_bool()),
            xudp: value.get("xudp").and_then(|v| v.as_bool()),
            tfo: value.get("tfo").and_then(|v| v.as_bool()),
            mptcp: value.get("mptcp").and_then(|v| v.as_bool()),
            icon: value
                .get("icon")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }
    }

    pub async fn get_provider_proxy_counts(&self) -> Result<HashMap<String, usize>, ApiError> {
        let resp = self
            .client
            .get(self.url("/providers/proxies"))
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(ApiError::Api {
                action: "get provider proxy counts".to_string(),
                status,
                detail: body.trim().to_string(),
            });
        }
        let data: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| ApiError::Parse(format!("providers response: {e}")))?;
        let providers_obj = data
            .get("providers")
            .and_then(|v| v.as_object())
            .ok_or_else(|| ApiError::Parse("missing providers field".to_string()))?;

        Ok(providers_obj
            .iter()
            .map(|(name, value)| {
                let count = value
                    .get("proxies")
                    .and_then(serde_json::Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0);
                (name.clone(), count)
            })
            .collect())
    }

    pub async fn health_check_provider(&self, name: &str) -> Result<(), ApiError> {
        let url = self.url(&format!("/providers/proxies/{}/healthcheck", name));
        let resp = self.client.get(&url).send().await?;
        Self::ensure_success(resp, &format!("health check provider {}", name)).await
    }

    pub async fn update_provider_proxy(&self, name: &str) -> Result<(), ApiError> {
        let url = self.url(&format!("/providers/proxies/{}", name));
        let resp = self.client.put(&url).send().await?;
        Self::ensure_success(resp, &format!("update provider {}", name)).await
    }

    fn parse_provider(name: &str, value: &serde_json::Value) -> Provider {
        let provider_name = value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string();
        let provider_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let vehicle_type = value
            .get("vehicleType")
            .or_else(|| value.get("vehicle_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let proxies = value
            .get("proxies")
            .and_then(|p| p.as_array())
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(|(idx, proxy)| Self::parse_proxy(&format!("{name}#{idx}"), proxy))
                    .collect()
            })
            .unwrap_or_default();
        let updated_at = value
            .get("updatedAt")
            .or_else(|| value.get("updated_at"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let path = value
            .get("path")
            .and_then(|v| v.as_str())
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

    fn parse_subscription_info(value: Option<&serde_json::Value>) -> Option<SubscriptionInfo> {
        let value = value?;
        let object = value.as_object()?;
        let read = |keys: &[&str]| {
            keys.iter().find_map(|key| object.get(*key)).and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
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
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let url = object
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let interval = object
            .get("interval")
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(0) as u32;
        Some(HealthCheck {
            enable,
            url,
            interval,
        })
    }

    // ==================== Connections ====================

    pub async fn get_connections(&self) -> Result<ConnectionsResponse, ApiError> {
        let data = self.get_json("/connections", "get connections").await?;
        serde_json::from_value(data)
            .map_err(|e| ApiError::Parse(format!("get connections response: {e}")))
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), ApiError> {
        let url = self.url(&format!("/connections/{}", id));
        let resp = self.client.delete(&url).send().await?;
        Self::ensure_success(resp, "close connection").await
    }

    pub async fn close_all_connections(&self) -> Result<(), ApiError> {
        let resp = self.client.delete(self.url("/connections")).send().await?;
        Self::ensure_success(resp, "close all connections").await
    }

    // ==================== Rules ====================

    pub async fn get_rules(&self) -> Result<Vec<Rule>, ApiError> {
        let data = self.get_json("/rules", "get rules").await?;
        let data: RulesResponse = serde_json::from_value(data)
            .map_err(|e| ApiError::Parse(format!("get rules response: {e}")))?;
        Ok(data.rules)
    }

    // ==================== Config ====================

    pub async fn get_config(&self) -> Result<Config, ApiError> {
        let data = self.get_json("/configs", "get config").await?;
        let data: ConfigData = serde_json::from_value(data)
            .map_err(|e| ApiError::Parse(format!("get config response: {e}")))?;
        Ok(data.config)
    }

    pub async fn reload_config(&self, config_path: &str) -> Result<(), ApiError> {
        let url = self.url("/configs");
        let body = serde_json::json!({ "path": config_path });
        let resp = self.client.put(&url).json(&body).send().await?;
        Self::ensure_success(resp, "reload config").await
    }

    pub async fn change_mode(&self, mode: &str) -> Result<(), ApiError> {
        let url = self.url("/configs");
        let body = serde_json::json!({ "mode": mode });
        let resp = self.client.patch(&url).json(&body).send().await?;
        Self::ensure_success(resp, "change mode").await
    }

    // ==================== Subscription download ====================

    pub async fn fetch_subscription(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .header(USER_AGENT, "clash")
            .send()
            .await
            .context("download subscription")?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("read subscription response")?;
        if !status.is_success() {
            return Err(anyhow!(
                "download subscription failed: HTTP {status} - {}",
                truncate_chars(&body, 512)
            ));
        }
        Ok(body)
    }

    // ==================== System ====================

    pub async fn shutdown_core(&self) -> Result<(), ApiError> {
        let url = self.url("/stop");
        let _ = self.client.post(&url).send().await;
        Ok(())
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}
