# collab-core

Local-first realtime backend for collaboration.

Project docs:

- `/home/mrmattias/git/FrostWall/docs/collab-mvp.md`
- `/home/mrmattias/git/FrostWall/docs/collab-core-guide.md`

## Features (MVP)

- chat
- tasks
- timeline
- presence
- observer frame ingestion from `/tmp/frostwall-observer/frames`
- websocket event stream (`/ws`)

## Run

```bash
cd /home/mrmattias/git/FrostWall/tools/collab-core
cargo run
```

Default bind:

- `127.0.0.1:7878`

Environment variables:

- `COLLAB_BIND` (default `127.0.0.1:7878`)
- `COLLAB_OBSERVER_DIR` (default `/tmp/frostwall-observer/frames`)
- `COLLAB_OBSERVER_SCAN_MS` (default `800`)

## API

- `GET /health`
- `GET /api/state`
- `GET /api/chat`
- `POST /api/chat`
- `GET /api/tasks`
- `POST /api/tasks`
- `PATCH /api/tasks/{id}/status`
- `GET /api/timeline`
- `POST /api/timeline`
- `GET /api/presence`
- `POST /api/presence`
- `GET /api/observer/frames`
- `GET /ws`

## Minimal curl flow

```bash
curl -s http://127.0.0.1:7878/health
curl -s -X POST http://127.0.0.1:7878/api/chat \
  -H 'content-type: application/json' \
  -d '{"user":"mattias","text":"hej team"}'
curl -s -X POST http://127.0.0.1:7878/api/tasks \
  -H 'content-type: application/json' \
  -d '{"title":"Ship MVP","assignee":"mattias"}'
curl -s -X PATCH http://127.0.0.1:7878/api/tasks/1/status \
  -H 'content-type: application/json' \
  -d '{"status":"in_progress"}'
```
