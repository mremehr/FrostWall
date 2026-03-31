use super::sections::KeybindingsConfig;
use super::Config;

#[test]
fn display_defaults_disable_aspect_sort() {
    let config = Config::default();
    assert!(!config.display.aspect_sort);
}

#[test]
fn backend_defaults_to_auto() {
    let config = Config::default();
    assert_eq!(config.backend.kind.display_name(), "auto");
}

#[test]
fn config_serialization_includes_aspect_sort() {
    let config = Config::default();
    let toml = toml::to_string_pretty(&config).expect("serialize default config");
    assert!(toml.contains("aspect_sort = false"));
    assert!(toml.contains("kind = \"auto\""));
}

#[test]
fn test_config_default_is_valid() {
    let config = Config::default();
    assert!(!config.clip.enabled, "CLIP should be opt-in by default");
    assert!(
        !config.pairing.auto_apply,
        "auto-apply should be conservative"
    );
    assert!(config.pairing.max_history_records > 0);
    assert!(config.pairing.auto_apply_threshold > 0.0);
    assert!(config.pairing.auto_apply_threshold <= 1.0);
}

#[test]
fn test_config_toml_roundtrip() {
    let original = Config::default();
    let toml_str = toml::to_string_pretty(&original).expect("serialize");
    let restored: Config = toml::from_str(&toml_str).expect("deserialize");
    assert_eq!(restored.pairing.enabled, original.pairing.enabled);
    assert_eq!(
        restored.pairing.max_history_records,
        original.pairing.max_history_records
    );
    assert!(
        (restored.pairing.auto_apply_threshold - original.pairing.auto_apply_threshold).abs()
            < f32::EPSILON
    );
    assert_eq!(restored.thumbnails.width, original.thumbnails.width);
    assert_eq!(restored.keybindings.next, original.keybindings.next);
}

#[test]
fn test_keybinding_parse_key_basic() {
    use crossterm::event::KeyCode;
    assert_eq!(KeybindingsConfig::parse_key("Enter"), Some(KeyCode::Enter));
    assert_eq!(KeybindingsConfig::parse_key("q"), Some(KeyCode::Char('q')));
    assert_eq!(KeybindingsConfig::parse_key("f1"), Some(KeyCode::F(1)));
    assert_eq!(KeybindingsConfig::parse_key("Esc"), Some(KeyCode::Esc));
    assert_eq!(KeybindingsConfig::parse_key("Tab"), Some(KeyCode::Tab));
    assert_eq!(
        KeybindingsConfig::parse_key("BackTab"),
        Some(KeyCode::BackTab)
    );
}

#[test]
fn test_keybinding_parse_unknown_returns_none() {
    assert_eq!(KeybindingsConfig::parse_key("xyz123"), None);
    assert_eq!(KeybindingsConfig::parse_key("ctrl+c"), None);
    assert_eq!(KeybindingsConfig::parse_key(""), None);
}
