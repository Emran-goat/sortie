use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub targets: HashMap<String, TargetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    pub host: Option<String>,
    pub hosts: Option<Vec<String>>,
    pub port: Option<u16>,
    pub user: String,
    pub key_path: Option<String>,
    pub target_triple: String,
    pub deploy_path: String,
    pub health_check_url: Option<String>,
    pub health_check_timeout_secs: Option<u64>,
    pub service: Option<ServiceConfig>,
    pub build_args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub instances: Option<u32>,
    #[serde(default)]
    pub cross_compile: bool,
}

impl TargetConfig {
    pub fn get_hosts(&self) -> Vec<String> {
        if let Some(ref hosts) = self.hosts {
            if !hosts.is_empty() {
                return hosts.clone();
            }
        }
        if let Some(ref host) = self.host {
            if !host.is_empty() {
                return vec![host.clone()];
            }
        }
        vec![]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub restart: Option<String>,
    pub extra_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostState {
    pub version: String,
    pub binary_hash: String,
    pub timestamp: String,
    pub services: HashMap<String, ServiceEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub hosts: Vec<String>,
    pub port: u16,
    pub health: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostStatus {
    pub host: String,
    pub running: bool,
    pub error: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut targets = HashMap::new();
        targets.insert(
            "production".to_string(),
            TargetConfig {
                host: None,
                hosts: Some(vec!["1.2.3.4".to_string()]),
                port: None,
                user: String::new(),
                key_path: None,
                target_triple: "x86_64-unknown-linux-gnu".to_string(),
                deploy_path: String::new(),
                health_check_url: None,
                health_check_timeout_secs: Some(30),
                service: None,
                build_args: None,
                env: None,
                instances: None,
                cross_compile: false,
            },
        );
        Config { targets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_hosts_uses_hosts_field() {
        let cfg = TargetConfig {
            host: Some("1.2.3.4".into()),
            hosts: Some(vec!["10.0.0.1".into(), "10.0.0.2".into()]),
            port: None,
            user: "deploy".into(),
            key_path: None,
            target_triple: "x86_64-unknown-linux-gnu".into(),
            deploy_path: "/opt/app".into(),
            health_check_url: None,
            health_check_timeout_secs: None,
            service: None,
            build_args: None,
            env: None,
            instances: None,
            cross_compile: false,
        };
        assert_eq!(cfg.get_hosts(), vec!["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn test_get_hosts_falls_back_to_host() {
        let cfg = TargetConfig {
            host: Some("1.2.3.4".into()),
            hosts: None,
            port: None,
            user: "deploy".into(),
            key_path: None,
            target_triple: "x86_64-unknown-linux-gnu".into(),
            deploy_path: "/opt/app".into(),
            health_check_url: None,
            health_check_timeout_secs: None,
            service: None,
            build_args: None,
            env: None,
            instances: None,
            cross_compile: false,
        };
        assert_eq!(cfg.get_hosts(), vec!["1.2.3.4"]);
    }

    #[test]
    fn test_get_hosts_returns_empty_when_none_given() {
        let cfg = TargetConfig {
            host: None,
            hosts: None,
            port: None,
            user: "deploy".into(),
            key_path: None,
            target_triple: "x86_64-unknown-linux-gnu".into(),
            deploy_path: "/opt/app".into(),
            health_check_url: None,
            health_check_timeout_secs: None,
            service: None,
            build_args: None,
            env: None,
            instances: None,
            cross_compile: false,
        };
        assert!(cfg.get_hosts().is_empty());
    }

    #[test]
    fn test_get_hosts_ignores_empty_hosts() {
        let hosts: Vec<String> = vec![];
        let cfg = TargetConfig {
            host: Some("fallback".into()),
            hosts: Some(hosts),
            port: None,
            user: "deploy".into(),
            key_path: None,
            target_triple: "x86_64-unknown-linux-gnu".into(),
            deploy_path: "/opt/app".into(),
            health_check_url: None,
            health_check_timeout_secs: None,
            service: None,
            build_args: None,
            env: None,
            instances: None,
            cross_compile: false,
        };
        assert_eq!(cfg.get_hosts(), vec!["fallback"]);
    }

    #[test]
    fn test_default_config_has_production_target() {
        let cfg = Config::default();
        assert!(cfg.targets.contains_key("production"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert!(parsed.targets.contains_key("production"));
    }
}
