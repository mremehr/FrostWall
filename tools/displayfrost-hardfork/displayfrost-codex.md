# DisplayFrost Codex Review

Datum: 2026-02-12

## Scope
- Las hela projektet (CLI, TUI, Chromecast/castv2, setup/doctor, docs).
- Korde lokal kvalitetskontroll (`cargo test`, `cargo clippy`, `cargo fmt --check`).
- Gjorde extern Chromecast-research mot primarkallor (Google Cast docs + etablerade implementationer).

## Snabb helhetsbild
Projektet ar pragmatiskt byggt och har en tydlig MVP-linje: Linux-capture (`wf-recorder`) -> packning/HTTP (`ffmpeg`) -> cast-kontroll med flera backends (`castv2`, `rust_caster`, `pychromecast`, `catt`).

Arkitekturen ar rimlig for snabb leverans, och fallback-mekaniken ar en tydlig styrka.

## Styrkor i koden
- Tydlig modulindelning: `src/main.rs`, `src/chromecast.rs`, `src/castv2.rs`, `src/tui.rs`, `src/config.rs`.
- Robust fallback-strategi for cast backends med verifiering av faktisk PLAYING-state (`src/chromecast.rs:1096`, `src/chromecast.rs:1144`).
- Bra livscykelhantering for processer och stop-signal (`src/chromecast.rs:496`, `src/chromecast.rs:1409`).
- Bra felkontext via `anyhow::Context` i stora delar av kodbasen.
- Relevanta enhetstester for parsing/konfig (`src/chromecast.rs:1505`, `src/config.rs:115`).

## Fynd som sticker ut (prioriterat)

### 1) Ambiguitet vid flera Chromecast med samma friendly name (Medel/Hog)
- Discovery deduperar enheter pa namn (`src/chromecast.rs:424`).
- Start/stop valjer enhet via namnmatch (`src/chromecast.rs:509`, `src/chromecast.rs:413`).
- Effekt: Fel TV kan valjas om tva enheter delar namn.
- Rekommendation: Identifiera enheter med stabilt ID (TXT `id`) som primarnyckel, visa namn som label.

### 2) castv2-koppling accepterar ogiltigt cert/hostname utan extra verifiering (Sakerhetsrisk, Medel)
- TLS byggs med `danger_accept_invalid_certs(true)` och `danger_accept_invalid_hostnames(true)` (`src/castv2.rs:279`).
- Detta ar funktionellt vanligt i tredjeparts Cast-klienter, men gor modellen svagare pa opalitliga LAN.
- Rekommendation: Dokumentera "trusted LAN" tydligt, och overvaga device-auth/challenge-verifiering for direkt `castv2`-backend.

### 3) TUI-volym kan blockera UI-traden (Medel)
- `adjust_volume` kor synkront i event-flodet (`src/tui.rs:303`).
- `castv2::set_volume` har upp till 10s connect-timeout (`src/castv2.rs:188`).
- Effekt: UI kan frysa vid natverksproblem.
- Rekommendation: Flytta volymanrop till worker-trad/async-kanal med kortare timeout.

### 4) Dokumentation driver ifran faktisk runtime (Liten/Medel)
- README-exempel visar stream-URL med `/stream.mp4` (`README.md:102`), men kod bygger URL utan path (`src/chromecast.rs:536`).
- README beskriver backend-ordning `python -> rust_caster -> catt -> castv2` (`README.md:104`), medan default i kod ar `castv2 -> rust_caster -> python -> catt` (`src/chromecast.rs:247`).
- Rekommendation: Synka README med faktisk default eller andom defaulten.

### 5) Kvalitetsgrindar ar inte helt "clean" (Liten)
- `cargo test --quiet`: OK (20/20 tester passerar).
- `cargo clippy --all-targets --all-features -- -D warnings`: failar (dead code i `get_volume`, large enum variant i `ChromecastCommand`).
- `cargo fmt -- --check`: failar (formateringsdrift).
- Rekommendation: Lagg in CI-jobb for `fmt`, `clippy`, `test` sa main alltid ar gron.

## Chromecast research: ar ni pa ratt vag?
Kort svar: Ja, i huvudsak.

### Det som matchar officiell modell
- Google Cast ar en sender->receiver-modell dar receiver laddar media via URL. Er modell (lokal stream URL + LOAD) ligger i linje med detta.
- Discovery via mDNS `_googlecast._tcp` ar ratt spar.
- Ni anvander "Default Media Receiver"-tank och LIVE media-load, vilket ar ett etablerat monster.

