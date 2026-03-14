# Fork Verification Checklist

Status: **Active** | Created: 2026-03-14
Purpose: Verify all fork functionality is intact before fully departing from upstream (`zeroclaw-labs/zeroclaw`).

## Baseline Snapshot (2026-03-14)

| Metric | Value |
|---|---|
| Total tests | 4658 pass, 0 fail, 3 ignored (lib) |
| Integration tests | 106+ pass across 20 test crates |
| Clippy | 0 warnings (`-D warnings`) |
| Fmt | Clean |
| Previously failing tests | 2 — **fixed** (were upstream bugs, not fork regressions) |

## Fork-Specific Features

### Feature 1: Gemini Compatibility Layer

**Files:** `src/providers/compatible.rs`, `src/tools/schema.rs`, `src/config/schema.rs`

| Check | Command / Action | Status |
|---|---|---|
| Schema sanitizer tests | `cargo test test_remove_unsupported --lib` | Pass |
| Gemini compat detection | `cargo test gemini_compat --lib` | Pass |
| All compatible provider tests | `cargo test compatible --lib` (full module) | Pass |
| All schema tests | `cargo test tools::schema --lib` | Pass |
| Manual: Gemini via compatible provider | Config with `api_mode: "gemini_compat"` or auto-detect via `gemini-*` model name, send request, verify tool calls work | TODO |

**What to verify manually:**
- `SchemaCleanr::clean_for_gemini()` removes `default`, `null` enums, `additionalProperties`, flattens `anyOf/oneOf`
- `flatten_tool_history_for_gemini()` converts tool_calls to text `[Called tool ...]`
- `tool_choice: "auto"` is omitted for Gemini requests
- Auto-detection by model name prefix (`gemini-*`, `google/gemini-*`)

---

### Feature 2: Status Connector (Redis Sidecar)

**Files:** `src/status_connector/mod.rs`, `src/status_connector/redis_connector.rs`, `src/status_connector/traits.rs`

| Check | Command / Action | Status |
|---|---|---|
| Unit tests (default features) | `cargo test status_connector` | 8 pass |
| Compilation with `status-redis` | `cargo build --features status-redis` | Pass |
| Tests with `status-redis` | `cargo test --features status-redis status_connector` | Pass |
| Manual: Redis status publishing | Config `[sidecar_status]` with `enabled=true`, `redis_url`, verify `Starting`/`Working`/`CompletedAwaiting` lifecycle | TODO |

**What to verify manually:**
- Redis keys: `zeroclaw:agent:{agent_id}:{user_id}:status`
- JSON payload: `{"status":"...", "updated_at":"..."}`
- Idle timeout → `CompletedAwaiting` after configurable delay (default 30s)
- Graceful degradation when Redis unavailable

---

### Feature 3: MCP Tool Integration Unification

**Files:** `src/tools/mod.rs`, `src/agent/agent.rs`, `src/agent/loop_.rs`, `src/channels/mod.rs`

| Check | Command / Action | Status |
|---|---|---|
| Agent from_config async tests | `cargo test from_config --lib` | Pass |
| MCP helper compiles | Part of `cargo check` | Pass |
| Manual: `zeroclaw run` with MCP | Config `[mcp]` section with servers, verify tools registered in CLI mode | TODO |
| Manual: channel daemon with MCP | Start channel daemon, verify MCP tools available | TODO |
| Manual: gateway with MCP | Start gateway, verify MCP tools available via API | TODO |

**What to verify manually:**
- `extend_with_mcp_tools()` connects to all configured MCP servers
- Tools namespaced as `{server_name}__{tool_name}`
- Non-fatal: connection errors logged but don't block startup
- Works in all three entry points: CLI, gateway, channels

---

### Feature 4: History Echo Recovery (Gemini Parsing)

**Files:** `src/agent/loop_/parsing.rs`

| Check | Command / Action | Status |
|---|---|---|
| Parsing module tests | `cargo test loop_::parsing --lib` | Pass |
| Manual: Gemini echo recovery | Send conversation where Gemini echoes `[Called tool ...]` format, verify tool call is recovered | TODO |

