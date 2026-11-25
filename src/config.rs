use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::backend::ProbeKey;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub last_probe: Option<ProbeKey>,
    #[serde(default)]
    pub last_target: String,
    #[serde(default)]
    pub last_hex: Option<PathBuf>,
    #[serde(default = "default_verify")]
    pub verify: bool,
    #[serde(default)]
    pub chip_erase: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_probe: None,
            last_target: String::new(),
            last_hex: None,
            verify: true,
            chip_erase: false,
        }
    }
}

fn default_verify() -> bool {
    true
}

pub fn config_path() -> PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return PathBuf::from(app_data).join("AnyLink").join("config.json");
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(home).join(".anylink").join("config.json");
    }
    PathBuf::from(".anylink-config.json")
}

pub fn load_config() -> AppConfig {
    load_config_from(&config_path())
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    save_config_to(&config_path(), config)
}

pub fn load_config_from(path: &Path) -> AppConfig {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return AppConfig::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save_config_to(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建配置目录“{}”：{error}", parent.display()))?;
    }

    let json =
        serde_json::to_string_pretty(config).map_err(|error| format!("无法序列化配置：{error}"))?;
    std::fs::write(path, json).map_err(|error| format!("无法保存配置“{}”：{error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_expected_defaults() {
        let path = std::env::temp_dir().join("anylink-never-created-config.json");
        let config = load_config_from(&path);
        assert_eq!(config, AppConfig::default());
        assert!(config.verify);
        assert!(!config.chip_erase);
    }

    #[test]
    fn config_round_trips() {
        let path =
            std::env::temp_dir().join(format!("anylink-config-test-{}.json", std::process::id()));
        let config = AppConfig {
            last_probe: Some(ProbeKey {
                vendor_id: 0x0d28,
                product_id: 0x0204,
                serial_number: Some("test".to_string()),
                interface: Some(3),
            }),
            last_target: "STM32H743ZI".to_string(),
            last_hex: Some(PathBuf::from("C:/firmware/demo.hex")),
            verify: false,
            chip_erase: true,
        };

        save_config_to(&path, &config).expect("write config");
        assert_eq!(load_config_from(&path), config);
        let _ = std::fs::remove_file(path);
    }
}
