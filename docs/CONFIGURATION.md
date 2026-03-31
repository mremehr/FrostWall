# FrostWall Configuration Guide

This document covers the settings that matter most in day-to-day use.

## File Locations

- Main config: `~/.config/frostwall/config.toml`
- Profiles: `~/.config/frostwall/profiles.toml`
- Cache and generated data: XDG cache/data directories for `frostwall`

If `config.toml` does not exist, FrostWall creates it automatically.

## Minimal Example

```toml
[wallpaper]
directory = "~/Pictures/wallpapers"
recursive = false

[display]
match_mode = "Flexible"
resize_mode = "Fit"
aspect_sort = false

[thumbnails]
width = 800
height = 600
quality = 85
grid_columns = 3
preload_count = 20

[terminal]
kitty_safe_thumbnails = true

[pairing]
enabled = true
auto_apply = false

[time_profiles]
enabled = false
```

## The Important Sections

### `[wallpaper]`

```toml
[wallpaper]
directory = "~/Pictures/wallpapers"
extensions = ["jpg", "jpeg", "png", "webp", "bmp", "gif"]
recursive = false
```

Use this section to point FrostWall at the folder you actually want scanned.

### `[display]`

```toml
[display]
match_mode = "Flexible"
resize_mode = "Fit"
aspect_sort = false
```

`match_mode`:

- `Strict`: only exact aspect category matches
- `Flexible`: compatible aspect categories are allowed
- `All`: show everything

`resize_mode`:

- `Crop`
- `Fit`
- `No`
- `Stretch`

### `[display.fill_color]`

Used when the resize mode leaves padding:

```toml
[display.fill_color]
r = 0
g = 0
b = 0
a = 255
```

### `[transition]`

```toml
[transition]
transition_type = "fade"
duration = 1.0
fps = 60
```

Useful values for `transition_type`:

- `fade`
- `wipe`
- `grow`
- `center`
- `outer`
- `none`

### `[thumbnails]`

```toml
[thumbnails]
width = 800
height = 600
quality = 85
grid_columns = 3
preload_count = 20
```

What matters:

- increase `width` / `height` if previews look too soft
- lower them if you want lighter disk/cache usage
- increase `grid_columns` if your terminal is very wide
- increase `preload_count` if you scroll fast and want more warm thumbnails

### `[theme]`

```toml
[theme]
mode = "auto"
check_interval_ms = 500
```

Valid `mode` values:

- `auto`
- `light`
- `dark`

### `[terminal]`

```toml
[terminal]
recommended_repaint_delay = 5
recommended_input_delay = 1
hint_shown = false
kitty_safe_thumbnails = true
```

`kitty_safe_thumbnails = true` is the safest default in Kitty and reduces graphics-protocol weirdness.

### `[pairing]`

```toml
[pairing]
enabled = true
auto_apply = false
undo_window_secs = 5
auto_apply_threshold = 0.7
max_history_records = 1000
preview_match_limit = 10
screen_context_weight = 8.0
visual_weight = 5.0
harmony_weight = 3.0
tag_weight = 2.0
semantic_weight = 7.0
repetition_penalty_weight = 1.0
```

Practical guidance:

- leave the weights alone first
- tune only after using pairing for a while
- `auto_apply = false` is the safer default
- `preview_match_limit = 10` is a good balance for the pairing UI

### `[time_profiles]`

```toml
[time_profiles]
enabled = false
```

Each period can bias wallpapers by brightness and tags:

```toml
[time_profiles.morning]
brightness_range = [0.5, 0.9]
preferred_tags = ["nature", "bright", "pastel"]
brightness_weight = 0.6
tag_weight = 0.4
```

Default periods:

- morning
- afternoon
- evening
- night

### `[clip]`

```toml
[clip]
enabled = false
threshold = 0.25
show_in_filter = true
cache_embeddings = true
# visual_model_url = "https://..."
# visual_model_sha256 = "..."
```

Notes:

- this only matters when FrostWall is built with `--features clip`
- custom model URLs should also provide `visual_model_sha256`

### `[keybindings]`

Defaults:

```toml
[keybindings]
next = "l"
prev = "h"
apply = "Enter"
quit = "q"
random = "r"
toggle_match = "m"
toggle_resize = "f"
next_screen = "Tab"
prev_screen = "BackTab"
```

## Recommended Starting Tweaks

If you just want a sane setup:

```toml
[wallpaper]
recursive = true

[display]
match_mode = "Flexible"
resize_mode = "Fit"

[pairing]
enabled = true
auto_apply = false

[time_profiles]
enabled = true
```

## Related Docs

- [Usage Guide](USAGE.md)
- [Contributing](../CONTRIBUTING.md)
