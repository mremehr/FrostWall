# FrostWall

**Intelligent wallpaper manager for Wayland, built for multi-monitor setups.**

FrostWall helps you browse, match, preview, and apply wallpapers without fighting different screen shapes, manual shell scripts, or broken monitor pairings.

## Why FrostWall

- **Screen-aware matching**: filters wallpapers by aspect category per screen
- **Real previews in the terminal**: browse with thumbnails instead of filenames
- **Pairing mode**: preview one wallpaper together with suggested matches for other monitors
- **Useful automation**: tags, collections, watch daemon, time profiles, pywal export
- **Optional AI tagging**: CLIP-based auto-tagging when built with the `clip` feature

## Quick Start

### 1. Requirements

- Wayland
- `awww` for wallpaper application
- `niri` or `wlr-randr` for screen detection
- Rust + Cargo
- Recommended terminal for the best preview experience: Kitty or another terminal supported by `ratatui-image`

### 2. Install

```bash
git clone https://github.com/mrmattias/frostwall.git
cd frostwall

# Install the normal build
cargo install --path .

# Optional: install with CLIP auto-tagging
# cargo install --path . --features clip

# Optional: CLIP + CUDA
# cargo install --path . --features clip-cuda
```

### 3. First Run

```bash
frostwall init
frostwall scan
frostwall
```

`frostwall init` creates your config and points FrostWall at your wallpaper directory.

## Everyday Use

```bash
frostwall                 # Open the TUI
frostwall random          # Apply a random wallpaper per screen
frostwall next            # Next wallpaper
frostwall prev            # Previous wallpaper
frostwall screens         # Show detected screens
frostwall scan            # Rescan wallpaper directory
frostwall watch           # Background rotation daemon
```

### Common Workflows

```bash
# Tagging
frostwall tag list
frostwall tag add ~/Pictures/wallpapers/forest.jpg nature
frostwall tag show nature

# Similarity search
frostwall similar ~/Pictures/wallpapers/favorite.jpg --limit 10

# Pairing
frostwall pair stats
frostwall pair suggest ~/Pictures/wallpapers/favorite.jpg

# Collections
frostwall collection save work
frostwall collection apply work

# Time profiles
frostwall time-profile enable
frostwall time-profile status
frostwall time-profile apply

# pywal export
frostwall pywal ~/Pictures/wallpapers/forest.jpg --apply

# Web import
frostwall import wallhaven "nature 4k"
frostwall import download w8x7y9
```

## TUI Cheat Sheet

| Key | Action |
|-----|--------|
| `h` / `←` | Previous wallpaper |
| `l` / `→` | Next wallpaper |
| `Enter` | Apply wallpaper |
| `Tab` / `Shift+Tab` | Switch screen |
| `p` | Open pairing preview |
| `r` | Random wallpaper |
| `R` | Incremental rescan |
| `:` | Command mode |
| `m` | Toggle match mode |
| `f` | Toggle resize mode |
| `a` | Toggle aspect grouping |
| `c` | Toggle palette view |
| `C` | Open color filter |
| `t` / `T` | Cycle / clear tag filter |
| `w` / `W` | Export / auto-export pywal |
| `i` | Toggle thumbnail protocol |
| `u` | Undo pairing auto-apply |
| `?` | Help |
| `q` / `Esc` | Quit |

Pairing preview:

- `←` / `→`: cycle alternatives
- `1`-`9`, `0`: jump to a specific alternative
- `y`: cycle style mode `Off -> Soft -> Strict`
- `Enter`: apply the selected pairing

## Optional Features

### CLIP Auto-Tagging

Build with `clip` to unlock:

```bash
cargo install --path . --features clip
frostwall auto-tag --incremental
```

### Watch Daemon

```bash
frostwall watch --interval 30m
frostwall watch --interval 1h --shuffle false
frostwall watch --watch-dir false
```

### Override the Wallpaper Directory

```bash
frostwall --dir ~/Pictures/wallpapers
frostwall --dir ~/Pictures/wallpapers random
```

## Config

- Main config: `~/.config/frostwall/config.toml`
- Profiles: `~/.config/frostwall/profiles.toml`

If the config file does not exist, FrostWall creates it automatically.

## Docs

- [Usage Guide](docs/USAGE.md)
- [Configuration Guide](docs/CONFIGURATION.md)
- [Contributing](CONTRIBUTING.md)

## License

GPL-2.0
