# RayCore — Open Questions

Decisions that are deferred and will be resolved as separate tasks.

## Architecture

### Timeout and long-running tasks
How long can a single agent invocation run? What happens when a task exceeds the limit? Options:
- Hard timeout with partial response
- Sidecar-managed timeout with graceful shutdown signal
- Agent self-reports "task incomplete, needs continuation" as response

### Context window management
Agent loads conversation history from hydrated SQLite on every start. As history grows:
- Do we load the full history every time?
- Sliding window (last N messages)?
- Summary/compaction of older messages?
This directly impacts startup latency.

## Tooling

### Browser automation
The agent needs browser capabilities (test apps, interact with web services). Open:
- Headless Chrome/Playwright bundled in the base container image?
- External browser service accessed via MCP?
- Hybrid: lightweight browser in image, heavy browser via MCP?
- Impact on base image size and startup time?

### MCP server topology
MCP configs are per-user, prepared by sidecar. Open:
- Are MCP servers running inside the agent's container?
- Or external services the agent connects to at startup?
- Connection latency impact on single-shot execution model?
- Standard set of MCP servers vs fully user-customizable?

### Base image tool inventory
One universal image for all users. Final set of pre-installed runtimes TBD:
- Python, Node.js, git — likely yes
- Go, Java, Ruby — TBD
- System tools (curl, jq, ffmpeg, imagemagick) — TBD
- Size budget for the base image?

## Channels

### Input/output channel separation
Currently: input channel = output channel = session. Future enhancement:
- User sends message via Telegram, agent responds via Slack
- Use case: agent receives task from one system, delivers result to another
- See `docs/research/channel-input-output-separation.md` for prior research
- Priority: not immediate, but architecture should not preclude it

## Naming

### Full rename to RayCore
Binary, crate name, config keys, CLI commands — all currently `zeroclaw`. Full rename to `raycore` is planned. Scope:
- `Cargo.toml` package name
- Binary name (`zeroclaw` CLI → `raycore`)
- Config file name (`config.toml` keys referencing zeroclaw)
- Docker image name
- Environment variable prefixes (`ZEROCLAW_*` → `RAYCORE_*`)
- Internal string references and branding
