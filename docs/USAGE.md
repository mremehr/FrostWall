# FrostWall Usage Guide

This guide is for people who want FrostWall working quickly without reverse-engineering the project.

## 1. First-Time Setup

### Requirements

- Wayland
- `awww`
- `niri` or `wlr-randr`
- Rust + Cargo

### Install

```bash
git clone https://github.com/mrmattias/frostwall.git
cd frostwall
cargo install --path .
```

Optional builds:

```bash
# CLIP auto-tagging
cargo install --path . --features clip

# CLIP + CUDA
cargo install --path . --features clip-cuda
```

### Initialize the App

```bash
frostwall init
frostwall scan
frostwall
```

What happens:

1. `init` creates `~/.config/frostwall/config.toml`
2. `scan` indexes your wallpaper directory
3. `frostwall` opens the TUI

## 2. The Main Ways to Use FrostWall

### Browse Visually in the TUI

```bash
frostwall
```

Best when you want to:

- browse thumbnails
- preview how a wallpaper fits the current screen
- use pairing preview for multiple monitors
- apply wallpapers interactively

### Use It Like a Scriptable CLI Tool

```bash
frostwall random
frostwall next
frostwall prev
frostwall screens
frostwall scan
```

Best when you want:

- keybindings from your compositor
- shell aliases
- automation via scripts

### Run It as a Background Rotator

```bash
frostwall watch --interval 30m
frostwall watch --interval 1h --shuffle false
frostwall watch --watch-dir false
```

## 3. Daily Workflow Cheatsheet

### Tag Wallpapers

```bash
frostwall tag list
frostwall tag add ~/Pictures/wallpapers/forest.jpg nature
frostwall tag remove ~/Pictures/wallpapers/forest.jpg nature
frostwall tag show nature
```

### Find Similar Wallpapers

```bash
frostwall similar ~/Pictures/wallpapers/favorite.jpg --limit 10
```

### Save a Multi-Monitor Setup

```bash
frostwall collection save work
frostwall collection show work
frostwall collection apply work
frostwall collection delete work
```

### Work with Pairing Suggestions

```bash
frostwall pair stats
frostwall pair clear
frostwall pair suggest ~/Pictures/wallpapers/favorite.jpg
```

### Use Time-Based Selection

```bash
frostwall time-profile status
frostwall time-profile enable
frostwall time-profile preview --limit 10
frostwall time-profile apply
```

### Export pywal Colors

```bash
frostwall pywal ~/Pictures/wallpapers/forest.jpg --apply
```

### Use Profiles

```bash
frostwall profile list
frostwall profile create work
frostwall profile use work
frostwall profile set work directory ~/Pictures/wallpapers/work
```

### Import from the Web

```bash
# Wallhaven
frostwall import wallhaven "nature 4k"
frostwall import featured --count 20
frostwall import download w8x7y9

# Unsplash
export UNSPLASH_ACCESS_KEY=your_key
frostwall import unsplash "mountains"
frostwall import download unsplash_<id>
```

## 4. TUI Controls

### Main TUI

| Key | Action |
|-----|--------|
| `h` / `←` | Previous wallpaper |
| `l` / `→` | Next wallpaper |
| `Enter` | Apply wallpaper |
| `Tab` | Next screen |
| `Shift+Tab` | Previous screen |
| `r` | Random wallpaper |
| `R` | Incremental rescan |
| `m` | Toggle match mode |
| `f` | Toggle resize mode |
| `s` | Toggle sort mode |
| `a` | Toggle aspect grouping |
| `c` | Toggle palette view |
| `C` | Open color filter |
| `t` | Cycle tag filter |
| `T` | Clear tag filter |
| `p` | Open pairing preview |
| `w` | Export pywal colors |
| `W` | Toggle auto pywal export |
| `i` | Toggle thumbnail protocol |
| `u` | Undo pairing auto-apply |
| `:` | Command mode |
| `?` | Help |
| `q` / `Esc` | Quit |

### Pairing Preview

| Key | Action |
|-----|--------|
| `←` / `→` | Next / previous alternative |
| `1`-`9`, `0` | Jump to a specific alternative |
| `y` | Cycle style mode |
| `Enter` | Apply the pairing |
| `p` / `Esc` | Close preview |

Style modes:

- `Off`: pure score ranking
- `Soft`: prefers style overlap but still allows looser matches
- `Strict`: pushes much harder for style/content consistency

### Command Mode

Press `:` in the TUI.

| Command | Meaning |
|---------|---------|
| `:t <tag>` | Filter by tag |
| `:tag` | List tags |
| `:clear` / `:c` | Clear filters |
| `:random` / `:r` | Random wallpaper |
| `:apply` / `:a` | Apply wallpaper |
| `:img [toggle|hb|kitty]` | Thumbnail protocol |
| `:similar` / `:sim` | Find similar wallpapers |
| `:sort name/date/size` | Sorting |
| `:aspect [toggle|on|off]` | Aspect grouping |
| `:screen <n>` | Switch screen |
| `:go <n>` | Jump to wallpaper index |
| `:rescan` / `:scan` | Incremental rescan |
| `:pair-reset` / `:pair-rebuild` | Rebuild pairing affinity |
| `:help` / `:h` | Help |
| `:q` / `:quit` | Quit |

## 5. Optional CLIP Auto-Tagging

`auto-tag` only exists in builds with the `clip` feature.

```bash
cargo install --path . --features clip
frostwall auto-tag
frostwall auto-tag --incremental
frostwall auto-tag --threshold 0.55
frostwall auto-tag --max-tags 5
frostwall auto-tag --verbose
```

What it does:

- downloads the CLIP visual model on first use
- generates semantic tags
- stores CLIP embeddings for similarity and pairing

## 6. Troubleshooting

### “No wallpapers found”

Check:

- your `wallpaper.directory` in `config.toml`
- that the directory actually contains supported images
- that you ran `frostwall scan`

### “No screens detected”

FrostWall needs screen information from:

- `niri msg outputs`, or
- `wlr-randr`

### Thumbnails look wrong in Kitty

Use the safer protocol:

- press `i` inside the TUI, or
- set `[terminal].kitty_safe_thumbnails = true`

### `auto-tag` command is missing

Reinstall with CLIP enabled:

```bash
cargo install --path . --features clip
```

### Use a Different Wallpaper Directory for One Command

```bash
frostwall --dir ~/Pictures/wallpapers random
frostwall --dir ~/Pictures/wallpapers scan
frostwall --dir ~/Pictures/wallpapers auto-tag
```

## 7. Next Step

When FrostWall is working, read [Configuration Guide](CONFIGURATION.md) to tune matching, thumbnails, pairing, keybindings, and time profiles.
