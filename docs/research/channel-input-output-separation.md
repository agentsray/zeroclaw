# Research: Channel Input/Output Separation

**Date:** 2026-03-04
**Status:** Research / Proposal
**Scope:** `src/channels/`, `src/config/schema.rs`, `src/gateway/`, `src/cron/`

---

## 1. Problem Statement

Currently a ZeroClaw channel is a monolithic abstraction: a single `Channel` trait that combines **input** (listening for messages) and **output** (sending replies). Reply routing is hardcoded: the output always goes back to the same channel that received the message.

This prevents several useful scenarios:

- **Cross-channel routing**: receive from GitHub webhook, reply in Slack.
- **Fan-out**: one incoming message triggers replies to multiple output channels (e.g. Telegram + Discord).
- **Input-only channels**: receive from a sensor/webhook with no reply capability, route output elsewhere.
- **Output-only channels**: a notification-only channel (e.g. email digest) that never listens.
- **Multiple instances of one type**: two Telegram bots or two Slack workspaces in one runtime.

---

## 2. Current Architecture

### 2.1 Channel Trait (`src/channels/traits.rs`)

The `Channel` trait bundles input and output:

```rust
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;                                          // identity
    async fn send(&self, message: &SendMessage) -> Result<()>;       // OUTPUT
    async fn listen(&self, tx: Sender<ChannelMessage>) -> Result<()>; // INPUT
    async fn health_check(&self) -> bool;
    async fn start_typing(&self, recipient: &str) -> Result<()>;     // output-adjacent
    async fn stop_typing(&self, recipient: &str) -> Result<()>;      // output-adjacent
    fn supports_draft_updates(&self) -> bool;                        // output-adjacent
    async fn send_draft(&self, message: &SendMessage) -> Result<Option<String>>;
    async fn update_draft(...) -> Result<Option<String>>;
    async fn finalize_draft(...) -> Result<()>;
    async fn cancel_draft(...) -> Result<()>;
    async fn send_approval_prompt(...) -> Result<()>;                // output-adjacent
    async fn add_reaction(...) -> Result<()>;                        // output-adjacent
    async fn remove_reaction(...) -> Result<()>;                     // output-adjacent
}
```

**Observation:** 12 out of 14 methods are output-related. Only `listen()` is pure input. `name()` is identity.

### 2.2 Message Flow

```
Channel.listen() → tx.send(ChannelMessage) → mpsc bus → run_message_dispatch_loop
    → process_channel_message(ctx, msg)
        → target_channel = ctx.channels_by_name.get(&msg.channel)   // same channel!
        → [LLM + tools] → response
        → target_channel.send(SendMessage::new(response, &msg.reply_target))
```

The output channel is resolved by `msg.channel` — a string set by the listener (e.g. `"telegram"`). It is always the same channel that produced the input.

### 2.3 Config Schema (`src/config/schema.rs`)

`ChannelsConfig` has one `Option<XxxConfig>` per channel type:

```rust
pub struct ChannelsConfig {
    pub cli: bool,
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub slack: Option<SlackConfig>,
    // ... one slot per type, no arrays, no named instances
}
```

This means:
- Maximum one instance per channel type.
- No concept of "input channel" vs "output channel" in config.
- No routing configuration between channels.

### 2.4 Gateway Webhook Handlers (`src/gateway/mod.rs`)

Webhook-based channels (GitHub, WhatsApp, Linq, BlueBubbles, WATI, Nextcloud Talk, QQ) bypass `start_channels` entirely:

```
HTTP POST /github → handle_github_webhook()
    → github.parse_webhook_payload()
    → run_gateway_chat_with_tools()    // LLM call
    → github.send(SendMessage::new(response, &msg.reply_target))
```

Each handler constructs a channel instance inline and replies on it. There is no access to other channels.

### 2.5 Existing Patterns That Already Separate Input/Output

| Pattern | Location | Description |
|---------|----------|-------------|
| **Cron delivery** | `src/cron/scheduler.rs` | Agent runs with no input channel; output goes to `delivery.channel` + `delivery.to`. Input ≠ output by design. |
| **Heartbeat delivery** | `src/daemon/mod.rs` | Sends to a configured `target` channel. No input channel. |
| **Goal loop delivery** | `src/config/schema.rs` | `channel` + `target` fields for output. No input. |
| **ACP `response_channel`** | `src/channels/acp.rs` | ACP can set a separate `response_channel: Option<Arc<dyn Channel>>` to route output to a different channel. |
| **Hooks `on_message_sending`** | `src/channels/mod.rs` | Hook receives `(channel, recipient, content)` and can return modified values. **However, channel/recipient changes are explicitly blocked** — only content mutation is applied. |

