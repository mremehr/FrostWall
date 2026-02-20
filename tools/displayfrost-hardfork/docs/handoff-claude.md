# DisplayFrost Handoff for Claude (2026-02-10)

## Current state

Chromecast streaming works in this environment, with Python backend confirmed as successful (`Streaming started via python`).

Recent work focused on:

- robust cast backend fallback and ordering
- config-based device aliases
- simpler CLI syntax for `start/stop`
- better shutdown behavior on Ctrl+C
- handling "gray progress bar / buffering forever" by confirming receiver `PLAYING` state

## Key commits (latest first)

- `6678cae` Confirm PLAYING state and fallback when cast backend buffers
- `6eedda3` Support positional Chromecast device argument
- `febdf71` Add config-based Chromecast device aliases
- `4527456` Always attempt cast stop on Ctrl+C shutdown
- `be5e8b7` Add catt fallback and configurable cast backend order
- `acd46bb` Fix Chromecast live streaming compatibility and pipeline defaults

## Behavioral changes to know

### Backend order and fallback

- Cast backend order is configurable via `--cast-backend`.
- Supported values: `python`, `rust_caster`, `catt`, `castv2`.
- If a backend attaches but playback does not reach `PLAYING`, DisplayFrost now treats it as failed and tries next backend.

Implementation references:

- `src/chromecast.rs` (`cast_play`, `wait_for_playing_state`)
- `src/castv2.rs` (`media_player_state`)

### Startup and streaming robustness

- Short warmup before first cast attach (`INITIAL_CAST_ATTACH_WARMUP_MS`).
- Low-latency ffmpeg flags added:
  - `-fflags nobuffer`
  - `-flags low_delay`
  - `-probesize 32`
  - `-analyzeduration 0`
  - `-muxdelay 0 -muxpreload 0 -flush_packets 1`

### Ctrl+C / shutdown

- On shutdown, app always attempts `cast_stop(...)` against current device (even if cast session was not fully confirmed).

### Device aliases

- Aliases are loaded from:
  - `$XDG_CONFIG_HOME/displayfrost/config.json`, or
  - `~/.config/displayfrost/config.json`
- Alias config shape:

```json
{
  "chromecast": {
    "aliases": {
      "vardagsrum-lg": "LG-SN-Series-ThinQ-S-b96f53d22d4ed2486d61148ba5cb1430",
      "shield-sovrum": "SHIELD-Android-TV-9e209d40e6e0bc0d7a2026ac84fdbea2"
    }
  }
}
```

### CLI syntax

Both forms now work:

- `displayfrost chromecast start shield-sovrum`
- `displayfrost chromecast start --device shield-sovrum`

Same for `stop`.

## Quick runbook

```bash
cargo run -- doctor
cargo run -- chromecast list
cargo run -- chromecast start shield-sovrum
```

Force backend order for debugging:

```bash
cargo run -- chromecast start shield-sovrum --cast-backend rust_caster,castv2,python
```

Stop:

```bash
cargo run -- chromecast stop shield-sovrum
```

## Known caveats

- In some sandboxed runs, `avahi-browse` returned `Access denied`; this is environment-specific and not necessarily a project bug.
- Rust backends (`rust_caster`/`castv2`) can still attach successfully but fail actual playback on some TVs; Python backend has been the most reliable in this setup.

## Suggested next tasks

- Add structured runtime telemetry line for cast state transitions (`BUFFERING` -> `PLAYING` etc.).
- Add optional timeout/threshold knobs for playback confirmation (`PLAYBACK_CONFIRM_TIMEOUT_SECS`, polling interval).
- Add small integration smoke test script (non-CI) that validates:
  - alias resolution
  - CLI parse modes
  - backend order fallback behavior
