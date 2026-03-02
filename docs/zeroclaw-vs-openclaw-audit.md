# Аудит: ZeroClaw vs OpenClaw — рассуждения и AI-функции

Отчёт составлен по результатам аудита репозитория openclaw/openclaw через MCP DeepWiki и анализа кодовой базы ZeroClaw.

**Дата:** 27 февраля 2026.

### Условия сравнения

**Встроенный UI не учитывается.** Сравнение ведётся с точки зрения headless/API-ориентированного использования: CLI, интеграция через каналы (Telegram, Discord, webhook), sidecar-агенты, edge-deployment. Web dashboard, Control UI, macOS app и прочие готовые интерфейсы не рассматриваются как критерий выбора.

---

## 1. Архитектура

| Аспект | OpenClaw | ZeroClaw |
|--------|----------|----------|
| **Язык** | TypeScript / Node.js | Rust |
| **Агент-рантайм** | `pi-agent-core` (pi-coding-agent SDK) | Собственный trait-driven оркестратор |
| **RAM** | > 1 GB (Node.js) | \< 5 MB |
| **Старт** | > 500 s на слабом железе | \< 10 ms |

---

## 2. Возможности рассуждений (Reasoning)

### OpenClaw — более развитая поддержка reasoning

- **Thinking levels** (`off`, `low`, `medium`, `high`, `xhigh`):
  - приоритеты: inline directive → session override → глобальный дефолт
  - поддержка моделей с reasoning (deepseek-v3.2, qwen3-235b)
  - отдельная настройка для Z.AI (`on`/`off`)
- **Директивы**:
  - `/think:medium` и др.
  - `/reasoning on|off|stream` — отдельная доставка reasoning пользователю
  - в `stream` (Telegram) reasoning показывается в draft bubble во время генерации
- **Структурированный вывод**:
  - тег `think` для рассуждений
  - тег `final` для финального ответа
- **Интеграция**:
  - `reasoningTagHint` в системном промпте
  - `onReasoningStream`, `onReasoningEnd`
  - `isReasoningTagProvider` для Gemini, MiniMax
  - события `thinking_start`, `thinking_delta`, `thinking_end`

### ZeroClaw — базовая поддержка reasoning

- **Конфиг**:
  - `provider.reasoning_level` / `runtime.reasoning_level`
  - уровни: minimal, low, medium, high, xhigh
- **Провайдеры**:
  - чтение `reasoning_content` (OpenAI-compatible, Ollama, Gemini, OpenRouter)
  - `strip_think_tags` для моделей, выносящих CoT в content
  - `clamp_reasoning_effort` для Codex
- **Отсутствует**:
  - директивы `/think`, `/reasoning`
  - отдельный стриминг reasoning
  - управление видимостью рассуждений для пользователя

**Вывод по reasoning:** OpenClaw заметно опережает по управлению, интеграции и UX рассуждений.

---

## 3. AI-возможности

### Провайдеры и модели

| Функция | OpenClaw | ZeroClaw |
|---------|----------|----------|
| Провайдеры | Много (OpenAI, Anthropic, Gemini, MiniMax, OpenRouter, Z.AI, Ollama, Antigravity и др.) | Trait-driven набор провайдеров |
| Fallback | Цепочка fallback-моделей | Failover |
| Алиасы моделей | Да (`opus` → `anthropic/claude-opus-4-6`) | Через конфиг |
| API key rotation | Да | Нет |
| Streaming | Да | Да |

### Инструменты и память

| Функция | OpenClaw | ZeroClaw |
|---------|----------|----------|
| Tool calling | Да, через pi-agent-core | Да, собственный loop |
| Политика инструментов | Каскадная (allow/deny по agent/group/sandbox) | Allowlist/denylist |
| Профили инструментов | `minimal`, `coding`, `messaging`, `full` | Профили в конфиге |
| Память | Гибрид vector + BM25 (SQLite), chunking | markdown + sqlite + lucid |
| Embeddings | OpenAI, Gemini, Voyage, local (llama-cpp) | Настраиваемые |

### Мультиагентность и длительные задачи

| Функция | OpenClaw | ZeroClaw |
|---------|----------|----------|
| Subagents | `sessions_spawn` в изолированных сессиях | `subagent_spawn`, `subagent_list`, `subagent_manage` |
| Маршрутизация по agent | Да (channel/account/peer → agent) | Через конфиг |
| Delegate / multi-agent | Через subagents | `delegate` tool (researcher, coder и др.) |
| Cron | Да, встроенный | Планировщик (если есть) |

---

## 4. Структурированное планирование и цели

### ZeroClaw — отдельный движок целей

- **Goals Engine** (`src/goals/engine.rs`):
  - `state/goals.json` с целями, шагами, приоритетами
  - статусы: Pending, InProgress, Completed, Blocked, Cancelled
  - retry, обнаружение stalled goals
- **Research Phase** (`src/agent/research.rs`):
  - проактивный сбор информации до основного ответа
  - триггеры: Never, Always, Keywords, Length, Question
  - `[RESEARCH COMPLETE]` как маркер завершения
- **Identity Goals** (SOUL.md):
  - `short_term_goals`, `long_term_goals` в системном промпте

### OpenClaw — через подсистемы

- **Subagents** для длительных задач (в т.ч. research)
- **Cron** для планирования
- **Lobster** — workflow runtime с approval gates
- Системный промпт: «avoid long-term plans beyond user's request», явного goals engine нет

**Вывод:** У ZeroClaw есть явный goals engine и встроенная research-фаза; у OpenClaw это реализуется через subagents и Lobster.

---

## 5. Итоговая оценка (без учёта UI)

| Критерий | Более сильная сторона |
|----------|------------------------|
| **Рассуждения (reasoning)** | OpenClaw — thinking levels, директивы, отдельный стриминг, теги |
| **Провайдеры и модели** | OpenClaw — больше провайдеров, алиасы, API key rotation |
| **Инструменты и память** | Сопоставимо: разная гранулярность политик, оба — tool calling, embeddings |
| **Структурированные цели** | **ZeroClaw** — goals engine, research phase, identity goals |
| **Производительность** | **ZeroClaw** — Rust, \< 5 MB RAM, \< 10 ms cold start |
| **Безопасность/песочница** | Оба: OpenClaw — Docker sandbox, ZeroClaw — strict allowlist |
| **Headless/edge** | **ZeroClaw** — единый бинарник, низкие требования, нет зависимости от Node.js |

### Краткий вывод при headless-сценарии

- **OpenClaw** сильнее в контроле reasoning (thinking levels, директивы, streaming) и в числе провайдеров; Lobster даёт workflow с approval gates. Учёт reasoning UX менее критичен, если интерфейс — канал (Telegram, webhook и т.п.), а не встроенный dashboard.
- **ZeroClaw** сильнее в планировании (goals engine, research phase), производительности и пригодности для edge/sidecar/микросервисных сценариев. Goals и research — встроенные, а не через subagents.
- **Сводка для headless:** при отсутствии требований к встроенному UI ZeroClaw чаще предпочтителен для edge, sidecar, CI, низкомощного железа и задач с долгосрочными целями; OpenClaw — когда нужен максимум reasoning-инструментария и богатый набор провайдеров без учёта footprint.