**Key insight:** The codebase already has output-only delivery (`cron`, `heartbeat`, `goal_loop`) and one instance of explicit input/output separation (`ACP`). The hooks system was designed with cross-channel routing in mind but intentionally blocks it.

---

## 3. Proposed Design

### 3.1 Core Concept: Split `Channel` into `ChannelInput` + `ChannelOutput`

```rust
/// Input-only: listens for incoming messages
#[async_trait]
pub trait ChannelInput: Send + Sync {
    fn name(&self) -> &str;
    async fn listen(&self, tx: Sender<ChannelMessage>) -> Result<()>;
    async fn health_check(&self) -> bool { true }
}

/// Output-only: sends messages, manages typing, drafts, reactions
#[async_trait]
pub trait ChannelOutput: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, message: &SendMessage) -> Result<()>;
    async fn health_check(&self) -> bool { true }
    async fn start_typing(&self, recipient: &str) -> Result<()> { Ok(()) }
    async fn stop_typing(&self, recipient: &str) -> Result<()> { Ok(()) }
    fn supports_draft_updates(&self) -> bool { false }
    async fn send_draft(&self, message: &SendMessage) -> Result<Option<String>> { Ok(None) }
    async fn update_draft(&self, recipient: &str, message_id: &str, text: &str) -> Result<Option<String>> { Ok(None) }
    async fn finalize_draft(&self, recipient: &str, message_id: &str, text: &str) -> Result<()> { Ok(()) }
    async fn cancel_draft(&self, recipient: &str, message_id: &str) -> Result<()> { Ok(()) }
    async fn send_approval_prompt(&self, recipient: &str, request_id: &str, tool_name: &str, arguments: &Value, thread_ts: Option<String>) -> Result<()>;
    async fn add_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> { Ok(()) }
    async fn remove_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> { Ok(()) }
}
```

### 3.2 Backward Compatibility: Keep `Channel` as Blanket

```rust
/// Full channel — implements both input and output.
/// Existing channel implementations continue to use this.
pub trait Channel: ChannelInput + ChannelOutput {}

/// Blanket implementation
impl<T: ChannelInput + ChannelOutput> Channel for T {}
```

All existing channels (Telegram, Discord, Slack, etc.) continue implementing the full `Channel` by implementing both sub-traits. No existing code breaks.

### 3.3 Config Changes

#### 3.3.1 Output Routing Table (new section)

```toml
# New config section: output routing rules
[channel_routing]
enabled = true

# Default: reply on the same channel (current behavior)
default = "same"

# Per-input-channel overrides
[channel_routing.rules.github]
output = ["slack"]                     # GitHub input → Slack output
default_recipient = "C01234ABCD"       # Slack channel ID

[channel_routing.rules.telegram]
output = ["telegram", "discord"]       # Fan-out: reply on both
# recipients inferred from msg.reply_target for same-channel,
# explicit for cross-channel:
discord_recipient = "123456789012345678"
```

#### 3.3.2 Named Channel Instances (future, not in phase 1)

```toml
# Future: multiple instances of the same type
[channels_config.telegram.main]
bot_token = "..."
allowed_users = [...]

[channels_config.telegram.ops]
bot_token = "..."
allowed_users = [...]
```

This is a larger config schema change and should be deferred.

### 3.4 Message Routing Changes

In `process_channel_message`, replace:

```rust
// Current
let target_channel = ctx.channels_by_name.get(&msg.channel).cloned();
```

With:

```rust
// Proposed
let output_channels = ctx.resolve_output_channels(&msg);
// Returns Vec<(Arc<dyn ChannelOutput>, String /* recipient */)>
// Default: vec![(same_channel, msg.reply_target)]
```

### 3.5 Hook System Update

Unblock `on_message_sending` channel routing — currently it is explicitly blocked:

```rust
// Current (src/channels/mod.rs:3894)
if hook_channel != msg.channel || hook_recipient != msg.reply_target {
    tracing::warn!(
        "on_message_sending attempted to rewrite channel routing; only content mutation is applied"
    );
}
```

With output routing enabled, this restriction can be relaxed for configured routing rules while keeping the safety guard for hooks that attempt arbitrary rewrites.

---

## 4. Potential Problems and Blockers

### 4.1 `reply_target` Semantics Are Platform-Specific (BLOCKER)

`reply_target` is the core connection between input and output. Its format is platform-specific:

| Channel | `reply_target` format |
|---------|----------------------|
| Telegram | `chat_id` or `chat_id:thread_id` |
| Discord | `channel_id` |
| Slack | `channel_id` |
| GitHub | `owner/repo#issue_number` |
| Signal | E.164 phone number |
| Email | email address |
| IRC | `#channel` or nickname |

