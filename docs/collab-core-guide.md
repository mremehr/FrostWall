# Collab Core Guide

Praktisk guide för nya samarbetsstacken i projektet.

## Delar

- `tools/collab-core`: realtime backend (chat/tasks/timeline/presence/observer)
- extern frame-producer: skriver bilder till observer-katalog

## Startordning (lokalt)

1. Se till att en extern frame-producer skriver bilder till `/tmp/frostwall-observer/frames`
eller sätt `COLLAB_OBSERVER_DIR` till katalogen du använder.

2. Starta backend:

```bash
cd /home/mrmattias/git/FrostWall/tools/collab-core
cargo run
```

3. Lista observer frames (för test):

```bash
curl -s http://127.0.0.1:7878/api/observer/frames
```

## Snabb verifiering

Health:

```bash
curl -s http://127.0.0.1:7878/health
```

Skapa chat:

```bash
curl -s -X POST http://127.0.0.1:7878/api/chat \
  -H 'content-type: application/json' \
  -d '{"user":"mrmattias","text":"hej team"}'
```

Skapa task:

```bash
curl -s -X POST http://127.0.0.1:7878/api/tasks \
  -H 'content-type: application/json' \
  -d '{"title":"Ship collab MVP","assignee":"mrmattias"}'
```

Lista observer frames:

```bash
curl -s http://127.0.0.1:7878/api/observer/frames
```

## WebSocket (`/ws`)

Vid connect skickar servern en initial `snapshot`, sedan events:

- `chat.created`
- `task.created`
- `task.updated`
- `timeline.created`
- `presence.updated`
- `observer.frame`

Eventformat:

```json
{
  "type": "chat.created",
  "data": {
    "id": 1,
    "user": "mrmattias",
    "text": "hej",
    "created_at_ms": 1771626560446
  }
}
```

## Miljövariabler

- `COLLAB_BIND` (default `127.0.0.1:7878`)
- `COLLAB_OBSERVER_DIR` (default `/tmp/frostwall-observer/frames`)
- `COLLAB_OBSERVER_SCAN_MS` (default `800`)

Exempel:

```bash
COLLAB_BIND=127.0.0.1:7999 \
COLLAB_OBSERVER_DIR=/tmp/frostwall-observer/frames \
COLLAB_OBSERVER_SCAN_MS=200 \
cargo run
```

## Status nu

- In-memory state (ingen DB än)
- Ingen auth än
- Local-first/dev-first design
