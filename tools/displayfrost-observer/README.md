# DisplayFrost Observer

Minimal "fork-light" focused only on desktop observation for automation.

It wraps your existing `DisplayFrost` stream-only mode and adds fast frame capture.

## What it does

- Starts `displayfrost chromecast start --stream-only`
- Stores runtime state in `/tmp/displayfrost-observer`
- Captures one frame or a continuous frame stream from HLS URL
- Falls back to direct `grim` capture in `--mode auto` when HLS is unavailable

## Requirements

- `ffmpeg`
- Hard-fork source at `tools/displayfrost-hardfork` (default)
- Optional release binary at:
  `tools/displayfrost-hardfork/target/release/displayfrost`

Override binary path:

```bash
export DISPLAYFROST_BIN=/custom/path/displayfrost
```

Override source repo (for cargo fallback):

```bash
export DISPLAYFROST_REPO=/custom/path/DisplayFrost
```

## Usage

Start stream-only:

```bash
tools/displayfrost-observer/bin/observer-start --port 8888 --output DP-2
```

Capture one frame:

```bash
tools/displayfrost-observer/bin/observer-frame
```

Force direct screen capture (no HLS):

```bash
tools/displayfrost-observer/bin/observer-frame --mode grim
```

Capture continuous frames (2 fps, 30 frames):

```bash
tools/displayfrost-observer/bin/observer-loop --fps 2 --count 30
```

Force direct screen loop:

```bash
tools/displayfrost-observer/bin/observer-loop --mode grim --fps 1 --count 20
```

Stop stream:

```bash
tools/displayfrost-observer/bin/observer-stop
```

## Runtime files

- `/tmp/displayfrost-observer/stream.pid`
- `/tmp/displayfrost-observer/stream.log`
- `/tmp/displayfrost-observer/stream.url`
- `/tmp/displayfrost-observer/frames/`