**Problem:** When routing from GitHub → Slack, the `reply_target` `"owner/repo#42"` is meaningless to Slack. The output channel needs a valid Slack channel ID.

**Solution:** The routing config must include explicit `default_recipient` per output channel, or a mapping function. For cross-channel routing, `reply_target` from the input is not used; the config-specified recipient is used instead. The original `reply_target` can be included in the message content as context.

### 4.2 `thread_ts` Semantics Are Platform-Specific

`thread_ts` is used for threaded replies (Slack `ts`, Discord thread ID, GitHub comment ID). Cross-channel routing cannot preserve threading semantics — threads don't map between platforms.

**Solution:** For cross-channel output, `thread_ts` is dropped. Optionally, the output message content includes threading context as text (e.g. "In reply to GitHub PR #42 comment").

### 4.3 Conversation History Isolation

Conversation history is keyed by `{channel}_{sender}` or `{channel}_{thread_ts}_{sender}`. With cross-channel routing, the question arises: should history follow the input or the output?

**Solution:** History remains keyed by **input channel + sender**. The output channel is a delivery detail; the conversation context is defined by where the user is talking.

### 4.4 Draft/Typing Indicators for Multiple Outputs

If a message fans out to multiple output channels, `start_typing`, `send_draft`, `update_draft` need to run on all output channels in parallel.

**Solution:** The typing task spawner already takes a channel reference. For fan-out, spawn one typing task per output channel. Draft updates require tracking per-output-channel draft message IDs.

### 4.5 Approval Flow Is Channel-Scoped

Approval prompts (`send_approval_prompt`) and responses (`/approve-allow`, `/approve-deny`) assume the user interacts on the same channel. Cross-channel routing means the approval prompt might go to a different channel than where the user can respond.

**Solution:** Keep approval prompts on the **input channel** (where the user can respond), not the output channel. Only the final response is routed to output channels.

### 4.6 Gateway Webhook Handlers Bypass Channel Registry

Webhook handlers (GitHub, WhatsApp, etc.) construct channels inline and don't have access to `channels_by_name`. To support cross-channel output from webhooks, the gateway needs access to the live channel registry.

**Solution:** Extend `AppState` with `channels_by_name` (or use the existing `get_live_channel()` function). This requires `start_channels` or daemon mode to be running alongside the gateway.

### 4.7 Outbound Leak Guard / Sanitization

`sanitize_gateway_response` and `sanitize_channel_response` run before sending. With multiple output channels, sanitization should run once and the sanitized result sent to all outputs.

**Solution:** Sanitize once, send the sanitized content to all output channels. No architectural change needed.

### 4.8 ACK Reactions

ACK reactions (emoji reactions on incoming messages) are currently scoped to the input channel. This is correct and should remain unchanged with cross-channel routing.

### 4.9 Multiple Instances of Same Channel Type

The current `ChannelsConfig` struct uses `Option<TelegramConfig>` — one slot per type. Named instances require a schema change to `HashMap<String, TelegramConfig>` or similar.

**Blocker:** This is a significant config migration with backward-compatibility implications. Defer to a later phase.

---

## 5. Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Breaking existing channel implementations | High | Keep `Channel` trait as blanket over `ChannelInput + ChannelOutput`; all existing impls unchanged |
| `reply_target` mismatch across platforms | High | Require explicit `default_recipient` in routing config for cross-channel rules |
| Config migration complexity | Medium | Add new `[channel_routing]` section; existing config unchanged |
| Gateway handler isolation from channel registry | Medium | Wire `get_live_channel()` into `AppState`; requires daemon/channel mode |
| Approval flow confusion | Medium | Keep approvals on input channel; only route final response |
| Conversation history fragmentation | Low | Key history by input, not output |
| Test coverage for cross-channel paths | Medium | Add integration tests for each routing rule combination |

---

## 6. Concrete Work Plan

### Phase 1: Trait Split (Foundation)

**Goal:** Split `Channel` into `ChannelInput` + `ChannelOutput` without changing any behavior.

1. Define `ChannelInput` and `ChannelOutput` traits in `src/channels/traits.rs`.
2. Define `Channel` as a supertrait of both: `trait Channel: ChannelInput + ChannelOutput {}`.
3. Update all channel implementations to implement both sub-traits (mechanical refactor — move `listen()` to `ChannelInput` impl, move `send()` + output methods to `ChannelOutput` impl).
4. Keep all call sites using `dyn Channel` or `Arc<dyn Channel>` unchanged.
5. Validate: `cargo test`, `cargo clippy`, no behavior change.

