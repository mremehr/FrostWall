# Collab Core MVP

## Goal

Build a local-first collaboration backend that supports:

- realtime chat
- task tracking
- shared timeline
- user presence
- desktop observer frame events (from extern frame-producer)

MVP is optimized for speed and iteration, not long-term storage yet.

## Scope (MVP)

### Included

- HTTP API for CRUD-lite operations
- WebSocket realtime channel for all clients
- In-memory state snapshot
- Observer integration that watches `/tmp/frostwall-observer/frames`
- Local development deployment (`127.0.0.1`)

### Not included (yet)

- Auth/roles
- Persistent database
- Conflict-free offline sync
- File upload pipeline
- Multi-tenant isolation

## Domain Model

- `ChatMessage`: `{ id, user, text, created_at_ms }`
- `TaskItem`: `{ id, title, assignee?, status, created_at_ms, updated_at_ms }`
- `TimelineEvent`: `{ id, kind, text, created_at_ms }`
- `Presence`: `{ user, status, updated_at_ms }`
- `ObserverFrame`: `{ path, filename, size_bytes, modified_at_ms, observed_at_ms }`

`TaskStatus` values:

- `todo`
- `in_progress`
- `done`

## API Contract (MVP)

### Health

- `GET /health`

### Snapshot and lists

- `GET /api/state`
- `GET /api/chat`
- `GET /api/tasks`
- `GET /api/timeline`
- `GET /api/presence`
- `GET /api/observer/frames`

### Mutations

- `POST /api/chat`
  - body: `{ "user": "...", "text": "..." }`
- `POST /api/tasks`
  - body: `{ "title": "...", "assignee": "optional" }`
- `PATCH /api/tasks/:id/status`
  - body: `{ "status": "todo|in_progress|done" }`
- `POST /api/timeline`
  - body: `{ "kind": "...", "text": "..." }`
- `POST /api/presence`
  - body: `{ "user": "...", "status": "online|away|busy|offline" }`

### Realtime

- `GET /ws`
  - server sends initial `snapshot` event on connect
  - then pushes domain events as changes happen

## Realtime Event Types

- `snapshot`
- `chat.created`
- `task.created`
- `task.updated`
- `timeline.created`
- `presence.updated`
- `observer.frame`

## Observer Integration

Source:

- extern frame-producer writes frames into `/tmp/frostwall-observer/frames`

Behavior:

- backend scans directory on interval
- only new files become `observer.frame` events
- every observer frame also emits a timeline entry (`kind=observer`)

## Done Criteria (for this phase)

- backend starts and serves HTTP + WS
- all mutation endpoints emit realtime events
- observer frames appear in `/api/observer/frames`, timeline, and websocket stream
- `cargo check` passes for `tools/collab-core`
