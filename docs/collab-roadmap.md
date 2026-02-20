# Collab Roadmap

## Vision

Bygg ett lokalt, snabbt och säkert samarbetsverktyg som kan växa från intern MVP till stabil produkt.

## V2 (Core Stabilization)

Mål: gör nuvarande MVP robust för daglig användning.

- Lägg till persistens (SQLite) för chat/tasks/timeline/presence.
- Lägg till enkel token-auth för API/WS.
- Lägg till pagination/filter på list-endpoints.
- Lägg till integrationstester för API + observer-flöde.
- Lägg till bättre felkoder + standardiserat JSON-felsvar.

Exit-kriterier:

- Restart tappar inte state.
- End-to-end testsvit passerar i CI.
- Basic auth krävs på mutationer.

## V3 (Collab UX)

Mål: faktisk team-upplevelse i realtid.

- Bygg web-client (chat/task board/timeline/presence).
- Lägg till optimistic UI + reconnect-logik för WS.
- Lägg till notifieringar för task-changes och mentions.
- Lägg till observer-panel med senaste frames + refresh-kontroll.
- Lägg till audit-logg per user action.

Exit-kriterier:

- Minst två samtidiga klienter fungerar stabilt.
- Reconnect återhämtar snapshot + fortsätter events.
- Team kan köra dagligt workflow i verktyget.

## V4 (Automation + Intelligence)

Mål: automation som verkligen sparar tid.

- Regelmotor: auto-task/update baserat på observer-events.
- Agent hooks: trigger för sammanfattning och förslag.
- Prioriteringsmotor för task-triage.
- “Daily brief” med status, blockerare, nästa actions.
- Rollout-kontroller (feature flags).

Exit-kriterier:

- Mätbar tidsbesparing i teamflödet.
- Automation kan slås av/på säkert per feature.
- Inga kritiska regressionsbuggar i 2 veckors aktiv användning.

## Tekniska principer

- Local-first som default.
- Eventdriven arkitektur (HTTP mutations + WS fanout).
- Små, testbara moduler.
- Tydlig observability: logs, counters, healthchecks.
- Säkra defaults före “smart magic”.

## Nästa konkreta sprint

1. Lägg in SQLite + migrationer.
2. Lägg in enkel auth middleware.
3. Lägg in websocket integration test.
4. Lägg in web skeleton (read-only dashboard).
5. Lägg in release-notes/checklista.

