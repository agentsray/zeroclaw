# Channels Connection Reference

Quick reference for agent/developer: all communication channels, config namespaces, connection parameters, and allowlist semantics. For full examples and troubleshooting see [channels-reference.md](channels-reference.md).

## Overview

| Channel | Config key | Receive mode | Public inbound? | Allowlist field | Allowlist format |
|---------|------------|--------------|-----------------|-----------------|------------------|
| CLI | `channels_config.cli` | stdin/stdout | No | — | — |
| Telegram | `telegram` | polling | No | `allowed_users` | User ID (numeric) or username (no `@`); `*` = all |
| Discord | `discord` | gateway/websocket | No | `allowed_users` | Discord user ID; `*` = all |
| Slack | `slack` | Events API | No | `allowed_users` | Slack user ID (e.g. `U01234ABCD`); `*` = all |
| Mattermost | `mattermost` | polling | No | `allowed_users` | Mattermost user ID; `*` = all |
| Matrix | `matrix` | sync API (E2EE supported) | No | `allowed_users` | Matrix ID (`@user:server`); `*` = all |
| Signal | `signal` | signal-cli HTTP/SSE | No | `allowed_from` | E.164 phone; `*` = all |
| WhatsApp | `whatsapp` | webhook (Cloud) or websocket (Web) | Cloud: Yes | `allowed_numbers` | E.164; `*` = all |
| Linq | `linq` | webhook | Yes | `allowed_senders` | E.164; `*` = all |
| WATI | `wati` | webhook | Yes | `allowed_numbers` | E.164; `*` = all |
| Nextcloud Talk | `nextcloud_talk` | webhook | Yes | `allowed_users` | Nextcloud actor ID; `*` = all |
| Webhook | `webhook` | gateway HTTP | Usually yes | — | Gateway pairing / `X-Webhook-Secret` |
| Email | `email` | IMAP poll + SMTP | No | `allowed_senders` | Email address or domain; `*` = all |
| IRC | `irc` | IRC socket | No | `allowed_users` | Nickname (case-insensitive); `*` = all |
| Lark | `lark` | websocket or webhook | Webhook only | `allowed_users` | Open/union ID; `*` = all |
| Feishu | `feishu` | websocket or webhook | Webhook only | `allowed_users` | Open/union ID; `*` = all |
| DingTalk | `dingtalk` | stream | No | `allowed_users` | Staff ID; `*` = all |
| QQ Official | `qq` | webhook (default) or websocket | Webhook: Yes | `allowed_users` | QQ user ID; `*` = all |
| Napcat | `napcat` (alias `onebot`) | websocket | No | `allowed_users` | QQ user ID; `*` = all |
| GitHub | `github` | webhook | Yes | `allowed_repos` | `owner/repo`, `owner/*`, or `*` |
| BlueBubbles | `bluebubbles` | webhook | Yes (or tunnel) | `allowed_senders` | Phone or Apple ID; `*` = all |
| iMessage | `imessage` | AppleScript (macOS) | No | `allowed_contacts` | Phone or email; `*` = all |
| Nostr | `nostr` | relay websocket | No | `allowed_pubkeys` | Hex or npub; `*` = all |
| ACP | `acp` | stdio JSON-RPC | No | `allowed_users` | Protocol-defined ID; `*` = all |
| ClawdTalk | `clawdtalk` | Telnyx webhook | Yes | `allowed_destinations` | E.164 or pattern |

Empty allowlist = deny all (except where pairing/bind flow applies, e.g. Telegram).

---

## Per-channel parameters

### Telegram — `[channels_config.telegram]`

- **Required:** `bot_token`, `allowed_users`.
- **Optional:** `base_url` (Telegram-compatible API), `stream_mode`, `draft_update_interval_ms`, `mention_only`, `group_reply`, `interrupt_on_new_message`, `ack_enabled`, `progress_mode`.
- **Note:** Empty `allowed_users` enables one-time pairing: bind code on startup, user sends `/bind <code>` in Telegram.

### Discord — `[channels_config.discord]`

- **Required:** `bot_token`, `allowed_users`.
- **Optional:** `guild_id` (restrict to one server), `listen_to_bots`, `mention_only`, `group_reply`.

### Slack — `[channels_config.slack]`

- **Required:** `bot_token`, `allowed_users`.
- **Optional:** `app_token` (Socket Mode), `channel_id` or `channel_ids` (omit/`*` = all channels), `group_reply`.

### Mattermost — `[channels_config.mattermost]`

- **Required:** `url`, `bot_token`, `allowed_users`; `channel_id` required for listening.
- **Optional:** `thread_replies`, `mention_only`, `group_reply`.

### Matrix — `[channels_config.matrix]`

- **Required:** `homeserver`, `access_token`, `room_id`, `allowed_users`.
- **Optional:** `user_id`, `device_id` (recommended for E2EE), `mention_only`.
- **Build:** `channel-matrix` feature (opt-in). See [matrix-e2ee-guide.md](matrix-e2ee-guide.md) for E2EE.

### Signal — `[channels_config.signal]`

- **Required:** `http_url` (signal-cli daemon), `account` (E.164), `allowed_from`.
- **Optional:** `group_id` (`"dm"` = DMs only; specific ID = one group; omit = all), `ignore_attachments`, `ignore_stories`.

### WhatsApp — `[channels_config.whatsapp]`

