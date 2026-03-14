# RayCore — Production Readiness Roadmap

Status: **Active** | Created: 2026-03-14

## Phase 1 — Core Lifecycle [P0, blocks production]

### 1.1 Ephemeral channel mode

Current `channel start` is an infinite daemon loop. The vision requires single-shot: receive one message, process, respond, exit.

**Scope:**
- New command or flag: `raycore channel listen --once`
- Listen on configured channel, receive exactly one message
- Process message through full agent loop (tools, LLM, etc.)
- Respond to user through the same channel
- Publish completion status to Redis
- Exit cleanly

**Key files:** `src/channels/mod.rs` (line ~3809 `run_message_dispatch_loop`), `src/main.rs`

### 1.2 Wire status_connector into execution path

Status connector code exists (`src/status_connector/`) but is **not integrated** into the agent execution path.

**Scope:**
- Call `publish(Starting)` on agent startup
- Call `on_new_message()` when message arrives from channel
- Add periodic heartbeat: publish `Working` every N seconds during tool execution
- Call `publish(CompletedAwaiting)` before exit
- Feature-gate all integration behind `status-redis`

**Key files:** `src/status_connector/`, `src/agent/loop_.rs`, `src/channels/mod.rs`

### 1.3 Graceful shutdown

No SIGTERM/SIGINT handling exists. Container runtime sends SIGTERM on pod termination — agent must handle it.

**Scope:**
- `tokio::signal` handler for SIGTERM and SIGINT
- On signal: stop accepting new messages
- Finish in-flight message processing (or abort with partial response)
- Flush memory/SQLite to disk
- Publish final status to Redis
- Close connections (channel, MCP, Redis)
- Exit with clean exit code

**Key files:** `src/main.rs`, `src/daemon/mod.rs`, `src/channels/mod.rs`

---

## Phase 2 — Container & Image [P1, blocks production]

### 2.1 Base image for ephemeral mode

Current Dockerfile has two targets: `dev` (debian-slim) and `production` (distroless). The vision needs a rich runtime image with full toolkit.

**Scope:**
- Base image with: shell, Python, Node.js, git, curl, common CLI tools
- Browser automation: decision deferred (see `docs/open-questions.md`)
- Entrypoint: `raycore channel listen --once` (not `gateway`)
- Optimize layer caching for fast pulls
- Document image size budget

**Key files:** `Dockerfile`, `docker-compose.yml`

### 2.2 Startup time benchmark

Every millisecond of startup is user-visible latency. Need baseline measurement and optimization targets.

**Scope:**
- Measure: container start → config load → channel connect → ready to receive
- Identify bottlenecks (MCP connection, SQLite hydration, channel auth)
- Set target: < N ms from container start to message-ready
- Optimize critical path

---

## Phase 3 — Rename [P1, can follow Phase 1]

### 3.1 Full rename zeroclaw → raycore

**Scope:**
- `Cargo.toml`: package name `zeroclaw` → `raycore`
- Binary: `zeroclaw` → `raycore`
- Environment variables: `ZEROCLAW_*` → `RAYCORE_*`
- Config paths: `~/.zeroclaw/` → `~/.raycore/`
- Redis key prefix: `zeroclaw:agent` → `raycore:agent`
- Docker image: `ghcr.io/agentsray/zeroclaw` → `ghcr.io/agentsray/raycore`
- All internal string references, log messages, error messages
- README, docs, CLAUDE.md, AGENTS.md

---

## Phase 4 — Hardening [P2, non-blocking]

### 4.1 Dead code cleanup

- Remove 5 `#[allow(dead_code)]` suppressions and dead fields
- Remove OpenClaw migration code (`src/tools/openclaw_migration.rs`)
- Resolve undefined feature `ampersona-gates` (used in 5+ files, not declared in Cargo.toml)

### 4.2 Audit unwrap() in production paths

3121 `unwrap()` calls in codebase. Replace with explicit error handling in critical paths:
- HMAC operations in `src/channels/linq.rs` (security-critical)
- Channel initialization paths
- Config loading and parsing
- Provider request construction

### 4.3 TODO cleanup

- `src/security/pairing.rs:41,190` — mutex choice and async refactor
- `src/tools/browser_open.rs:136,187,208` — remove deprecated Brave fallback
- `src/hooks/builtin/webhook_audit.rs:128` — replace panic with graceful error

---

## Phase 5 — Future [P3, deferred]

Tracked in `docs/open-questions.md`:

- 5.1 Context window management (history size, sliding window, compaction)
- 5.2 Browser automation strategy (bundled vs MCP vs hybrid)
- 5.3 MCP server topology (in-container vs external)
- 5.4 Channel input/output separation (cross-channel routing)
- 5.5 Base image tool inventory (final set of pre-installed runtimes)

---

## Summary

| Phase | Scope | Blocks Production |
|---|---|---|
| Phase 1 | Core Lifecycle (ephemeral mode, status connector, shutdown) | **Yes** |
| Phase 2 | Container & Image (rich base image, startup benchmark) | **Yes** |
| Phase 3 | Rename zeroclaw → raycore | Partially |
| Phase 4 | Hardening (dead code, unwrap audit, TODOs) | No |
| Phase 5 | Future (context, browser, MCP, channels) | No |
