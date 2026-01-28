# Contributing to FrostWall

Thank you for your interest in contributing to FrostWall!

## Development Setup

```bash
# Clone the repository
git clone https://github.com/mrmattias/frostwall.git
cd frostwall

# Build in debug mode
cargo build

# Run tests
cargo test

# Run with clippy (should have zero warnings)
cargo clippy --release

# Build release binary
cargo build --release
```

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy --release` and fix all warnings
- Use `&Path` instead of `&PathBuf` for function parameters
- Mark unused utility functions with `#[allow(dead_code)]`
- Add doc comments for public APIs

## Architecture

```
src/
├── main.rs        # CLI entry point, command routing
├── app.rs         # TUI state management, event loop
├── screen.rs      # Screen detection (niri/wlr-randr)
├── wallpaper.rs   # Wallpaper scanning, caching, filtering
├── swww.rs        # swww daemon interface
├── thumbnail.rs   # Thumbnail generation with SIMD
├── pywal.rs       # pywal color export
├── profile.rs     # Profile management
├── watch.rs       # Watch daemon
├── init.rs        # Interactive setup wizard
├── utils.rs       # Shared utilities
└── ui/
    ├── mod.rs     # UI module exports
    ├── theme.rs   # Frost theme colors
    └── layout.rs  # TUI rendering
```

## Key Dependencies

- `ratatui` + `crossterm` - TUI framework
- `ratatui-image` - Image rendering in terminal
- `image` + `fast_image_resize` - Image processing
- `kmeans_colors` + `palette` - Color extraction
- `rayon` - Parallel processing
- `notify` - File system watching
- `clap` - CLI argument parsing
- `serde` + `toml` - Configuration

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_aspect_categories

# Run with output
cargo test -- --nocapture
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run `cargo fmt` and `cargo clippy --release`
5. Add tests if applicable
6. Update documentation if needed
7. Commit your changes (`git commit -m 'Add amazing feature'`)
8. Push to the branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

## License

By contributing, you agree that your contributions will be licensed under the GPL-2.0 License.
