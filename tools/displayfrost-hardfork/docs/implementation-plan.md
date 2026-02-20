# Implementation Plan

## Target outcome

Single Rust toolchain for wireless desktop streaming, with both CLI and TUI workflows.

## MVP scope (Chromecast)

1. Discovery
   - scan `_googlecast._tcp` via mDNS
   - list friendly names + IP
2. Stream
   - start capture process (Wayland-friendly path)
   - produce stream URL accessible by TV
3. Control
   - connect to selected Chromecast
   - send `play media` command with stream URL
4. Lifecycle
   - health monitoring
   - clean stop on SIGINT/SIGTERM
5. UX
   - add TUI controls for discover/select/start/stop
6. Performance guardrail
   - auto-switch from `libx264` to `h264_vaapi` on sustained high CPU

## Phase 2 scope (Miracast bridge)

1. Add `miracast` transport adapter interface.
2. Integrate external Linux components first.
3. Unify config and status reporting between adapters.

## Guardrails

- Keep protocol logic isolated in adapter modules.
- Keep capture/encode path shared between adapters.
- Add structured logs early for easier latency debugging.
