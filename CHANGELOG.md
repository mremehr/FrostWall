# Changelog

All notable changes to FrostWall will be documented in this file.

## [Unreleased]

### Changed
- **Session persistence** - TUI now restores selection per monitor and reopens the last active screen
- **Protocol indicator UI** - Thumbnail protocol label is shown in header only (removed duplicate footer label)

## [0.5.0] - 2026-02-07

### Added
- **CLIP AI auto-tagging** - Semantic image tagging using CLIP ViT-B/32
  - 67 semantic categories (57 base + 10 library mixes)
  - Categories span nature, urban, abstract, style/era, mood, anime, fantasy/sci-fi, and composition
  - CUDA GPU acceleration via `--features clip-cuda`
  - SHA256 model verification after download
- **Visual pairing preview** - 50/50 split-view with real thumbnails for multi-monitor pairing
  - `p` key toggles pairing mode
  - `y` key cycles style mode (Off / Soft / Strict)
  - `1`-`0` keys jump to alternative index
- **Incremental rescan** - `R` key / `:rescan` command reloads wallpaper directory
  - Preserves all tags, auto-tags, CLIP embeddings, and pairing data
  - Only re-extracts colors for modified files
  - Reports added/removed/untagged counts
- **Terminal resize handling** - Thumbnail cache resets cleanly on resize
- **Dynamic thumbnail cache** - LRU cache scales with grid columns × 4
- **Cache versioning** - Automatic invalidation when cache format changes
- **Pairing commands** - `:pair-reset` and `:pair-rebuild` in command mode
- **Affinity auto-rebuild** - Pairing affinity scores rebuilt from history on startup
- **App struct refactoring** - 28-field App split into sub-structs (ThumbnailState, UiState, FilterState, PairingState, SelectionState)
- **109 unit tests** - Comprehensive coverage for utils, wallpaper, screen, pairing, timeprofile, clip_embeddings_bin
- **5 integration tests** - CLI smoke tests (help, version, random, scan)
- **Benchmark suite** - Criterion benchmarks for color operations

### Changed
- **Binary embeddings** - Replaced 13K lines of inlined Rust source with 114 KB binary format
- **Pairing scoring overhaul** - Fixed double-counting bug, normalized base scores to 0-1 range
- **Repetition penalty** - Increased from max 1.5 to max 8.0, lookback from 5 to 20
- **Strict mode** - Style bonus ×4.0, mismatch penalty ×6.0, history weight reduced to 0.15
- **Style tags expanded** - 29 style tags with canonical aliases for pairing
- **Affinity cleanup** - `prune_old_records()` now removes orphaned affinity entries

### Removed
- `clip_embeddings.rs` - Replaced by compact `clip_embeddings_bin.rs` + `data/embeddings.bin`
- `delta_e()` in utils.rs - Superseded by `delta_e_2000()`
- `from_path_with_existing()` in wallpaper.rs - Unused

## [0.4.0] - 2026-01-28

### Added
- **Configurable Keybindings**: Customize keyboard shortcuts via config file
  - Parse named keys (Enter, Tab, Esc, F1-F12, arrow keys, etc.)
  - Configurable: next, prev, apply, quit, random, toggle_match, toggle_resize, next_screen, prev_screen
- **Recursive Directory Scanning**: Scan subdirectories for wallpapers
  - Enable via `wallpaper.recursive = true` in config
  - Uses walkdir crate for efficient traversal
- **Graceful Shutdown**: Watch daemon handles Ctrl+C cleanly
  - Saves cache before exit
  - Proper signal handling via tokio

### Changed
- **Improved Cache Validation**: More robust cache invalidation
  - Checks 20 files instead of 5
  - Verifies modification timestamps
  - Detects file count changes in directory
- **Better Error Handling**: Errors displayed in UI instead of lost
  - `last_error` field shows issues in TUI
  - Apply/pywal errors properly reported
- **Fixed swww Output Parsing**: Exact match on output names
  - DP-1 no longer matches DP-10

### Removed
- **Preview Mode**: Removed due to swww not exposing current wallpaper path

### Fixed
- Random wallpaper now properly returns errors
- pywal export errors shown in UI
- Tag filter shows message when no tags defined

## [0.3.0] - 2026-01-28

### Added
- **pywal Integration**: Export dominant colors to `~/.cache/wal/` in multiple formats
  - `frostwall pywal <path>` CLI command
  - `w` key in TUI for one-shot export
  - `W` key to toggle auto-export on wallpaper apply
  - Generates: `colors.json`, `colors.sh`, `colors.Xresources`, `colors`
- **Color Filtering**: Filter wallpapers by dominant color in TUI
  - `C` key opens color picker popup
  - Visual color swatches with selection
  - Filter indicator in header
- **Parallel Scanning**: Multi-core wallpaper scanning with rayon
  - ~4x faster on quad-core systems
  - Progress indicator during scan
  - Parallel color space conversion

### Changed
- Optimized sorting: File size and modification time now cached in wallpaper metadata
- Improved string truncation: Safe char-boundary truncation prevents panics
- Better RNG: Using `rand` crate instead of system time seeding
- Cleaner code: Fixed all clippy warnings, improved type usage (`&Path` vs `&PathBuf`)

### Fixed
- Potential panic on narrow terminal widths during filename display
- Bounds check in thumbnail requests prevents index out of bounds

## [0.2.0] - 2026-01-28

### Added
- **Interactive Setup** (`frostwall init`): Guided configuration wizard
- **Profile System**: Multiple configuration profiles for different contexts
  - `frostwall profile {list,create,use,set,delete}` commands
- **Watch Daemon** (`frostwall watch`): Background wallpaper rotation
  - Configurable intervals (30m, 1h, etc.)
  - File system monitoring with inotify
  - Auto-updates cache when files change
- **Tagging System**: Organize wallpapers with tags
  - `frostwall tag {list,add,remove,show}` commands
  - Tag filtering in TUI (`t`/`T` keys)
- **Sorting Modes**: Sort by name, size, or date (`s` key)
- **Live Preview**: Preview wallpaper without committing (`p` key)
- **Help Popup**: Full keybinding reference (`?` key)
- **Color Palette Display**: View dominant colors (`c` key)

### Changed
- Version bump to 0.2.0
- Updated README with new features

## [0.1.0] - 2026-01-26

### Added
- Initial release
- TUI with image thumbnails (Kitty/Sixel protocols)
- Screen detection via niri/wlr-randr
- Aspect ratio matching (Ultrawide, Landscape, Portrait, Square)
- Match modes: Strict, Flexible, All
- Resize modes: Crop, Fit, Center, Stretch
- Dominant color extraction with k-means (5 colors)
- Thumbnail caching with SIMD acceleration
- swww integration with transition effects
- Dual theme support (Frostglow Light / Deep Cracked Ice Dark)
- TOML configuration
- CLI commands: random, next, prev, screens, scan