### Tekniska noteringar fran research
- MP4 med H.264-video och AAC/MP3-ljud ar inom Casts stodmatris.
- Google lyfter ocksa HLS/DASH (inkl. low-latency-profiler). For latens/kompatibilitet kan segmenterade protokoll vara nasta steg.
- Cast playback genom web stack kan paverkas av CORS-krav beroende pa receiver/path. Er ffmpeg inbyggda HTTP-server ar enkel och snabb, men detta ar en punkt att verifiera systematiskt.

## Slutsats
Ni forsoker ga i ratt riktning for ett pragmatiskt MVP-spor:
- Chromecast-forst ar rimligt.
- Adapter/fallback-tanket ar starkt.
- Den storsta kortsiktiga tekniska skulden ar enhetsidentifiering (namn-krockar), UI-blockering vid volymanrop, och drift mellan docs och faktisk runtime.

## Rekommenderad ordning pa nasta steg
1. Los device identity (stabilt ID i discovery + val/start/stop).
2. Synka README med verkligt beteende (URL-path + backend-default).
3. Gor volymkontroll icke-blockerande i TUI.
4. Satt CI-grindar for `fmt`/`clippy`/`test`.
5. Utvardera LL-HLS/CMAF eller DASH-profil som valbart alternativ om latens/kompatibilitet fortsatter vara problem.

## Kallor (online)
- Google Cast overview: https://developers.google.com/cast/docs/overview
- Media playback messages: https://developers.google.com/cast/docs/media/messages
- Supported media for Google Cast: https://developers.google.com/cast/docs/media
- iOS local network permissions/discovery (`_googlecast._tcp`): https://developers.google.com/cast/docs/ios_sender/permissions_and_discovery
- Discovery troubleshooting (mDNS responder): https://developers.google.com/cast/docs/discovery
- pychromecast source (service type + default media receiver app id): https://raw.githubusercontent.com/home-assistant-libs/pychromecast/master/pychromecast/discovery.py
- pychromecast constants (`APP_MEDIA_RECEIVER`, player states): https://raw.githubusercontent.com/home-assistant-libs/pychromecast/master/pychromecast/const.py
- node-castv2 reference (protocol namespaces, ports, implementation notes): https://www.npmjs.com/package/castv2

## Update 2026-02-12 (svart ruta + gra progressbar)

### Vad som gick fel
- Vi fick stabil cast-control men mediaspelaren fastnade i `BUFFERING`/`IDLE` med live-stream.
- Statisk MP4-fil till samma enhet spelade korrekt, sa grundnatverk + cast-styrning var inte huvudfelet.
- Online-kallor visar att Cast live primart ar byggt for HLS/DASH, och att adaptiva protokoll behover CORS.
- Vart tidigare upplagg var live fMP4 over `ffmpeg -listen 1`, vilket enligt ffmpeg-dokumentation ar experimentellt.

### Relevant online-stod
- Google Cast streaming protocols: HLS och DASH for live.
  - https://developers.google.com/cast/docs/media/streaming_protocols
- For HLS ska `contentType` vara `application/x-mpegurl`.
  - https://developers.google.com/cast/docs/media/streaming_protocols#hls
- Cast receiver kraver CORS for DASH/HLS/smooth/progressive.
  - https://developers.google.com/cast/docs/web_receiver/basic
- ffmpeg `listen` server ar markerad som experimentell.
  - https://ffmpeg.org/ffmpeg-protocols.html#http

### Aendring implementerad i kod
- Bytt stream-url till `http://<ip>:<port>/stream.m3u8`.
- Bytt pipeline till HLS playlist + MPEG-TS segment i temporar katalog.
- Lagt till lokal HTTP-server med CORS-headers for segment/playlist.
- Uppdaterat cast-load MIME-type till `application/x-mpegurl` i:
  - python fallback
  - rust_caster backend
  - intern castv2 LOAD payload
- Lagt till playlist-readiness-gate:
  - appen startar inte cast attach forran `stream.m3u8` faktiskt finns och innehaller segmentreferens.
  - samma gate kor efter eventuell encoder-switch/fallback.
  - detta adresserar observerad `GET /stream.m3u8 -> 404` direkt vid uppstart.