- **Cloud API:** `access_token`, `phone_number_id`, `verify_token`, `allowed_numbers`; optional `app_secret`.
- **Web mode** (feature `whatsapp-web`): `session_path`, `allowed_numbers`; optional `pair_phone`, `pair_code`.
- Only one mode per config; Cloud takes precedence if both set.

### Linq — `[channels_config.linq]`

- **Required:** `api_token`, `from_phone`, `allowed_senders`.
- **Optional:** `signing_secret` (webhook HMAC). Env: `ZEROCLAW_LINQ_SIGNING_SECRET`.

### WATI — `[channels_config.wati]`

- **Required:** `api_token`, `webhook_secret`, `allowed_numbers`.
- **Optional:** `api_url`, `tenant_id`. Env: `ZEROCLAW_WATI_WEBHOOK_SECRET`.

### Nextcloud Talk — `[channels_config.nextcloud_talk]`

- **Required:** `base_url`, `app_token`, `allowed_users`.
- **Optional:** `webhook_secret`. Env: `ZEROCLAW_NEXTCLOUD_TALK_WEBHOOK_SECRET`. See [nextcloud-talk-setup.md](nextcloud-talk-setup.md).

### Webhook — `[channels_config.webhook]`

- **Required:** `port`.
- **Optional:** `secret` (shared secret for verification). Auth: gateway pairing and/or `X-Webhook-Secret`.

### Email — `[channels_config.email]`

- **Required:** `imap_host`, `imap_port`, `smtp_host`, `smtp_port`, `username`, `password`, `from_address`, `allowed_senders`.
- **Optional:** `imap_folder`, `smtp_tls`, `idle_timeout_secs`, `imap_id` (RFC 2971; some providers require it).

### IRC — `[channels_config.irc]`

- **Required:** `server`, `port`, `nickname`, `channels`, `allowed_users`.
- **Optional:** `username`, `server_password`, `nickserv_password`, `sasl_password`, `verify_tls`.

### Lark — `[channels_config.lark]`

- **Required:** `app_id`, `app_secret`, `allowed_users`.
- **Optional:** `encrypt_key`, `verification_token`, `mention_only`, `group_reply`, `use_feishu`, `receive_mode` (`websocket` | `webhook`), `port` (webhook), `draft_update_interval_ms`, `max_draft_edits`.
- **Build:** `channel-lark` feature (default). Webhook mode needs public HTTPS.

### Feishu — `[channels_config.feishu]`

- **Required:** `app_id`, `app_secret`, `allowed_users`.
- **Optional:** Same as Lark (no `use_feishu`); `receive_mode`, `port` for webhook, `group_reply`, `draft_update_interval_ms`, `max_draft_edits`.
- **Build:** `channel-lark` feature.

### DingTalk — `[channels_config.dingtalk]`

- **Required:** `client_id`, `client_secret`, `allowed_users`.

### QQ Official — `[channels_config.qq]`

- **Required:** `app_id`, `app_secret`, `allowed_users`.
- **Optional:** `receive_mode` (`webhook` | `websocket`), `environment` (`production` | `sandbox`).

### Napcat (OneBot) — `[channels_config.napcat]` or `[channels_config.onebot]`

- **Required:** `websocket_url`, `allowed_users`.
- **Optional:** `api_base_url` (derived from WS URL if omitted), `access_token`.

### GitHub — `[channels_config.github]`

- **Required:** `access_token`, `allowed_repos`.
- **Optional:** `webhook_secret`, `api_base_url` (GHES).

### BlueBubbles — `[channels_config.bluebubbles]`

- **Required:** `server_url`, `password`, `allowed_senders`.
- **Optional:** `webhook_secret`, `ignore_senders`.

### iMessage — `[channels_config.imessage]`

- **Required:** `allowed_contacts`.
- **Platform:** macOS only (AppleScript bridge).

### Nostr — `[channels_config.nostr]`

- **Required:** `private_key`, `allowed_pubkeys`.
- **Optional:** `relays` (wss URLs). Supports NIP-04 and NIP-17.

### ACP — `[channels_config.acp]`

- **Required:** `allowed_users`.
- **Optional:** `opencode_path`, `workdir`, `extra_args`. Runs `opencode acp` over stdio (JSON-RPC 2.0).

### ClawdTalk — `[channels_config.clawdtalk]`

- **Required:** `api_key` (Telnyx), `connection_id`, `from_number`, `allowed_destinations`.
- **Optional:** `webhook_secret`.

---

## Group-chat policy (Telegram, Discord, Slack, Mattermost, Lark, Feishu)

- `[channels_config.<channel>.group_reply]`: `mode` = `all_messages` | `mention_only`; `allowed_sender_ids` = IDs that bypass mention gate.
- Allowlist (`allowed_users`) is always enforced first; `allowed_sender_ids` only affects mention gating in groups.

---

## Build features

- **Matrix:** `channel-matrix` (opt-in). Without it, `matrix` config is ignored.
- **Lark/Feishu:** `channel-lark` (default). Without it, `lark`/`feishu` configs are ignored.
- **WhatsApp Web:** `whatsapp-web` for `session_path`-based mode.

```bash
# Minimal (no Matrix/Lark)
cargo build --no-default-features --features hardware

# With Matrix
cargo build --no-default-features --features hardware,channel-matrix
```

---

## See also

- [channels-reference.md](channels-reference.md) — full config examples, delivery modes, troubleshooting.
- [config-reference.md](config-reference.md) — global and gateway config.
- [network-deployment.md](network-deployment.md) — polling vs webhook, public URL requirements.
