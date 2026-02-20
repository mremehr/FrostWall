# DisplayFrost Research

Date: 2026-02-08

## Goal

Wireless desktop streaming from Linux computer to TV with Chromecast and Miracast available.

## Baseline from previous project

Your existing script `/home/mrmattias/.local/bin/chromecast-screen` does:

- capture with `wf-recorder`
- stream packaging with `ffmpeg` HTTP listen
- discovery with `avahi-browse`
- cast control via `pychromecast`

This validates the MVP direction for Chromecast.

## Protocol reality check

### Sunshine

- Sunshine is a host for Moonlight/GameStream ecosystem, not a Chromecast/Miracast sender.
- It is useful as quality reference (low latency, hardware encode workflows), but not a direct protocol match for this project.

### Chromecast

- Official Cast model is sender app instructing receiver app to load media.
- Practical engineering path: host low-latency local media stream and command Cast receiver to play it.
- This matches your current script and is the shortest path to Rust MVP.

### Miracast

- Miracast/Wi-Fi Display stack is more complex and typically includes protocol layers that are harder to implement robustly.
- Linux implementations exist (for example GNOME Network Displays) and demonstrate that integration is feasible through existing components.

## Recommended architecture

1. Shared core in Rust:
   - capture session
   - encoder/transcoder process management
   - device discovery abstraction
   - stream lifecycle and cleanup
2. Transport adapter `chromecast` (MVP):
   - mDNS discovery
   - cast session control
   - start/stop/reconnect logic
3. Transport adapter `miracast` (Phase 2):
   - bridge to mature Linux stack first
   - keep the adapter boundary stable so native implementation can replace bridge later

## Why not native Miracast in MVP

- Higher protocol complexity and higher delivery risk.
- Harder to test across heterogeneous TV firmware.
- Slower path to a usable result compared with proven Chromecast baseline.

## Sources

- Sunshine repo: <https://github.com/LizardByte/Sunshine>
- Sunshine GameStream docs: <https://docs.lizardbyte.dev/projects/sunshine/v0.23.0/gamestream/gamestream.html>
- Moonlight common core: <https://github.com/moonlight-stream/moonlight-common-c>
- GNOME Network Displays: <https://github.com/GNOME/gnome-network-displays>
- xdg-desktop-portal ScreenCast API: <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html>
- Google Cast Web Receiver docs: <https://developers.google.com/cast/docs/web_receiver>
- Google Cast media docs: <https://developers.google.com/cast/docs/media>
- Miracast over infrastructure (MS-MICE): <https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/miracast-over-infrastructure>

