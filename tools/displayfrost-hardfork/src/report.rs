pub fn recommendation() -> &'static str {
    r#"DisplayFrost recommendation (best practical solution)

1) Build a Rust orchestrator around one shared capture/encode path:
   - Screen capture via PipeWire/portal (Wayland-safe)
   - Hardware-friendly H.264 pipeline
2) Ship Chromecast first as MVP transport:
   - Discover via mDNS (_googlecast._tcp)
   - Cast a local low-latency stream URL to TV
3) Add Miracast as phase-2 bridge, not greenfield protocol rewrite:
   - Integrate existing Linux stack (GStreamer + GNOME Network Displays style path)
   - Keep protocol-specific logic behind adapters

Why this is best:
- Fastest path to stable desktop->TV result using your existing chromecast-screen experience.
- Avoids high-risk reimplementation of full Miracast/Wi-Fi Direct stack in early phases.
- Keeps architecture open for native Miracast sender work later.

Run:
  displayfrost doctor
  displayfrost roadmap
  displayfrost sources
"#
}

pub fn roadmap() -> &'static str {
    r#"DisplayFrost roadmap

Phase 0 (now): Research baseline and architecture
- Lock requirements: latency target, FPS target, audio sync tolerance.
- Validate local dependency set with `displayfrost doctor`.

Phase 1: Chromecast MVP in Rust
- Implement device discovery.
- Implement stream lifecycle (start ffmpeg/wf-recorder, cast URL, stop/cleanup).
- Implement simple CLI UX (list devices, pick, start/stop).

Phase 2: Stabilization
- Add bitrate/FPS profiles.
- Add auto encoder fallback (software -> VAAPI/NVENC/AMF).
- Add reconnect logic and health checks.

Phase 3: Miracast integration
- Add adapter that reuses mature Linux components instead of rewriting WFD from scratch.
- Normalize config and telemetry so Chromecast/Miracast share the same core pipeline.

Phase 4: Native Miracast sender R&D (optional)
- Only after MVP is stable and benchmarked.
"#
}

pub fn sources() -> &'static str {
    r#"Research sources

- Sunshine repository: https://github.com/LizardByte/Sunshine
- Sunshine GameStream docs: https://docs.lizardbyte.dev/projects/sunshine/v0.23.0/gamestream/gamestream.html
- Moonlight GameStream core: https://github.com/moonlight-stream/moonlight-common-c
- GNOME Network Displays (Miracast + Chromecast support): https://github.com/GNOME/gnome-network-displays
- xdg-desktop-portal ScreenCast API: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html
- Google Cast Web Receiver overview: https://developers.google.com/cast/docs/web_receiver
- Google Cast Media Playback guide: https://developers.google.com/cast/docs/media
- Miracast over infrastructure profile (MS-MICE): https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/miracast-over-infrastructure
"#
}