**What to verify manually:**
- `parse_called_tool_format()` extracts tool name and args from `[Called tool \`name\` with: {args}]`
- Fallback fires only when primary parsers don't find tool calls
- `detect_tool_call_parse_issue()` flags responses with echo pattern

---

### Feature 5: Misc Fork Changes

| Check | Command / Action | Status |
|---|---|---|
| Makefile targets | `make help` | TODO |
| Memory retrieval changes | `cargo test memory::retrieval --lib` | Pass |
| Onboard wizard changes | `cargo test onboard --lib` | Pass |

---

## Phase 1: Static Analysis (All Automated)

| Check | Command | Status |
|---|---|---|
| Type check | `cargo check --all-targets` | Pass |
| Clippy | `cargo clippy --all-targets -- -D warnings` | Pass (0 warnings) |
| Format | `cargo fmt --all -- --check` | Pass (clean) |
| Release build | `cargo build --release` | TODO |
| Feature: status-redis | `cargo build --features status-redis` | Pass |
| Feature: hardware | `cargo build --features hardware` | TODO |
| Feature: channel-matrix | `cargo build --features channel-matrix` | TODO |
| Feature: channel-lark | `cargo build --features channel-lark` | TODO |

## Phase 2: Full Test Suite

| Check | Command | Status |
|---|---|---|
| Library tests | `cargo test --lib` | 4658 pass, 0 fail |
| Integration tests | `cargo test --tests` | All pass |
| Doc tests | `cargo test --doc` | Pass |

## Phase 3: Integration Test Crates

| Crate | Status |
|---|---|
| `circuit_breaker_integration` | Pass |
| `config_persistence` | Pass |
| `config_schema` | Pass |
| `dockerignore_test` | Pass |
| `e2e_circuit_breaker_simple` | Pass |
| `gemini_fallback_oauth_refresh` | Pass |
| `gemini_model_availability` | Pass |
| `hooks_integration` | Pass |
| `memory_comparison` | Pass |
| `memory_restart` | Pass |
| `openai_codex_vision_e2e` | Pass |
| `otel_dependency_feature_regression` | Pass |
| `provider_resolution` | Pass |
| `provider_schema` | Pass |
| `reliability_fallback_api_keys` | Pass |
| `reply_target_field_regression` | Pass |
| `stress_test_5min` | Ignored (long-running) |
| `stress_test_complex_chains` | Ignored (long-running) |
| `telegram_attachment_fallback` | Pass |
| `whatsapp_webhook_security` | Pass |

## Phase 4: CLI Smoke Tests

| Check | Command | Status |
|---|---|---|
| Help output | `cargo run -- --help` | TODO |
| Doctor diagnostics | `cargo run -- doctor` | TODO |
| Version output | `cargo run -- version` | TODO |

## Phase 5: CI Pipeline

| Check | Status |
|---|---|
| Workflow YAML validity | TODO |
| GitHub Actions run on branch | TODO |

## Phase 6: Documentation

| Check | Status |
|---|---|
| No broken links to upstream-specific resources | TODO |
| README doesn't reference upstream as canonical | TODO |
| All locale entry points navigable | TODO |

---

## Fixes Applied During Verification

### Fix 1: `audit_rejects_markdown_escape_links` (upstream bug)

**Root cause:** Test created a `../outside.md` link that stayed within the collection root (parent of skill dir). The cross-skill reference logic (`is_allowed_cross_skill_target`) correctly allowed it, but the test expected rejection.

**Fix:** Nested skill dir two levels deep (`skills/escape/`) and changed link to `../../outside.md` so it escapes beyond the collection root.

**File:** `src/skills/audit.rs`

### Fix 2: `medium_risk_create_requires_approval` (upstream bug)

**Root cause:** Double security validation — `ScheduleTool::handle_create_like()` validated with the correct `approved` flag, then called `cron::add_job()` which internally called `add_shell_job()` → `validate_shell_command(config, command, false)` with `approved=false`, causing the second check to always fail for medium-risk commands.

**Fix:** Added `_with_approval` variants for `cron::add_job`, `cron::add_once`, and `cron::add_once_at`. Changed `schedule.rs` to call these variants, passing through the `approved` flag consistently.

**Files:** `src/cron/mod.rs`, `src/tools/schedule.rs`
