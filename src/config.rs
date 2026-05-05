use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ClashConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    #[serde(rename = "socks-port", skip_serializing_if = "Option::is_none")]
    pub socks_port: Option<u16>,

    #[serde(rename = "redir-port", skip_serializing_if = "Option::is_none")]
    pub redir_port: Option<u16>,

    #[serde(rename = "tproxy-port", skip_serializing_if = "Option::is_none")]
    pub tproxy_port: Option<u16>,

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

    #[serde(rename = "allow-lan", skip_serializing_if = "Option::is_none")]
    pub allow_lan: Option<bool>,

    #[serde(rename = "bind-address", skip_serializing_if = "Option::is_none")]
    pub bind_address: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,

    #[serde(rename = "proxy-providers", skip_serializing_if = "Option::is_none")]
    pub proxy_providers: Option<HashMap<String, ProviderConfig>>,

    #[serde(rename = "proxy-groups", skip_serializing_if = "Option::is_none")]
    pub proxy_groups: Option<Vec<serde_yaml::Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxies: Option<Vec<serde_yaml::Value>>,

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
    explicit_config: bool,
}

impl ConfigManager {
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let explicit_config = config_path.is_some();
        let path = match config_path {
            Some(p) => p,
            None => Self::find_config_path()?,
        };
        Ok(Self {
            config_path: path,
            explicit_config,
        })
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn migrate_legacy_local_config_if_needed(&self) -> Result<()> {
        self.migrate_legacy_config_from(&[
            PathBuf::from("config.yaml"),
            PathBuf::from("clash.yaml"),
        ])
    }

    fn migrate_legacy_config_from(&self, legacy_paths: &[PathBuf]) -> Result<()> {
        if self.explicit_config || self.config_path.exists() {
            return Ok(());
        }

        for legacy_path in legacy_paths {
            if !legacy_path.exists() || legacy_path == &self.config_path {
                continue;
            }
            if let Some(parent) = self.config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(legacy_path, &self.config_path).with_context(|| {
                format!(
                    "migrate legacy config from {:?} to {:?}",
                    legacy_path, self.config_path
                )
            })?;
            return Ok(());
        }

        Ok(())
    }