- Efter verifiering mot live-logg justerades gaten till fail-soft:
  - timeout hojdes till 45s
  - om playlist inte ar klar i tid loggas varning men sessionen fortsatter med cast-retries
  - libx264 sattes till `keyint=60` for snabbare segment/publicering
- HLS-servern refaktorerades till ren Rust (ingen extern `python3`-process for filservern).

### Verifiering efter patch
- `cargo check`: OK
- `cargo test --quiet`: OK (23/23)
- `cargo clippy --all-targets --all-features -- -D warnings`: OK

### Kvar att validera live pa riktig enhet
- Verifiera att enheten nu gor GET pa bade `stream.m3u8` och `segXXXXX.ts`.
- Om svart ruta kvarstar: nasta steg ar att tvinga mer Chromecast-kompatibel videoprofil/upplosning (t.ex. 1080p, H.264 Main, yuv420p, rimlig GOP).

## Update 2026-02-12 (Miracast re-research + projektlage)

### Vad ni/Claude faktiskt andrade senaste passen
- `12288fc`:
  - Ny Rust-baserad HLS HTTP-server i `src/chromecast.rs` (ingen extern python-filserver langre).
  - Stream URL bytt till `stream.m3u8` och MIME till `application/x-mpegurl`.
  - Playlist readiness-gate + retry-beteende for att minska tidiga 404.
  - Device identity har borjat hardnas med `id` fran Avahi TXT i `CastDevice`.
  - TUI-volym flyttad till worker-kanal (mindre risk for UI-freeze).
- `17cdd69`: default cast-backend order prioriterar `castv2` foran `rust_caster/python`.
- `6678cae`: backend fallback triggas om receiver inte faktiskt nar `PLAYING`.

Samlad bedomning: Chromecast-sparet har blivit mer robust och mer "egen Rust" i implementationen.

### Miracast research (primarkallor)

#### Ar Miracast opensource?
Kort svar: Delvis.
- Det finns open source-implementationer (ex: GNOME Network Displays, MiracleCast).
- Men Miracast ar en certifierad Wi-Fi Alliance-standard; full interoperabilitet handlar inte bara om kod, utan ocksa om stack/drivrutiner/certifierad beteendeprofil.

#### Linux-realitet 2026
- GNOME Network Displays ar aktivt och utvecklingen ligger pa GNOME GitLab:
  - https://gitlab.gnome.org/GNOME/gnome-network-displays
- Projektet beskriver flera harda beroenden i praktiken:
  - `wpa_supplicant` med P2P/Wi-Fi Display-stod
  - NetworkManager med P2P-stod
  - PipeWire + portalflode for capture
- `wpa_supplicant` visar explicit `CONFIG_WIFI_DISPLAY` i upstream-konfig:
  - https://android.googlesource.com/platform/external/wpa_supplicant_8/+/master/wpa_supplicant/defconfig

#### MiracleCast status
- MiracleCast ar open source, men README sager uttryckligen att source-laget inte ar klart:
  - https://github.com/albfan/miraclecast/blob/master/README.md
- Det gor den olamplig som ensam grund for "native sender MVP".

#### Miracast over Infrastructure (MS-MICE)
- Finns och ar relevant i enterprise-scenarion:
  - https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/miracast-over-infrastructure
- Men kraver att infrastruktur och sink faktiskt stoder profilen; kan inte antas i vanligt hemmalan.

### Slutsats for DisplayFrost
- Ni ar pa ratt vag med adapter-strategin.
- Det ar inte realistiskt att lova en stabil, native Miracast-sandare i kort MVP-horisont.
- Praktisk riktning:
  1. Hall Miracast som bridge-integration (GNOME ND-liknande path) bakom ett tydligt adapter-interface.
  2. Atan tillforlitlig drift/telemetri och supportmatris (chipset, drivrutin, wpa_supplicant/NM-version) innan native-forsok.
  3. Native Miracast sender som separat R&D-spor efter att Chromecast-path ar stabil i verklig drift.

### Konkret "gor detta nu"
1. Lag till `miracast` kommandoyta i CLI (utan native protocol implementation).
2. Implementera bridge-runner (start/stop/status) mot extern Miracast stack.
3. Definiera tydlig supportmatris i docs (GPU/driver/NM/wpa_supplicant/desktop portal).
4. Logga WFD-sessionsteg strukturerat sa ni kan felsoka verkliga tv-problem.