**Estimated scope:** ~25 files (one per channel + traits + mod.rs). Mechanical change, low risk.

### Phase 2: Output Channel Registry

**Goal:** Allow runtime to look up output channels independently from input channels.

1. Add `outputs_by_name: HashMap<String, Arc<dyn ChannelOutput>>` to `ChannelRuntimeContext` (initially populated from the same channels).
2. Add `get_live_output_channel(name: &str) -> Option<Arc<dyn ChannelOutput>>` to the global registry.
3. Extend `AppState` in gateway with access to the output channel registry.
4. Validate: no behavior change, but output channels are now independently addressable.

**Estimated scope:** `src/channels/mod.rs`, `src/gateway/mod.rs`. Small change.

### Phase 3: Routing Config + Engine

**Goal:** Add configurable routing rules.

1. Define `ChannelRoutingConfig` in `src/config/schema.rs`:
   ```rust
   pub struct ChannelRoutingConfig {
       pub enabled: bool,
       pub rules: HashMap<String, ChannelRoutingRule>,
   }
   pub struct ChannelRoutingRule {
       pub output: Vec<String>,
       pub recipients: HashMap<String, String>,
   }
   ```
2. Add `[channel_routing]` to `Config`.
3. Implement `resolve_output_channels(&msg) -> Vec<OutputTarget>` in `src/channels/mod.rs`.
4. Default behavior (no config or `default = "same"`): returns `vec![(same_channel, msg.reply_target)]`.
5. When routing rules exist: resolve output channels from `outputs_by_name`, substitute recipients from config.
6. Validate: with default config, no behavior change. With routing config, messages route correctly.

**Estimated scope:** `src/config/schema.rs`, `src/channels/mod.rs`. Medium change.

### Phase 4: Wire Routing into Message Processing

**Goal:** Use routing engine in the main message processing path.

1. In `process_channel_message`: replace `target_channel = ctx.channels_by_name.get(&msg.channel)` with `resolve_output_channels()`.
2. For single output (default): no change in behavior.
3. For multiple outputs (fan-out): send to each output channel sequentially or in parallel.
4. Handle typing indicators for multiple outputs.
5. Keep approval prompts on input channel only.
6. Update `on_message_sending` hook to receive resolved output targets.
7. Validate: integration tests for same-channel, cross-channel, and fan-out scenarios.

**Estimated scope:** `src/channels/mod.rs` (process_channel_message is ~900 lines). High complexity, requires careful testing.

### Phase 5: Gateway Webhook Cross-Channel Output

**Goal:** Allow webhook handlers to route output to configured channels.

1. In each webhook handler (`handle_github_webhook`, etc.): after `run_gateway_chat_with_tools`, resolve output channels from routing config instead of always using the input channel.
2. Use `get_live_output_channel()` to look up output channels.
3. Fallback: if routing config absent or output channel not running, use the input channel (current behavior).
4. Validate: GitHub → Slack routing works when both are configured.

**Estimated scope:** `src/gateway/mod.rs` (7 webhook handlers). Medium complexity.

### Phase 6 (Future): Named Channel Instances

**Goal:** Support multiple instances of the same channel type.

1. Migrate `ChannelsConfig` from `Option<TelegramConfig>` to `HashMap<String, TelegramConfig>` (or a backwards-compatible wrapper).
2. Update `collect_configured_channels` to produce named instances.
3. Update `channels_by_name` keying from `channel.name()` to instance name.
4. Update `ChannelMessage.channel` to carry instance name.
5. Config migration for existing single-slot configs.

**Estimated scope:** Large schema change, all channel instantiation code, config migration. Defer until phase 1–5 are stable.

---

## 7. Out of Scope

- **Message transformation between channels** (e.g. converting Markdown to Slack blocks). Each channel's `send()` already handles formatting; the content is plain text.
- **Bidirectional bridging** (forwarding messages between two channels like a relay). This is a different feature (channel bridge/relay) with different UX implications.
- **Per-message dynamic routing** (user chooses output channel at message time). Routing is config-driven, not per-message.

---

## 8. Dependencies and Prerequisites

- No new external dependencies required.
- Phase 1 is a pure refactor with zero behavior change — safest to land first.
- Phase 2–5 can be landed incrementally; each phase is independently useful.
- Phase 6 requires a config migration strategy and should be planned separately.

---

## 9. Success Criteria

1. **Phase 1:** All existing tests pass. `Channel` trait split is invisible to callers.
2. **Phase 3:** `zeroclaw channel start` with `[channel_routing]` config correctly routes messages.
3. **Phase 5:** GitHub webhook input → Slack output works in daemon mode.
4. **Overall:** Default behavior (no routing config) is identical to current behavior — zero regression.