    fn find_config_path() -> Result<PathBuf> {
        let app_config = dirs::config_dir()
            .map(|d| d.join("clash-tui").join("config.yaml"))
            .unwrap_or_else(|| PathBuf::from("config.yaml"));
        let possible_paths = [
            app_config.clone(),
            dirs::config_dir()
                .map(|d| d.join("mihomo").join("config.yaml"))
                .unwrap_or_default(),
            dirs::config_dir()
                .map(|d| d.join("clash-meta").join("config.yaml"))
                .unwrap_or_default(),
            dirs::config_dir()
                .map(|d| d.join("clash").join("config.yaml"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|d| d.join(".config").join("mihomo").join("config.yaml"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|d| d.join(".config").join("clash-meta").join("config.yaml"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|d| d.join(".config").join("clash").join("config.yaml"))
                .unwrap_or_default(),
            PathBuf::from("config.yaml"),
            PathBuf::from("clash.yaml"),
        ];

        for path in &possible_paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        Ok(app_config)
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
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            serde_yaml::to_string(config).with_context(|| "Failed to serialize config to YAML")?;
        std::fs::write(&self.config_path, content)
            .with_context(|| format!("Failed to write config to {:?}", self.config_path))?;
        Ok(())
    }

    pub fn repair_for_tui(&self) -> Result<bool> {
        let mut config = self.load()?;
        let before = serde_yaml::to_string(&config)?;

        if let Some(providers) = config.proxy_providers.clone() {
            for name in providers.keys() {
                ensure_proxy_group_uses_provider(&mut config, name);
            }
        }
        ensure_default_route_uses_proxy(&mut config);

        let after = serde_yaml::to_string(&config)?;
        if before != after {
            self.save(&config)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn repair_provider_caches(&self) -> Result<usize> {
        let providers = self.get_providers()?;
        let mut repaired = 0;

        for (name, provider) in providers {
            let Some(path) = provider.path.as_deref() else {
                continue;
            };
            let cache_path = self.resolve_provider_path(path);
            if !cache_path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&cache_path)
                .with_context(|| format!("read provider cache {:?}", cache_path))?;
            match normalize_provider_cache(&content) {
                Ok(Some(normalized)) => {
                    std::fs::write(&cache_path, normalized)
                        .with_context(|| format!("write provider cache {:?}", cache_path))?;
                    repaired += 1;
                }
                Ok(None) => {}
                Err(e) => {
                    log::warn!("Skipping invalid provider cache '{}': {:#}", name, e);
                }
            }
        }

        Ok(repaired)
    }

    pub fn add_provider(&self, name: &str, url: &str, interval: u32) -> Result<()> {
        let mut config = self.load()?;
        let providers = config.proxy_providers.get_or_insert_with(HashMap::new);
        let provider = ProviderConfig {
            provider_type: "http".to_string(),
            url: url.to_string(),
            interval: Some(interval),
            path: Some(format!("./providers/{}.yaml", name)),
            health_check: Some(HealthCheckConfig {
                enable: true,
                interval: Some(600),
                url: Some("http://www.gstatic.com/generate_204".to_string()),
            }),
            extra: HashMap::new(),
        };
        providers.insert(name.to_string(), provider);
        ensure_proxy_group_uses_provider(&mut config, name);
        ensure_default_route_uses_proxy(&mut config);
        self.save(&config)
    }

    pub fn remove_provider(&self, name: &str) -> Result<()> {
        let mut config = self.load()?;
        if let Some(ref mut providers) = config.proxy_providers {
            providers.remove(name);
        }
        remove_provider_from_proxy_groups(&mut config, name);
        self.save(&config)
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
        self.save(&config)
    }

    pub fn get_providers(&self) -> Result<HashMap<String, ProviderConfig>> {
        let config = self.load()?;
        Ok(config.proxy_providers.unwrap_or_default())
    }

    pub fn get_api_url(&self) -> String {
        let config = self.load().ok();
        let ctrl = config
            .as_ref()
            .and_then(|c| c.external_controller.clone())
            .unwrap_or_else(|| "127.0.0.1:9090".to_string());
        format!("http://{}", ctrl)
    }

    pub fn resolve_provider_path(&self, path: &str) -> PathBuf {
        let provider_path = PathBuf::from(path);
        if provider_path.is_absolute() {
            provider_path
        } else {
            self.config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(provider_path)
        }
    }
}

fn yaml_key(key: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(key.to_string())
}

fn yaml_string(value: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(value.to_string())
}

fn ensure_proxy_group_uses_provider(config: &mut ClashConfig, provider_name: &str) {
    let groups = config.proxy_groups.get_or_insert_with(Vec::new);
    let provider_value = yaml_string(provider_name);

    for group in groups.iter_mut() {
        let Some(mapping) = group.as_mapping_mut() else {
            continue;
        };
        let name = mapping
            .get(yaml_key("name"))
            .and_then(serde_yaml::Value::as_str);
        if name != Some("Proxy") {
            continue;
        }

        let uses = mapping
            .entry(yaml_key("use"))
            .or_insert_with(|| serde_yaml::Value::Sequence(Vec::new()));
        if let Some(sequence) = uses.as_sequence_mut()
            && !sequence.iter().any(|value| value == &provider_value)
        {
            sequence.push(provider_value);
        }
        ensure_group_has_direct_fallback(mapping);
        return;
    }

    let mut group = serde_yaml::Mapping::new();
    group.insert(yaml_key("name"), yaml_string("Proxy"));
    group.insert(yaml_key("type"), yaml_string("select"));
    group.insert(
        yaml_key("proxies"),
        serde_yaml::Value::Sequence(vec![yaml_string("DIRECT")]),
    );
    group.insert(
        yaml_key("use"),
        serde_yaml::Value::Sequence(vec![provider_value]),
    );
    groups.push(serde_yaml::Value::Mapping(group));
}

fn ensure_group_has_direct_fallback(mapping: &mut serde_yaml::Mapping) {
    let direct = yaml_string("DIRECT");
    let proxies = mapping
        .entry(yaml_key("proxies"))
        .or_insert_with(|| serde_yaml::Value::Sequence(Vec::new()));
    if let Some(sequence) = proxies.as_sequence_mut()
        && !sequence.iter().any(|value| value == &direct)
    {
        sequence.push(direct);
    }
}

fn remove_provider_from_proxy_groups(config: &mut ClashConfig, provider_name: &str) {
    let Some(groups) = config.proxy_groups.as_mut() else {
        return;
    };
    let provider_value = yaml_string(provider_name);
    for group in groups {
        let Some(mapping) = group.as_mapping_mut() else {
            continue;
        };
        let Some(uses) = mapping.get_mut(yaml_key("use")) else {
            continue;
        };
        if let Some(sequence) = uses.as_sequence_mut() {
            sequence.retain(|value| value != &provider_value);
        }
    }
}

fn ensure_default_route_uses_proxy(config: &mut ClashConfig) {
    let proxy_match = yaml_string("MATCH,Proxy");
    match config.extra.get_mut("rules") {
        Some(serde_yaml::Value::Sequence(rules)) if rules.is_empty() => {
            rules.push(proxy_match);
        }
        Some(serde_yaml::Value::Sequence(rules)) if rules.len() == 1 => {
            if matches!(
                rules.first().and_then(serde_yaml::Value::as_str),
                Some("MATCH,DIRECT" | "FINAL,DIRECT")
            ) {
                rules[0] = proxy_match;
            }
        }
        Some(_) => {}
        None => {
            config.extra.insert(
                "rules".to_string(),
                serde_yaml::Value::Sequence(vec![proxy_match]),
            );
        }
    }
}

fn normalize_provider_cache(body: &str) -> Result<Option<String>> {
    let yaml = serde_yaml::from_str::<serde_yaml::Value>(body)
        .with_context(|| "parse provider cache yaml")?;

    let proxies = match yaml {
        serde_yaml::Value::Mapping(mapping) => mapping
            .get(yaml_key("proxies"))
            .and_then(serde_yaml::Value::as_sequence)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing top-level proxies list"))?,
        serde_yaml::Value::Sequence(sequence) => sequence,
        _ => return Err(anyhow::anyhow!("unsupported provider cache yaml shape")),
    };

    let proxy_count = proxies
        .iter()
        .filter(|proxy| {
            proxy
                .as_mapping()
                .and_then(|mapping| mapping.get(yaml_key("name")))
                .and_then(serde_yaml::Value::as_str)
                .is_some()
                && proxy
                    .as_mapping()
                    .and_then(|mapping| mapping.get(yaml_key("type")))
                    .and_then(serde_yaml::Value::as_str)
                    .is_some()
        })
        .count();
    if proxy_count == 0 {
        return Err(anyhow::anyhow!("provider cache has no usable proxies"));
    }

    let mut provider = serde_yaml::Mapping::new();
    provider.insert(yaml_key("proxies"), serde_yaml::Value::Sequence(proxies));
    let normalized = serde_yaml::to_string(&serde_yaml::Value::Mapping(provider))
        .with_context(|| "serialize provider cache")?;

    if body.trim() == normalized.trim() {
        Ok(None)
    } else {
        Ok(Some(normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "clash-tui-test-{}-{}",
                name,
                std::process::id()
            ));
            Self { path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }

        fn write(&self, content: &str) {
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut f = std::fs::File::create(&self.path).unwrap();
            f.write_all(content.as_bytes()).unwrap();
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            if let Some(parent) = self.path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
    }

    #[test]
    fn test_load_save_roundtrip() {
        let tmp = TempFile::new("roundtrip");
        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();

        let config = ClashConfig {
            mixed_port: Some(7890),
            external_controller: Some("127.0.0.1:9090".to_string()),
            mode: Some("rule".to_string()),
            ..Default::default()
        };

        manager.save(&config).unwrap();
        let loaded = manager.load().unwrap();

        assert_eq!(loaded.mixed_port, Some(7890));
        assert_eq!(
            loaded.external_controller,
            Some("127.0.0.1:9090".to_string())
        );
        assert_eq!(loaded.mode, Some("rule".to_string()));
    }

    #[test]
    fn test_add_provider() {
        let tmp = TempFile::new("add-provider");
        tmp.write("mixed-port: 7890\n");

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        manager
            .add_provider("test-sub", "https://example.com/sub.yaml", 3600)
            .unwrap();

        let providers = manager.get_providers().unwrap();
        assert_eq!(providers.len(), 1);

        let provider = providers.get("test-sub").unwrap();
        assert_eq!(provider.provider_type, "http");
        assert_eq!(provider.url, "https://example.com/sub.yaml");
        assert_eq!(provider.interval, Some(3600));
        assert_eq!(provider.path, Some("./providers/test-sub.yaml".to_string()));
        assert!(provider.health_check.is_some());

        let hc = provider.health_check.as_ref().unwrap();
        assert!(hc.enable);
        assert_eq!(hc.interval, Some(600));
        assert_eq!(
            hc.url,
            Some("http://www.gstatic.com/generate_204".to_string())
        );

        let loaded = manager.load().unwrap();
        let groups = loaded.proxy_groups.unwrap();
        let proxy_group = groups
            .iter()
            .find(|group| group.get("name").and_then(serde_yaml::Value::as_str) == Some("Proxy"))
            .unwrap();
        let uses = proxy_group.get("use").unwrap().as_sequence().unwrap();
        assert!(uses.contains(&serde_yaml::Value::String("test-sub".to_string())));
    }

    #[test]
    fn test_add_provider_preserves_existing_fields() {
        let tmp = TempFile::new("preserve");
        tmp.write(
            r#"mixed-port: 7890
external-controller: 127.0.0.1:9090
mode: rule
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
"#,
        );

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        manager.add_provider("sub1", "https://a.com", 3600).unwrap();

        let loaded = manager.load().unwrap();
        assert_eq!(loaded.mixed_port, Some(7890));
        assert_eq!(
            loaded.external_controller,
            Some("127.0.0.1:9090".to_string())
        );
        assert_eq!(loaded.mode, Some("rule".to_string()));
        assert!(loaded.proxy_groups.is_some());
        assert_eq!(loaded.proxy_groups.as_ref().unwrap().len(), 1);
        let proxy_group = loaded.proxy_groups.as_ref().unwrap().first().unwrap();
        let uses = proxy_group.get("use").unwrap().as_sequence().unwrap();
        assert!(uses.contains(&serde_yaml::Value::String("sub1".to_string())));
    }

    #[test]
    fn test_remove_provider() {
        let tmp = TempFile::new("remove");
        tmp.write(
            r#"proxy-providers:
  sub1:
    type: http
    url: https://a.com
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
    use:
      - sub1
"#,
        );

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        manager.remove_provider("sub1").unwrap();

        let providers = manager.get_providers().unwrap();
        assert!(providers.is_empty());

        let loaded = manager.load().unwrap();
        let group = loaded.proxy_groups.unwrap().into_iter().next().unwrap();
        let uses = group.get("use").unwrap().as_sequence().unwrap();
        assert!(!uses.contains(&serde_yaml::Value::String("sub1".to_string())));
    }

    #[test]
    fn test_update_provider() {
        let tmp = TempFile::new("update");
        tmp.write(
            r#"proxy-providers:
  sub1:
    type: http
    url: https://old.com
    interval: 3600
"#,
        );

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        manager
            .update_provider("sub1", Some("https://new.com"), Some(7200))
            .unwrap();

        let providers = manager.get_providers().unwrap();
        let provider = providers.get("sub1").unwrap();
        assert_eq!(provider.url, "https://new.com");
        assert_eq!(provider.interval, Some(7200));
    }

    #[test]
    fn test_get_api_url() {
        let tmp = TempFile::new("api-url");
        tmp.write(
            r#"external-controller: 127.0.0.1:9090
"#,
        );

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        assert_eq!(manager.get_api_url(), "http://127.0.0.1:9090");

        // Fallback when no external-controller
        let tmp2 = TempFile::new("api-url-fallback");
        tmp2.write("");
        let manager2 = ConfigManager::new(Some(tmp2.path().clone())).unwrap();
        assert_eq!(manager2.get_api_url(), "http://127.0.0.1:9090");
    }

    #[test]
    fn test_resolve_provider_path() {
        let tmp = TempFile::new("resolve");
        tmp.write("");
        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();

        // Absolute path stays absolute
        let abs = manager.resolve_provider_path("/home/user/providers/sub.yaml");
        assert_eq!(abs, PathBuf::from("/home/user/providers/sub.yaml"));

        // Relative path resolves against config parent
        let rel = manager.resolve_provider_path("./providers/sub.yaml");
        let expected = tmp.path().parent().unwrap().join("providers/sub.yaml");
        assert_eq!(rel, expected);
    }

    #[test]
    fn test_migrate_legacy_config_when_default_path_is_empty() {
        let legacy = TempFile::new("legacy-source");
        legacy.write("mixed-port: 7891\n");
        let target = TempFile::new("legacy-target");
        let _ = std::fs::remove_file(target.path());
        let manager = ConfigManager {
            config_path: target.path().clone(),
            explicit_config: false,
        };

        manager
            .migrate_legacy_config_from(&[legacy.path().clone()])
            .unwrap();

        let migrated = std::fs::read_to_string(target.path()).unwrap();
        assert_eq!(migrated, "mixed-port: 7891\n");
    }

    #[test]
    fn test_migrate_legacy_config_skips_explicit_path() {
        let legacy = TempFile::new("legacy-explicit-source");
        legacy.write("mixed-port: 7891\n");
        let target = TempFile::new("legacy-explicit-target");
        let _ = std::fs::remove_file(target.path());
        let manager = ConfigManager {
            config_path: target.path().clone(),
            explicit_config: true,
        };

        manager
            .migrate_legacy_config_from(&[legacy.path().clone()])
            .unwrap();

        assert!(!target.path().exists());
    }

    #[test]
    fn test_add_provider_valid_yaml() {
        let tmp = TempFile::new("valid-yaml");
        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();

        manager
            .add_provider("my-sub", "https://example.com/sub", 86400)
            .unwrap();

        // Verify raw YAML can be parsed back
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();

        let providers = parsed.get("proxy-providers").unwrap().as_mapping().unwrap();
        assert!(providers.contains_key(serde_yaml::Value::String("my-sub".to_string())));

        let sub = providers
            .get(serde_yaml::Value::String("my-sub".to_string()))
            .unwrap();
        assert_eq!(sub.get("type").unwrap().as_str().unwrap(), "http");
        assert_eq!(
            sub.get("url").unwrap().as_str().unwrap(),
            "https://example.com/sub"
        );
        assert_eq!(sub.get("interval").unwrap().as_u64().unwrap(), 86400);
        assert_eq!(
            sub.get("path").unwrap().as_str().unwrap(),
            "./providers/my-sub.yaml"
        );

        let proxy_group = parsed
            .get("proxy-groups")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .find(|group| group.get("name").and_then(serde_yaml::Value::as_str) == Some("Proxy"))
            .unwrap();
        let uses = proxy_group.get("use").unwrap().as_sequence().unwrap();
        assert!(uses.contains(&serde_yaml::Value::String("my-sub".to_string())));

        let rules = parsed.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules[0].as_str(), Some("MATCH,Proxy"));
    }

    #[test]
    fn test_add_provider_rewrites_generated_direct_fallback() {
        let tmp = TempFile::new("rewrite-direct");
        tmp.write(
            r#"proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
rules:
  - MATCH,DIRECT
"#,
        );

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        manager.add_provider("sub1", "https://a.com", 3600).unwrap();

        let loaded = manager.load().unwrap();
        let rules = loaded.extra.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules[0].as_str(), Some("MATCH,Proxy"));
    }

    #[test]
    fn test_repair_for_tui_adds_existing_providers_to_proxy_group() {
        let tmp = TempFile::new("repair-existing");
        tmp.write(
            r#"proxy-providers:
  sub1:
    type: http
    url: https://a.com
    path: ./providers/sub1.yaml
  sub2:
    type: http
    url: https://b.com
    path: ./providers/sub2.yaml
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
    use:
      - sub1
rules:
  - MATCH,DIRECT
"#,
        );

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        assert!(manager.repair_for_tui().unwrap());

        let loaded = manager.load().unwrap();
        let proxy_group = loaded.proxy_groups.as_ref().unwrap().first().unwrap();
        let uses = proxy_group.get("use").unwrap().as_sequence().unwrap();
        assert!(uses.contains(&serde_yaml::Value::String("sub1".to_string())));
        assert!(uses.contains(&serde_yaml::Value::String("sub2".to_string())));
        let rules = loaded.extra.get("rules").unwrap().as_sequence().unwrap();
        assert_eq!(rules[0].as_str(), Some("MATCH,Proxy"));
    }

    #[test]
    fn test_repair_provider_caches_extracts_proxies_from_full_config() {
        let tmp = TempFile::new("repair-cache/config.yaml");
        tmp.write(
            r#"proxy-providers:
  sub1:
    type: http
    url: https://a.com
    path: ./providers/sub1.yaml
"#,
        );
        let provider_path = tmp.path().parent().unwrap().join("providers/sub1.yaml");
        std::fs::create_dir_all(provider_path.parent().unwrap()).unwrap();
        std::fs::write(
            &provider_path,
            r#"mixed-port: 7890
dns:
  enable: true
proxies:
  - name: node-a
    type: direct
"#,
        )
        .unwrap();

        let manager = ConfigManager::new(Some(tmp.path().clone())).unwrap();
        assert_eq!(manager.repair_provider_caches().unwrap(), 1);

        let repaired = std::fs::read_to_string(provider_path).unwrap();
        let yaml: serde_yaml::Value = serde_yaml::from_str(&repaired).unwrap();
        assert!(yaml.get("proxies").is_some());
        assert!(yaml.get("mixed-port").is_none());
        assert!(yaml.get("dns").is_none());
        let _ = std::fs::remove_dir_all(tmp.path().parent().unwrap().join("providers"));
    }
}
