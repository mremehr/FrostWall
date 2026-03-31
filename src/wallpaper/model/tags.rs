use crate::wallpaper::Wallpaper;

fn normalized_tag(tag: &str) -> String {
    tag.trim().to_lowercase()
}

impl Wallpaper {
    /// Add a tag to this wallpaper.
    pub fn add_tag(&mut self, tag: &str) {
        let normalized = normalized_tag(tag);
        if !normalized.is_empty() && !self.tags.contains(&normalized) {
            self.tags.push(normalized);
            self.tags.sort();
        }
    }

    /// Remove a tag from this wallpaper.
    pub fn remove_tag(&mut self, tag: &str) {
        let normalized = normalized_tag(tag);
        self.tags.retain(|existing| existing != &normalized);
    }

    /// Check if wallpaper has a specific tag (manual or auto).
    pub fn has_tag(&self, tag: &str) -> bool {
        let normalized = normalized_tag(tag);
        self.tags.iter().any(|existing| existing == &normalized)
            || self
                .auto_tags
                .iter()
                .any(|auto_tag| normalized_tag(&auto_tag.name) == normalized)
    }

    /// Check if wallpaper has any of the given tags.
    #[cfg(test)]
    pub fn has_any_tag(&self, tags: &[String]) -> bool {
        tags.iter().any(|tag| self.has_tag(tag))
    }

    /// Check if wallpaper has all of the given tags.
    #[cfg(test)]
    pub fn has_all_tags(&self, tags: &[String]) -> bool {
        tags.iter().all(|tag| self.has_tag(tag))
    }

    /// Get all tags (manual + auto tag names).
    pub fn all_tags(&self) -> Vec<String> {
        let mut all_tags = self.tags.clone();
        all_tags.extend(self.auto_tags.iter().map(|tag| tag.name.clone()));
        all_tags.sort();
        all_tags.dedup();
        all_tags
    }

    /// Get auto tags above a confidence threshold.
    #[cfg(test)]
    pub fn auto_tags_above(&self, threshold: f32) -> Vec<&crate::clip::AutoTag> {
        self.auto_tags
            .iter()
            .filter(|tag| tag.confidence >= threshold)
            .collect()
    }
}
