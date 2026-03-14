# RayCore — Vision

RayCore is a high-performance ephemeral agent runtime built in Rust. It is designed for a single operational cycle: **start, execute, respond, terminate**.

## Core Concept

RayCore is an operational unit for the end user. The user sends a message through a channel (Telegram, Slack, Discord, etc.), RayCore receives it, performs the requested task using its full toolkit, responds to the user through the same channel, and terminates.

The "Ray" analogy: fire and regenerate. Each invocation is a clean, isolated run. Environment continuity between runs is achieved by hydrating/dehydrating the agent's working directory — not by keeping the agent alive.

## Architecture

```
User ──► Channel (Telegram/Slack/...) ──► Inbound Event Handler ──► Sidecar ──► RayCore
                                                                        │
                                                                   Hydrate env
                                                                   Start agent
                                                                   Wait (Redis)
                                                                   Dehydrate env
                                                                   Terminate
```

### Components

| Component | Responsibility | Scope |
|---|---|---|
| **Inbound Event Handler** | Listens for user messages via channel webhooks. Triggers agent launch. | Internal infrastructure service (separate project). |
| **Sidecar** | Prepares environment: hydrates working directory, configs, sqlite files. Launches RayCore. Monitors Redis for health/completion. Dehydrates after completion. Terminates pod. | Internal infrastructure service (separate project). |
| **RayCore** | The agent itself. Receives message from channel, executes task, responds to user, signals completion via Redis, exits. | This project. |
| **Redis** | Health check and completion signaling between RayCore and Sidecar. | External dependency. |
| **LLM Gateway** | Internal gateway that routes LLM API requests to the appropriate provider. RayCore sends requests to a single URL; the gateway handles model routing. | Internal infrastructure service (separate project). |

### Lifecycle (single invocation)

1. User sends message to channel (e.g. Telegram)
2. Inbound Event Handler receives webhook, triggers Sidecar
3. Sidecar creates pod, hydrates working directory (configs, sqlite memory, workspace files)
4. Sidecar starts RayCore with the prepared config
5. RayCore publishes `Starting` status to Redis
6. RayCore connects to channel, receives the user's message
7. RayCore executes the task (tool calls, LLM reasoning, browser, shell, MCP, etc.)
8. RayCore periodically updates health check in Redis (`Working`)
9. RayCore responds to user through the same channel
10. RayCore publishes `CompletedAwaiting` status to Redis, exits
11. Sidecar detects completion, dehydrates working directory (persists sqlite, workspace)
12. Sidecar terminates pod

### State Management

| What | Where | When |
|---|---|---|
| Conversation history | SQLite (memory backend) | Hydrated before start, dehydrated after completion |
| Workspace files | Working directory | Hydrated/dehydrated by sidecar |
| Agent config | TOML config file | Prepared by sidecar per-user |
| MCP server configs | Part of agent config | Per-user, prepared by sidecar |
| Health/completion signal | Redis | Written by agent, read by sidecar |
| LLM provider credentials | Not in agent scope | Handled by internal LLM gateway |

## Design Principles

1. **Fast startup.** Rust binary, minimal initialization. Every millisecond of startup is user-visible latency.

2. **Single-shot execution.** One message in, one response out, terminate. No long-lived daemon state to manage or leak.

3. **Clean environment via destruction.** No cleanup logic needed — the container is destroyed. Next invocation gets a fresh container with hydrated state.

4. **Rich tooling.** The agent is the user's operational unit. It must support: shell commands, file operations, browser automation, MCP tools, web search, web fetch, and more. If the user asks "test my app" or "clean my email", the agent should have the tools to do it.

5. **Channel-native.** Input channel = output channel = session. The agent communicates directly with the user through their preferred channel.

6. **Stateless binary, stateful filesystem.** RayCore itself is stateless. All persistent state lives in the working directory, which the sidecar manages.

7. **One agent, one user, one config.** Each invocation serves exactly one user with one specific configuration.

## Constraints

- Agent runs inside a container with limited access: container-local tools + MCP
- No direct access to LLM provider credentials — all LLM traffic goes through internal gateway
- No ambient state — everything the agent needs must be in the hydrated working directory or config
- Container image is universal — one optimized base image for all users
- Agent must signal health/completion via Redis — sidecar depends on this contract

## What RayCore Is NOT

- Not a long-running daemon (that's the upstream zeroclaw model — we departed from it)
- Not a multi-tenant service (one agent = one user = one config)
- Not responsible for orchestration (sidecar does that)
- Not responsible for LLM provider management (gateway does that)
- Not responsible for channel webhook listening (Inbound Event Handler does that)
