# MCP (Model Context Protocol) Integration Analysis

**Дата:** 27 февраля 2026  
**Статус:** Анализ текущего состояния и план работ

**Цель:** ZeroClaw должен **полностью** поддерживать работу с MCP-серверами, переданными через конфиг. ZeroClaw выступает только как **MCP-клиент** — режим MCP Server (ZeroClaw как сервер для внешних клиентов) не входит в текущую область.

---

## 1. Текущее состояние: почему MCP выглядит «не интегрированным»

### 1.1 Что уже реализовано

ZeroClaw **частично интегрировал** MCP как **клиент** (подключается к внешним MCP-серверам):

| Компонент | Назначение |
|-----------|------------|
| `src/tools/mcp_client.rs` | `McpRegistry` — подключение к нескольким MCP-серверам, получение списка tools, выполнение вызовов |
| `src/tools/mcp_protocol.rs` | JSON-RPC 2.0, `tools/list`, `tools/call`, `initialize` |
| `src/tools/mcp_transport.rs` | Транспорты: `stdio`, `HTTP`, `SSE` |
| `src/tools/mcp_tool.rs` | `McpToolWrapper` — адаптер под trait `Tool` |
| `src/config/schema.rs` | `McpConfig`, `McpServerConfig`, `McpTransport` |

Протокол: **2024-11-05**. Поддерживается только **tools** (инструменты). Resources и Prompts из спецификации MCP не реализованы.

### 1.2 Где MCP подключён

MCP tools **подключаются только в одном месте** — в channel daemon (`start_channels`):

```rust
// src/channels/mod.rs, ~4802
let mut built_tools = tools::all_tools_with_runtime(...);
if config.mcp.enabled && !config.mcp.servers.is_empty() {
    match crate::tools::McpRegistry::connect_all(&config.mcp.servers).await {
        Ok(registry) => {
            for name in registry.tool_names() {
                let wrapper = McpToolWrapper::new(...);
                built_tools.push(Box::new(wrapper));
            }
        }
        ...
    }
}
```

### 1.3 Где MCP не подключён

| Точка входа | Файл | Использует MCP? |
|-------------|------|-----------------|
| **CLI run** (`zeroclaw run`) | `src/agent/loop_.rs` | ❌ Нет |
| **Agent API** | `src/agent/agent.rs` | ❌ Нет |
| **Gateway** (OpenClaw-совместимый API) | `src/gateway/mod.rs` | ❌ Нет |
| **Channels daemon** (Telegram, Discord, webhook и т.п.) | `src/channels/mod.rs` | ✅ Да |

То есть MCP tools доступны **только** при работе через каналы (Telegram, Discord, webhook и т.д.). В CLI и gateway MCP не используется.

### 1.4 Документация и конфигурация

- Секция `[mcp]` **не описана** в `docs/config-reference.md`.
- В `docs/providers-reference.md` MCP упоминается в контексте Osaurus, а не ZeroClaw.
- В onboarding wizard (`src/onboard/wizard.rs`) используется `McpConfig::default()` — секция `[mcp]` не выводится пользователю.

### 1.5 Итог: почему кажется, что MCP «не интегрирован»

1. **Ограниченная область применения** — только channel daemon.
2. **Нет документации** по `[mcp]` и сценариям использования.
3. **Нет проверки через CLI** — `zeroclaw run` не получает MCP tools.
4. **Частичное покрытие спецификации** — только tools, без Resources и Prompts.

---

## 2. Варианты внедрения (в рамках MCP-клиента)

### Вариант A: Унификация MCP wiring (минимальный, низкий риск) — **ключевой**

Добавить MCP tools в общую функцию `all_tools_with_runtime()` или создать helper, который вызывается во всех точках сборки tools. Так MCP-серверы из конфига будут доступны во всех режимах работы (CLI run, gateway, channels).

**Плюсы:**
- Единый набор инструментов для CLI, gateway и channels — полная поддержка заданных MCP-серверов.
- Малое изменение — по сути дублирование уже написанного кода в channels в один общий модуль.

**Минусы:**
- `McpRegistry::connect_all()` — async; `all_tools_with_runtime()` сейчас синхронна по сборке tools. Нужна доработка сигнатуры или вынесение MCP в отдельный async-блок до/после.

**Файлы для изменения:**
- `src/tools/mod.rs` — добавить `extend_with_mcp_tools(config, tools) -> Result<Vec<Box<dyn Tool>>>` (или аналог).
- `src/agent/loop_.rs` — вызывать `extend_with_mcp_tools` после `all_tools_with_runtime`.
- `src/agent/agent.rs` — то же.
- `src/gateway/mod.rs` — то же (с учётом async-контекста).
- `src/channels/mod.rs` — заменить дублированный блок на вызов того же helper.

