# LLM Logger — Implementation Plan

**Assignment:** Ollive AI — Fullstack Engineer Assignment
**Goal:** LLM chatbot + auto-instrumented inference logging + ingestion pipeline + storage + dashboards
**Deadline:** Saturday night
**Repo:** `~/skills/inferwatch`

---

## Stack

| Layer | Choice | Why |
|---|---|---|
| Backend | Rust + Axum + Tokio | Strength, stands out, event-based via tokio channels |
| DB | PostgreSQL + SQLx (migrations) | Assignment explicitly cares about schema design |
| LLM calls | reqwest → OpenAI REST API (start), trait for multi-provider | No SDK lock-in, trait = multi-provider bonus |
| Streaming | SSE (Server-Sent Events) | Axum native, client read-only stream |
| Events | tokio mpsc channel → ingestion worker | Event-based architecture (bonus), non-blocking ingest |
| Frontend | SvelteKit + TS | Chat UI, conversation list/resume, dashboard |
| Deploy | docker-compose up (bonus) | one-command setup |

---

## Architecture

```
Svelte UI ──HTTP/SSE──► Axum routes ──► SDK wrapper (InstrumentedProvider)
                              │              │           │
                              │         OpenAI call   LogEvent → tx
                              │              │           │
                              │   ◄── stream tokens ◄───┘ (SSE passthrough)
                              │                            │
                        ingestion_worker (tokio::spawn) ◄── rx
                              │
                        PostgreSQL: conversations, messages, inference_logs
```

## Data flow (one message)

1. POST /chat/:conv_id { message, session_id }
2. Load history from DB → build messages array
3. sdk.complete_stream(messages) → InstrumentedProvider
   - calls OpenAI with stream=true
   - measures latency (first token + total)
   - accumulates tokens
   - fires ONE LogEvent into channel at end (non-blocking)
4. Tokens streamed to client via SSE immediately
5. chat handler saves user + assistant messages to DB
6. Worker receives LogEvent → inserts into inference_logs

---

## DB Schema

**conversations:** id UUID PK, session_id UUID (browser identity), title, created_at, updated_at
**messages:** id UUID PK, conversation_id FK→conversations ON DELETE CASCADE, role, content, created_at
**inference_logs:** id UUID PK, conversation_id FK, message_id FK, model, provider, latency_ms, input_tokens, output_tokens, status, error_msg, input_preview, output_preview, created_at

Design decisions (for README):
- session_id not users table: assignment doesn't need auth; tradeoff documented
- inference_logs links message_id: one assistant reply = one inference call = one log row
- previews capped at 500 chars: avoid storing full payloads (PII/cost tradeoff)
- indexes on session_id, conversation_id, created_at for list/dashboard queries

---

## API

| Method | Route | Purpose |
|---|---|---|
| POST | /api/conversations | create conversation |
| GET | /api/conversations?session_id= | list user's conversations |
| GET | /api/conversations/:id | get one + messages (resume) |
| POST | /api/chat/:conv_id | send message → SSE stream reply |
| GET | /api/metrics | latency/throughput/error aggregates |
| GET | /api/logs | recent inference logs (dashboard table) |
| GET | /health | health check |

---

## Build Order

- [x] 1. Scaffold: backend (cargo) + folders + .env
- [x] 2. Migrations 001–003 (above schema, idempotent)
- [x] 3. errors.rs — AppError enum + IntoResponse
- [x] 4. state.rs — AppState { db pool, sdk }
- [x] 5. providers/mod.rs — LlmProvider trait, StreamChunk, BoxStream
- [x] 6. providers/openai.rs — reqwest + SSE chunk parsing + usage capture
- [x] 7. sdk/mod.rs — InstrumentedProvider: timing, previews, try_send LogEvent
- [x] 8. ingestion/worker.rs — rx loop → db insert
- [x] 9. db/*.rs — query functions per table
- [x] 10. routes/conversations.rs — create/list/get(+messages)
- [x] 11. routes/chat.rs — history, placeholder assistant row, SSE stream (port 3001)
- [x] 12. routes/metrics.rs — summary aggregates, latency buckets, recent logs
- [x] 13. main.rs — wiring: pool, migrations, channel, worker spawn, CORS, routes
  (tested: health, conv CRUD, error path logged end-to-end)
- [x] 14. Frontend scaffold (sv create, minimal TS)
- [x] 15. Chat UI — send message, render SSE stream token-by-token (manual SSE parse over POST)
- [x] 16. Conversation list + resume (load messages by id via ?c=<id>)
- [x] 17. Dashboard — summary cards, SVG latency chart, logs table (5s live refresh)
- [x] 18. Dockerfile + docker-compose.yml (db + backend + frontend) — verified: one-command stack, real streams through containers
- [x] 19. Multi-provider: Gemini + OpenAI behind LlmProvider trait, env-switched (LLM_PROVIDER)
- [x] 20. README: setup, architecture, schema decisions, tradeoffs, scaling, future work
- [ ] 21. Demo: screenshots / Loom video
- [ ] 22. Submit to work@ollive.ai

## Rename → inferwatch + publish phase
- [x] Rename llm-logger → inferwatch (all source, compose, README, dir)
- [x] Fresh `inferwatch` local DB + migrations applied
- [x] Bug fix: `AVG(latency_ms)` decoded garbage → `::float8` cast in metrics.rs (both queries), `.sqlx` regenerated
- [x] Local smoke test: compile ✓, health ✓, Gemini stream ✓, log row ✓, metrics ✓
- [x] Docker compose verified end-to-end post-rename (fresh volume; frontend 200 via 127.0.0.1:5173)
- [x] Repo hygiene: secrets scan clean, removed boilerplate frontend/README.md
- [ ] Push to GitHub: pratikgiramkar1 (personal) + pratikjuspay (work)

Stretch (documented as tradeoffs, not built): cancel conversation mid-stream endpoint, PII redaction, self-hosted k8s.

---

## Failure handling (for README)

- OpenAI down → stream emits error event, LogEvent status=error, worker logs it
- Channel full (10k buffer) → drop log + tracing::warn (never block chat path)
- DB down in worker → retries with backoff are possible; documented as improvement
- Graceful shutdown → drain channel before exit (worker finishes in-flight events)

## Scaling notes (for README)

- Single worker sequential → multi-worker / batch inserts / Kafka+consumer groups
- mpsc in-memory → durable queue (Kafka/Redis Streams) if delivery guaranteed
- Postgres fine to ~thousands req/s for this schema; read replicas for dashboards
