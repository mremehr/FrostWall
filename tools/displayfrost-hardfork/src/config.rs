use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DeviceResolution {
    pub input: String,
    pub resolved: String,
    pub used_alias: bool,
}

#[derive(Debug, Default, Deserialize)]
struct AppConfig {
    #[serde(default)]
    chromecast: ChromecastConfig,
    #[serde(default)]
    default_output: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChromecastConfig {
    #[serde(default)]
    aliases: HashMap<String, String>,
    #[serde(default)]
    default_device: Option<String>,
}

pub fn resolve_device_alias(input: &str) -> Result<DeviceResolution> {
    let name = input.trim();
    if name.is_empty() {
        bail!("Device name must not be empty");
    }

    let Some(config) = load_config()? else {
        return Ok(DeviceResolution {
            input: name.to_string(),
            resolved: name.to_string(),
            used_alias: false,
        });
    };

    if let Some(target) = lookup_alias(&config.chromecast.aliases, name) {
        let resolved = target.trim();
        if resolved.is_empty() {
            bail!(
                "Alias '{}' exists in config but resolves to an empty device name",
                name
            );
        }
        return Ok(DeviceResolution {
            input: name.to_string(),
            resolved: resolved.to_string(),
            used_alias: true,
        });
    }

    Ok(DeviceResolution {
        input: name.to_string(),
        resolved: name.to_string(),
        used_alias: false,
    })
}

pub fn default_output() -> Result<Option<String>> {
    let Some(config) = load_config()? else {
        return Ok(None);
    };
    Ok(config.default_output)
}

pub fn default_device() -> Result<Option<String>> {
    let Some(config) = load_config()? else {
        return Ok(None);
    };
    Ok(config.chromecast.default_device)
}

fn lookup_alias<'a>(aliases: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    if let Some(value) = aliases.get(key) {
        return Some(value.as_str());
    }

    let needle = key.to_ascii_lowercase();
    aliases
        .iter()
        .find_map(|(alias, value)| (alias.to_ascii_lowercase() == needle).then_some(value.as_str()))
}

fn load_config() -> Result<Option<AppConfig>> {
    let Some(path) = config_path() else {
        return Ok(None);
    };
    if !path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {}", path.display()))?;
    let parsed: AppConfig = serde_json::from_str(&raw)
        .with_context(|| format!("Invalid JSON config at {}", path.display()))?;
    Ok(Some(parsed))
}

fn config_path() -> Option<PathBuf> {
    if let Some(xdg_home) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg_home).join("displayfrost/config.json"));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/displayfrost/config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_alias_exact_match() {
        let mut aliases = HashMap::new();
        aliases.insert("tv".to_string(), "Living Room TV".to_string());
        assert_eq!(lookup_alias(&aliases, "tv"), Some("Living Room TV"));
    }

    #[test]
    fn lookup_alias_case_insensitive() {
        let mut aliases = HashMap::new();
        aliases.insert("Vardagsrum".to_string(), "LG-TV-abc123".to_string());
        assert_eq!(lookup_alias(&aliases, "vardagsrum"), Some("LG-TV-abc123"));
        assert_eq!(lookup_alias(&aliases, "VARDAGSRUM"), Some("LG-TV-abc123"));
    }

    #[test]
    fn lookup_alias_missing() {
        let aliases = HashMap::new();
        assert_eq!(lookup_alias(&aliases, "nonexistent"), None);
    }

    #[test]
    fn resolve_device_alias_empty_fails() {
        let result = resolve_device_alias("");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_device_alias_passthrough() {
        // Without a config file, input passes through unchanged
        let result = resolve_device_alias("Some Device").unwrap();
        assert_eq!(result.resolved, "Some Device");
        assert!(!result.used_alias);
    }

    #[test]
    fn deserialize_config_with_default_device() {
        let json = r#"{"chromecast": {"default_device": "My TV", "aliases": {"tv": "My TV"}}}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.chromecast.default_device, Some("My TV".to_string()));
        assert_eq!(config.chromecast.aliases.get("tv").unwrap(), "My TV");
    }

    #[test]
    fn deserialize_config_without_default_device() {
        let json = r#"{"chromecast": {"aliases": {"tv": "My TV"}}}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.chromecast.default_device, None);
    }

    #[test]
    fn deserialize_empty_config() {
        let json = r#"{}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert!(config.chromecast.aliases.is_empty());
        assert_eq!(config.chromecast.default_device, None);
    }
}
