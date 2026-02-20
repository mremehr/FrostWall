# DisplayFrost

Research and implementation baseline for wireless desktop streaming from Linux desktop to TV with built-in Chromecast and Miracast.

## Decision

Best solution for this project:

1. Build one Rust core for capture + encode + lifecycle.
2. Ship Chromecast path first (lowest delivery risk).
3. Build native Miracast control plane in Rust now, then iterate toward full sender pipeline.

This gives the fastest path to a stable product while still supporting both protocols over time.

## Why this decision

- Your previous script (`~/.local/bin/chromecast-screen`) already proves the Chromecast route works in your environment.
- Native Miracast is significantly more complex (Wi-Fi Direct + WFD/RTSP details), so this path is marked experimental and should be validated incrementally.
- A shared Rust core keeps the codebase clean and allows adding protocol adapters incrementally.

## Project layout

- `src/main.rs`: CLI entrypoint.
- `src/doctor.rs`: local dependency checks.
- `src/report.rs`: recommendation, roadmap, and source links.
- `src/chromecast.rs`: Chromecast discovery, stream lifecycle, cast control.
- `src/tui.rs`: terminal UI for selecting and starting streams.
- `docs/research.md`: protocol comparison and architecture choices.

## Quick start

```bash
cd /home/mrmattias/git/DisplayFrost
cargo run -- recommend
cargo run -- doctor
cargo run -- setup
cargo run -- roadmap
cargo run -- chromecast list
cargo run -- chromecast start "Living Room TV"
cargo run -- chromecast start --stream-only --port 8888
cargo run -- miracast discover
cargo run -- miracast connect --peer AA:BB:CC:DD:EE:FF
cargo run -- tui
```

Required for Chromecast MVP:

- `wf-recorder`
- `ffmpeg`
- `avahi-browse` (Avahi tools)
- `avahi-daemon`

Optional compatibility fallback (used automatically on some TVs):

- `rust_caster` (from `rust_cast`) for native Rust cast control
- `python3` + `pychromecast`
- `catt` (Python CLI backend)
- `wpa_cli`, `nmcli`, `iw` (native Miracast prerequisites)

Install `rust_caster`:

```bash
cargo install --git https://github.com/azasypkin/rust-cast --example rust_caster
```

Install `catt`:

```bash
pipx install catt
# or: pip3 install --user catt
```

Install `pychromecast`:

```bash
pip3 install --user pychromecast
```

Install native Miracast prerequisites:

```bash
# Arch
sudo pacman -S --needed wpa_supplicant networkmanager iw
```

## Device aliases

Create `~/.config/displayfrost/config.json` (or `$XDG_CONFIG_HOME/displayfrost/config.json`):

```json
{
  "chromecast": {
    "aliases": {
      "vardagsrum": "LG-SN-Series-ThinQ-S-b96f53d22d4ed2486d61148ba5cb1430",
      "shield": "SHIELD-Android-TV-96dda2ddabce56a57e5b2026d61fa801"
    }
  }
}
```

Then use aliases in commands:

```bash
cargo run -- chromecast start vardagsrum
cargo run -- chromecast stop vardagsrum
```

## Troubleshooting

- If cast attach fails at startup, DisplayFrost now keeps the HTTP stream alive and retries cast attach every 5 seconds.
- DisplayFrost waits a short initial warmup before first cast attach and now logs which backend actually succeeded.
- Cast attach is now confirmed against receiver `PLAYING` state; if one backend only buffers, DisplayFrost tries the next backend automatically.
- While it retries, test the stream URL printed in logs (for example `http://192.168.x.x:8888/stream.m3u8`) from another device on the LAN.
- If Firefox shows `Unable to connect`, confirm the `cargo run -- chromecast start ...` process is still running.
- Cast backend order is `castv2` -> `rust_caster` -> `python` -> `catt`. Available backends and active order are printed at stream start.

If discovery fails with daemon error, start Avahi:

```bash
sudo systemctl enable --now avahi-daemon
```

Setup helper:

```bash
cargo run -- setup --apply
```

Stop media manually:

```bash
cargo run -- chromecast stop "Living Room TV"
```

Advanced start example:

```bash
cargo run -- chromecast start \
  "Living Room TV" \
  --profile balanced \
  --encoder auto \
  --cast-backend castv2,rust_caster,python,catt \
  --auto-threshold 75 \
  --auto-samples 3 \
  --auto-interval 2 \
  --framerate 60 \
  --crf 18
```

`--encoder auto` starts with `libx264` and then tries `h264_vaapi`, `h264_nvenc`, `h264_amf` (if available) when CPU stays above threshold.
`--cast-backend` can be repeated or comma-separated to set fallback priority.

## Stream-only mode (for custom receiver app)

Run local HLS stream without Chromecast attach:

```bash
cargo run -- chromecast start --stream-only --port 8888
```

DisplayFrost prints URL like `http://<pc-ip>:8888/stream.m3u8`.  
Use that URL in your TV/receiver app (for example ExoPlayer).

## Miracast native commands

Current Miracast support uses native Rust control (experimental): Wi-Fi Direct discovery/connect plus RTSP preflight.

```bash
cargo run -- miracast discover
cargo run -- miracast connect --peer AA:BB:CC:DD:EE:FF
cargo run -- miracast status
cargo run -- miracast start --host <SINK_RTSP_IP>
cargo run -- miracast stop
```

`miracast discover` and `miracast connect` drive `wpa_cli` P2P flow.
`miracast start` performs native RTSP preflight (`OPTIONS`/`GET_PARAMETER`/`SET_PARAMETER`) and stores local session state in `~/.cache/displayfrost/miracast-native.state`.

## TUI controls

- `r`: refresh devices
- `j`/`k` or arrow keys: select device
- `a`: toggle audio on/off
- `e`: cycle encoder (`auto`, `libx264`, `h264_vaapi`, `h264_nvenc`, `h264_amf`)
- `p`: cycle profile (`low-latency`, `balanced`, `quality`)
- `+`/`-`: adjust framerate
- `[`/`]`: adjust CRF
- `Enter`: start streaming to selected device
- `s`: stop current stream
- `q`: quit (or stop stream + quit if currently streaming)

## Baseline reference

Existing script used as baseline for migration:

- `/home/mrmattias/.local/bin/chromecast-screen`