### Вариант B: Документация `[mcp]` (быстро, нулевой риск для кода)

Добавить в `docs/config-reference.md` секцию `## [mcp]` с описанием:

- `mcp.enabled`
- `mcp.servers` (name, transport, url, command, args, env, headers, tool_timeout_secs)
- Примеры для stdio, HTTP, SSE.

**Плюсы:** Пользователи смогут включать и настраивать MCP по документации.

### Вариант C: MCP Resources (средний объём работ)

Реализовать `resources/list`, `resources/read` и подмешивать содержимое ресурсов в контекст агента (RAG или system prompt).

**Плюсы:** Расширение возможностей MCP (файлы, БД и т.п. через ресурсы).

**Минусы:** Сложнее интеграция с текущей архитектурой контекста.

### Вариант D: MCP Prompts (средний объём работ)

Реализовать `prompts/list`, `prompts/get` и использовать MCP prompts как шаблоны для system prompt или пользовательских запросов.

**Плюсы:** Переиспользование промптов из MCP-серверов.

### Вариант E: Security policy для MCP tools (средний риск)

Сейчас `McpToolWrapper` не получает `SecurityPolicy`. Можно:
- Добавить обёртку, которая проверяет вызовы MCP tools через `SecurityPolicy`.
- Либо явно задокументировать, что MCP-серверы считаются доверенными.

---

## 3. План работ по внедрению

### Фаза 1: Полная поддержка MCP-серверов из конфига (1–2 PR)

Задача: любой заданный в `[mcp]` сервер должен работать во всех режимах (run, gateway, daemon), а не только в channels.

| # | Задача | Риск | Зависимости |
|---|--------|------|-------------|
| 1.1 | Добавить секцию `[mcp]` в `docs/config-reference.md` с примерами stdio/HTTP/SSE | Низкий | — |
| 1.2 | Вынести логику подключения MCP в общий helper (например, `tools::extend_with_mcp_tools`) | Низкий | — |
| 1.3 | Вызвать helper из `agent/loop_.rs`, `agent/agent.rs`, `gateway/mod.rs`, `channels/mod.rs` | Средний | 1.2 |
| 1.4 | Добавить unit/integration тест: MCP tools доступны при `zeroclaw run` с `[mcp]` в конфиге | Низкий | 1.3 |

### Фаза 2: Документация и onboarding (1 PR)

| # | Задача | Риск | Зависимости |
|---|--------|------|-------------|
| 2.1 | Упомянуть MCP в `docs/commands-reference.md` / runbook (если применимо) | Низкий | 1.1 |
| 2.2 | Добавить опциональный шаг onboarding wizard для `[mcp]` | Низкий | 1.1 |

### Фаза 3: Расширения протокола MCP-клиента (опционально)

| # | Задача | Риск | Зависимости |
|---|--------|------|-------------|
| 3.1 | MCP Resources — list/read, интеграция с контекстом агента | Средний | — |
| 3.2 | MCP Prompts — list/get, использование в system prompt | Средний | — |
| 3.3 | Security policy для MCP tools (если требуется) | Средний | Решение по политике |

---

## 4. Технические замечания

### Async vs sync

`McpRegistry::connect_all()` — async. Точки входа:

- **Channels:** уже async, можно вызывать напрямую.
- **Agent loop:** async (`run_agent_loop`).
- **Gateway:** async (Tokio).
- **Agent API:** async.

Проблема только в том, что `all_tools_with_runtime()` — синхронная функция, возвращающая `Vec<Box<dyn Tool>>`. Варианты:

1. Добавить async-helper `extend_tools_with_mcp(config, tools).await` и вызывать его в каждой точке **до** создания registry.
2. Или передавать `McpRegistry` отдельно и добавлять MCP tools в уже собранный список — как сейчас в channels.

### Префиксы имён

MCP tools получают имена вида `<server_name>__<tool_name>` (например, `filesystem__read_file`), чтобы избежать коллизий между серверами.

### Ошибки подключения

В channels сбой MCP обрабатывается как non-fatal: логируется ошибка, остальные tools остаются. Имеет смысл сохранить такое поведение и в остальных точках входа.

---

## 5. Ссылки

- MCP Specification 2024-11-05: https://modelcontextprotocol.io/specification/2024-11-05
- Текущая реализация: `src/tools/mcp_*.rs`
- Конфиг: `src/config/schema.rs` (McpConfig, McpServerConfig)
