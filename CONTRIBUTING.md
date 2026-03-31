# Contributing to FrostWall

Thanks for contributing.

## Development Setup

```bash
git clone https://github.com/mrmattias/frostwall.git
cd frostwall

cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

Optional feature builds:

```bash
cargo test --features clip
cargo test --features clip-cuda
```

## Expected Quality Bar

- run `cargo fmt` before committing
- keep `clippy` clean
- prefer `&Path` over `&PathBuf` in function signatures
- add tests for behavior changes
- update docs when user-facing behavior changes
- remove dead code instead of silencing it unless there is a real reason to keep it

## Repository Guide

### User-facing docs

- [README.md](README.md): fast onboarding and command overview
- [docs/USAGE.md](docs/USAGE.md): daily workflows and TUI usage
- [docs/CONFIGURATION.md](docs/CONFIGURATION.md): config reference

### Core source layout

```text
src/
  main.rs                  CLI entry point
  cli/                     clap args + top-level dispatch
  cli_cmds/                scriptable CLI command handlers
  app.rs                   app state container + module wiring
  app/                     TUI runtime, navigation, commands, actions, thumbnails
  ui/                      ratatui rendering and theme
  wallpaper.rs             wallpaper types + cache/model modules
  wallpaper/               cache persistence, scanning, matching, tags
  pairing.rs               pairing types + history/scoring modules
  pairing/                 learned pairing history + ranking helpers
  screen.rs                screen detection
  thumbnail.rs             disk thumbnail cache and resizing
  timeprofile.rs           time-based scoring
  watch.rs                 background rotation daemon
  collections.rs           saved multi-monitor presets
  profile.rs               named config profiles
  pywal.rs                 pywal export
  webimport.rs             Wallhaven / Unsplash import
  clip.rs                  optional CLIP tagging/inference
```

## Common Checks

```bash
# Full verification
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings

# Single test
cargo test test_aspect_categories

# With output
cargo test -- --nocapture

# Benchmarks
cargo bench
```

## Regenerating CLIP Embeddings

If you change the CLIP category source in `scripts/gen_embeddings.py`:

```bash
uv run --with torch --with transformers scripts/gen_embeddings.py
cargo build --features clip
cargo test --features clip
```

## Pull Requests

1. Create a branch.
2. Make the change.
3. Run the checks above.
4. Update docs if needed.
5. Open the PR with a clear summary of user-visible changes and risk.

## License

By contributing, you agree that your contributions are licensed under GPL-2.0.
