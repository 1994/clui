use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Clash configuration file structure
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ClashConfig {
    #[serde(rename = "mixed-port", skip_serializing_if = "Option::is_none")]
    pub mixed_port: Option<u16>,

    #[serde(
        rename = "external-controller",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_controller: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,

    #[serde(rename = "proxy-providers", skip_serializing_if = "Option::is_none")]
    pub proxy_providers: Option<HashMap<String, ProviderConfig>>,

    #[serde(rename = "proxy-groups", skip_serializing_if = "Option::is_none")]
    pub proxy_groups: Option<Vec<serde_yaml::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxies: Option<Vec<serde_yaml::Value>>,

    /// Other fields to preserve
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: String,

    pub url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(rename = "health-check", skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct HealthCheckConfig {
    pub enable: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Clone)]
pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let path = match config_path {
            Some(p) => p,
            None => Self::find_config_path()?,
        };

        Ok(Self { config_path: path })
    }

    fn find_config_path() -> Result<PathBuf> {
        // Try common locations
        let possible_paths = [
            // Current directory
            PathBuf::from("config.yaml"),
            PathBuf::from("clash.yaml"),
            // Config directory
            dirs::config_dir()
                .map(|d| d.join("clash").join("config.yaml"))
                .unwrap_or_default(),
            dirs::config_dir()
                .map(|d| d.join("clash-meta").join("config.yaml"))
                .unwrap_or_default(),
            dirs::config_dir()
                .map(|d| d.join("mihomo").join("config.yaml"))
                .unwrap_or_default(),
            // Home directory
            dirs::home_dir()
                .map(|d| d.join(".config").join("clash").join("config.yaml"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|d| d.join(".config").join("clash-meta").join("config.yaml"))
                .unwrap_or_default(),
        ];

        for path in &possible_paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Return first option as default, will create new
        Ok(possible_paths[0].clone())
    }

    pub fn load(&self) -> Result<ClashConfig> {
        if !self.config_path.exists() {
            return Ok(ClashConfig::default());
        }

        let content = std::fs::read_to_string(&self.config_path)
            .with_context(|| format!("Failed to read config from {:?}", self.config_path))?;

        let config: ClashConfig =
            serde_yaml::from_str(&content).with_context(|| "Failed to parse YAML config")?;

        Ok(config)
    }

    pub fn save(&self, config: &ClashConfig) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content =
            serde_yaml::to_string(config).with_context(|| "Failed to serialize config to YAML")?;

        std::fs::write(&self.config_path, content)
            .with_context(|| format!("Failed to write config to {:?}", self.config_path))?;

        Ok(())
    }

    pub fn add_provider(
        &self,
        name: &str,
        url: &str,
        interval: u32,
        health_check: bool,
    ) -> Result<()> {
        let mut config = self.load()?;

        let providers = config.proxy_providers.get_or_insert_with(HashMap::new);

        let health_check_config = if health_check {
            Some(HealthCheckConfig {
                enable: true,
                interval: Some(600),
                url: Some("http://www.gstatic.com/generate_204".to_string()),
            })
        } else {
            None
        };

        let provider = ProviderConfig {
            provider_type: "http".to_string(),
            url: url.to_string(),
            interval: Some(interval),
            path: Some(format!("./providers/{}.yaml", name)),
            health_check: health_check_config,
            extra: HashMap::new(),
        };

        providers.insert(name.to_string(), provider);
        self.save(&config)?;

        Ok(())
    }

    pub fn remove_provider(&self, name: &str) -> Result<()> {
        let mut config = self.load()?;

        if let Some(ref mut providers) = config.proxy_providers {
            providers.remove(name);
        }

        self.save(&config)?;
        Ok(())
    }

    pub fn update_provider(
        &self,
        name: &str,
        url: Option<&str>,
        interval: Option<u32>,
    ) -> Result<()> {
        let mut config = self.load()?;

        if let Some(ref mut providers) = config.proxy_providers
            && let Some(provider) = providers.get_mut(name)
        {
            if let Some(u) = url {
                provider.url = u.to_string();
            }
            if let Some(i) = interval {
                provider.interval = Some(i);
            }
        }

        self.save(&config)?;
        Ok(())
    }

    pub fn get_providers(&self) -> Result<HashMap<String, ProviderConfig>> {
        let config = self.load()?;
        Ok(config.proxy_providers.unwrap_or_default())
    }

    pub fn get_config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
mixed-port: 7890
external-controller: 127.0.0.1:9090
mode: rule

proxy-providers:
  my-sub:
    type: http
    url: https://example.com/sub
    interval: 86400
    path: ./providers/my-sub.yaml
    health-check:
      enable: true
      interval: 600
"#;

        let config: ClashConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.mixed_port, Some(7890));
        assert!(config.proxy_providers.is_some());
    }
}
