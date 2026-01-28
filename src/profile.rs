use crate::swww::ResizeMode;
use crate::wallpaper::MatchMode;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// A named profile with its own settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub directory: Option<PathBuf>,
    #[serde(default)]
    pub match_mode: Option<MatchMode>,
    #[serde(default)]
    pub resize_mode: Option<ResizeMode>,
    #[serde(default)]
    pub transition_type: Option<String>,
    #[serde(default)]
    pub transition_duration: Option<f32>,
    #[serde(default)]
    pub recursive: Option<bool>,
}

impl Profile {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            directory: None,
            match_mode: None,
            resize_mode: None,
            transition_type: None,
            transition_duration: None,
            recursive: None,
        }
    }
}

/// Manages multiple profiles
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileManager {
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub active_profile: Option<String>,
}

impl ProfileManager {
    fn config_path() -> PathBuf {
        directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("profiles.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let data = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let manager: ProfileManager = toml::from_str(&data)
                .with_context(|| "Failed to parse profiles.toml")?;
            Ok(manager)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = toml::to_string_pretty(self)?;
        fs::write(&path, data)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn create(&mut self, name: &str) -> &mut Profile {
        let profile = Profile::new(name);
        self.profiles.insert(name.to_string(), profile);
        self.profiles.get_mut(name).unwrap()
    }

    pub fn delete(&mut self, name: &str) -> bool {
        if name == "default" {
            return false; // Can't delete default
        }
        self.profiles.remove(name).is_some()
    }

    pub fn list(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        if self.profiles.contains_key(name) {
            self.active_profile = Some(name.to_string());
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn active(&self) -> Option<&Profile> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.get(name))
    }
}

/// CLI commands for profile management
pub fn cmd_profile_list() -> Result<()> {
    let manager = ProfileManager::load()?;

    if manager.profiles.is_empty() {
        println!("No profiles configured.");
        println!("Create one with: frostwall profile create <name>");
        return Ok(());
    }

    println!("Available profiles:\n");
    for (name, profile) in &manager.profiles {
        let active = manager.active_profile.as_ref() == Some(name);
        let marker = if active { " (active)" } else { "" };

        println!("  {}{}", name, marker);

        if let Some(dir) = &profile.directory {
            println!("    directory: {}", dir.display());
        }
        if let Some(mode) = &profile.match_mode {
            println!("    match_mode: {:?}", mode);
        }
        if let Some(mode) = &profile.resize_mode {
            println!("    resize_mode: {}", mode.display_name());
        }
        println!();
    }

    Ok(())
}

pub fn cmd_profile_create(name: &str) -> Result<()> {
    let mut manager = ProfileManager::load()?;

    if manager.profiles.contains_key(name) {
        println!("Profile '{}' already exists.", name);
        return Ok(());
    }

    manager.create(name);
    manager.save()?;

    println!("✓ Created profile '{}'", name);
    println!("\nEdit ~/.config/frostwall/profiles.toml to configure it, or use:");
    println!("  frostwall profile set {} directory ~/Pictures/wallpapers/work", name);

    Ok(())
}

pub fn cmd_profile_delete(name: &str) -> Result<()> {
    let mut manager = ProfileManager::load()?;

    if name == "default" {
        println!("Cannot delete the default profile.");
        return Ok(());
    }

    if manager.delete(name) {
        manager.save()?;
        println!("✓ Deleted profile '{}'", name);
    } else {
        println!("Profile '{}' not found.", name);
    }

    Ok(())
}

pub fn cmd_profile_use(name: &str) -> Result<()> {
    let mut manager = ProfileManager::load()?;

    if manager.set_active(name) {
        manager.save()?;
        println!("✓ Switched to profile '{}'", name);
    } else {
        println!("Profile '{}' not found.", name);
        println!("Available profiles: {:?}", manager.list());
    }

    Ok(())
}

pub fn cmd_profile_set(name: &str, key: &str, value: &str) -> Result<()> {
    let mut manager = ProfileManager::load()?;

    let profile = manager.profiles.get_mut(name);
    if profile.is_none() {
        println!("Profile '{}' not found.", name);
        return Ok(());
    }

    let profile = profile.unwrap();

    match key {
        "directory" | "dir" => {
            profile.directory = Some(PathBuf::from(value));
            println!("✓ Set {}.directory = {}", name, value);
        }
        "match_mode" | "match" => {
            let mode = match value.to_lowercase().as_str() {
                "strict" => MatchMode::Strict,
                "flexible" => MatchMode::Flexible,
                "all" => MatchMode::All,
                _ => {
                    println!("Invalid match_mode. Use: strict, flexible, or all");
                    return Ok(());
                }
            };
            profile.match_mode = Some(mode);
            println!("✓ Set {}.match_mode = {:?}", name, mode);
        }
        "resize_mode" | "resize" => {
            let mode = match value.to_lowercase().as_str() {
                "crop" => ResizeMode::Crop,
                "fit" => ResizeMode::Fit,
                "no" | "center" => ResizeMode::No,
                "stretch" => ResizeMode::Stretch,
                _ => {
                    println!("Invalid resize_mode. Use: crop, fit, center, or stretch");
                    return Ok(());
                }
            };
            profile.resize_mode = Some(mode);
            println!("✓ Set {}.resize_mode = {}", name, mode.display_name());
        }
        "transition" | "trans" => {
            profile.transition_type = Some(value.to_string());
            println!("✓ Set {}.transition = {}", name, value);
        }
        "recursive" => {
            let recursive = value.parse::<bool>().unwrap_or(value == "1" || value == "yes");
            profile.recursive = Some(recursive);
            println!("✓ Set {}.recursive = {}", name, recursive);
        }
        _ => {
            println!("Unknown setting: {}", key);
            println!("Available: directory, match_mode, resize_mode, transition, recursive");
            return Ok(());
        }
    }

    manager.save()?;
    Ok(())
}
