//! Wallpaper collections/sets management
//!
//! Save and recall favorite multi-screen wallpaper combinations as named presets.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A saved wallpaper collection (preset)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperCollection {
    /// Collection name
    pub name: String,
    /// Wallpaper paths per screen (screen_name -> wallpaper_path)
    pub wallpapers: HashMap<String, PathBuf>,
    /// When this collection was created (Unix timestamp)
    pub created_at: u64,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Collection storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectionStore {
    pub collections: Vec<WallpaperCollection>,
}

impl CollectionStore {
    /// Get storage path
    fn storage_path() -> PathBuf {
        directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.data_dir().join("collections.json"))
            .unwrap_or_else(|| PathBuf::from("/tmp/frostwall/collections.json"))
    }

    /// Load collections from disk
    pub fn load() -> Result<Self> {
        let path = Self::storage_path();

        if path.exists() {
            let content =
                std::fs::read_to_string(&path).context("Failed to read collections file")?;
            let store: Self =
                serde_json::from_str(&content).context("Failed to parse collections file")?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    /// Save collections to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::storage_path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        Ok(())
    }

    /// Add a new collection
    pub fn add(
        &mut self,
        name: String,
        wallpapers: HashMap<String, PathBuf>,
        description: Option<String>,
    ) -> Result<()> {
        // Check for duplicate name
        if self.collections.iter().any(|c| c.name == name) {
            anyhow::bail!("Collection '{}' already exists", name);
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.collections.push(WallpaperCollection {
            name,
            wallpapers,
            created_at: timestamp,
            description,
            tags: Vec::new(),
        });

        self.save()?;
        Ok(())
    }

    /// Get a collection by name
    pub fn get(&self, name: &str) -> Option<&WallpaperCollection> {
        self.collections.iter().find(|c| c.name == name)
    }

    /// Delete a collection by name
    pub fn delete(&mut self, name: &str) -> Result<bool> {
        let initial_len = self.collections.len();
        self.collections.retain(|c| c.name != name);

        if self.collections.len() < initial_len {
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// CLI commands for collection management
pub fn cmd_collection_list() -> Result<()> {
    let store = CollectionStore::load()?;

    if store.collections.is_empty() {
        println!("No collections saved.");
        println!("Create one with: frostwall collection save <name>");
    } else {
        println!("Collections:");
        for collection in &store.collections {
            let screen_count = collection.wallpapers.len();
            let desc = collection.description.as_deref().unwrap_or("");
            if desc.is_empty() {
                println!("  {} ({} screens)", collection.name, screen_count);
            } else {
                println!(
                    "  {} ({} screens) - {}",
                    collection.name, screen_count, desc
                );
            }
        }
    }

    Ok(())
}

pub fn cmd_collection_show(name: &str) -> Result<()> {
    let store = CollectionStore::load()?;

    if let Some(collection) = store.get(name) {
        println!("Collection: {}", collection.name);
        if let Some(desc) = &collection.description {
            println!("Description: {}", desc);
        }
        println!("Wallpapers:");
        for (screen, path) in &collection.wallpapers {
            println!("  {}: {}", screen, path.display());
        }
    } else {
        println!("Collection '{}' not found", name);
    }

    Ok(())
}

pub fn cmd_collection_delete(name: &str) -> Result<()> {
    let mut store = CollectionStore::load()?;

    if store.delete(name)? {
        println!("Deleted collection '{}'", name);
    } else {
        println!("Collection '{}' not found", name);
    }

    Ok(())
}
