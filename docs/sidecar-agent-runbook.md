# Sidecar Agent Runbook — ZeroClaw in Ephemeral Pods

Полная документация для агента-разработчика сайдкара, запускающего ZeroClaw-агента в контейнерном окружении (pod) с жизненным циклом: подготовка окружения → обработка запроса → сохранение → завершение.

**Last verified:** February 27, 2026.

---

## 1. Архитектура: сайдкар + под

```
┌─────────────────────────────────────────────────────────────────┐
│  Pod (ephemeral)                                                │
│  ┌───────────────┐    ┌─────────────────────────────────────┐   │
│  │ Sidecar       │───▶│ ZeroClaw Agent                      │   │
│  │ - restore env │    │ - channels / gateway                │   │
│  │ - build config│    │ - process_message / run             │   │
│  │ - save env    │◀───│ - signal completion                 │   │
│  └───────────────┘    └─────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         │                                    │
         ▼                                    ▼
  Storage (persistent)              Channel / Gateway API
  - workspace files                 - Telegram, Discord, webhook, etc.
  - config.toml
  - memory, state
```

---

## 2. Подготовка окружения из хранилища

### 2.1 Что восстанавливать

Все данные, которые ZeroClaw ожидает в рабочем каталоге и от которых зависит сессия:

| Артефакт | Путь | Описание |
|----------|------|----------|
| **Конфигурация** | `config.toml` | Основной конфиг (провайдеры, каналы, память, security) |
| **Рабочий каталог** | `workspace/` | Файлы проекта, bootstrap, skills |
| **Память** | `workspace/MEMORY.md`, `workspace/memory/*.md` | Markdown-бэкенд памяти |
| **Состояние памяти** | `workspace/*.sqlite3` (если sqlite) | SQLite-бэкенд |
| **Секреты** | `.env`, или через vault | API-ключи, токены |
| **Skills** | `workspace/skills/` | Установленные навыки |
| **Состояние** | `state/`, `daemon_state.json` | Опционально: runtime trace, daemon state |

### 2.2 Структура workspace (OpenClaw)

ZeroClaw использует структуру OpenClaw для identity и контекста:

```
<workspace_dir>/
├── config.toml              # Основной конфиг (или в родительской директории)
├── workspace/               # Рабочий каталог (если config рядом)
│   ├── AGENTS.md            # Инструкции для агента
│   ├── SOUL.md              # Идентичность
│   ├── TOOLS.md             # Описание инструментов
│   ├── IDENTITY.md          # Ролевая модель
│   ├── USER.md              # Контекст пользователя
│   ├── MEMORY.md            # Основной файл памяти (markdown)
│   ├── memory/              # Ежедневные логи
│   │   └── YYYY-MM-DD.md
│   ├── skills/              # Навыки и WASM-инструменты
│   │   └── <skill-name>/
│   │       └── tools/
│   ├── state/               # Runtime trace (если включено)
│   │   └── runtime-trace.jsonl
│   └── ...
```

### 2.3 Разрешение конфигурации и workspace

Порядок разрешения при старте ZeroClaw (от высшего приоритета):

1. **`ZEROCLAW_CONFIG_DIR`** (env) — явная директория с config.toml.
2. **`ZEROCLAW_WORKSPACE`** (env) — корень workspace (если задан без config_dir).
3. **`~/.zeroclaw/active_workspace.toml`** — маркер активного workspace (содержит `config_dir`).
4. **`~/.zeroclaw/config.toml`** и `~/.zeroclaw/workspace` — значения по умолчанию.

Для сайдкара рекомендуется явно задавать:

```bash
export ZEROCLAW_WORKSPACE=/path/to/restored/workspace
# или
export ZEROCLAW_CONFIG_DIR=/path/to/restored/config_dir
```

Если `config_dir` = `/app/zeroclaw`, то:
- `config_path` = `/app/zeroclaw/config.toml`
- `workspace_dir` = `/app/zeroclaw/workspace` (или как указано в `[storage]` / legacy)

### 2.4 Альтернативная схема: config в workspace

При `workspace/config.toml` и `workspace/workspace` (или `workspace` как корень):
- `config_path` = `workspace/config.toml`
- `workspace_dir` = `workspace/` (рабочий каталог)

Сайдкар должен восстановить ровно ту структуру, которую ожидала последняя сессия.

### 2.5 Memory backends

| Backend | Хранилище |
|---------|-----------|
| `markdown` | `workspace/MEMORY.md`, `workspace/memory/YYYY-MM-DD.md` |
| `sqlite` | Файл в `workspace_dir` или путь из `memory.path` |
| `lucid` | SQLite + markdown |

При восстановлении убедитесь, что все файлы памяти и БД присутствуют и доступны для записи.

### 2.6 Чеклист восстановления

```
[ ] Скопировать config.toml в целевую директорию
[ ] Восстановить workspace/ целиком (bootstrap, skills, memory)
[ ] Восстановить .env или инжектировать секреты
[ ] Установить ZEROCLAW_WORKSPACE или ZEROCLAW_CONFIG_DIR
[ ] Проверить права на запись (workspace, state)
[ ] При sqlite/lucid — проверить целостность БД
```

---

## 3. Сайдкар собирает пользовательские настройки

Сайдкар должен подготовить конфиг и окружение под конкретного пользователя:

1. **Подстановка переменных** — API-ключи, токены каналов из хранилища секретов.
2. **Слияние конфигов** — базовый `config.toml` + пользовательские overrides (например, `default_model`, `allowed_users` в канале).
3. **Канал** — включить только нужный канал (Telegram, Discord, webhook и т.д.) и настроить credentials.
4. **Workspace** — смонтировать/восстановить именно пользовательский workspace.

Критично: ZeroClaw читает конфиг при старте. Все изменения должны быть записаны до запуска `zeroclaw daemon` или `zeroclaw run`.

---

## 4. ZeroClaw агент и канал

### 4.1 Запуск с каналом

Типичный запуск для каналов:

```bash
zeroclaw daemon --host 0.0.0.0 --port 8080
```

Daemon поднимает gateway и каналы. Каналы слушают входящие сообщения и передают их в общую очередь.

### 4.2 Альтернатива: Gateway + POST /api/chat

Для «одного запроса — один ответ» без долгоживущего listen:

```bash
zeroclaw daemon --host 0.0.0.0 --port 8080
# Сайдкар делает:
curl -X POST http://localhost:8080/api/chat \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <token>" \
  -d '{"message": "user request"}'
```

Ответ HTTP приходит когда обработка завершена. Это удобно для сайдкара: завершение HTTP-запроса = завершение обработки.

### 4.3 Несколько пользователей в канале

#### Ключи сессий

- **`conversation_history_key`**: `{channel}_{thread_id?}_{sender}` — изоляция истории по отправителю и треду.
- **`interruption_scope_key`**: `{channel}_{reply_target}_{sender}` — область отмены (только для того же отправителя, Telegram).

#### Общая очередь и семафор

Все каналы шлют сообщения в **общую очередь** (mpsc, ёмкость 100). Порядок обработки — **FIFO**. Параллельность ограничена **`max_in_flight_messages`** (семафор).

#### Сообщения от других пользователей, пришедшие после начала обработки первого

Сообщения от **разных отправителей** не отменяют друг друга. Механизм:

1. Сообщения попадают в общую FIFO-очередь.
2. Для каждого сообщения берётся permit семафора.
3. Если есть свободный permit — обработка стартует сразу (параллельно с уже идущими).
4. Если все permits заняты — сообщение ждёт в очереди до освобождения любого permit.

Пример (User A обрабатывается, User B и C шлют сообщения):

| Событие | Что происходит |
|---------|-----------------|
| User A отправил — началась обработка | Занимается 1 permit |
| User B отправил (пока A ещё обрабатывается) | Если есть свободный permit — B стартует параллельно с A |
| User C отправил (A и B ещё обрабатываются) | Аналогично — параллельно, если permit свободен |
| Все permits заняты, User D отправил | D ждёт в очереди. Как только кто-то (A, B или C) закончит, D получит permit и начнётся обработка |

История и контекст изолированы по отправителю (`conversation_history_key`), поэтому A, B, C и D работают с разными сессиями и друг друга не пересекают.

#### Сообщения от того же пользователя

- **Без `interrupt_on_new_message`**: новые сообщения от того же пользователя идут в общую очередь и конкурируют за permits с остальными; могут обрабатываться параллельно, если успеют получить свободные permits.
- **С `interrupt_on_new_message` (Telegram)**: новое сообщение от того же пользователя **отменяет** текущее (cancellation token), старое досрочно завершается, новое обрабатывается.

Конфиг Telegram:

```toml
[channels.telegram]
interrupt_on_new_message = true   # новое сообщение от того же пользователя отменяет текущее
```

### 4.4 Лимиты

- `max_in_flight_messages` = `channel_count * 4` (в пределах 2–16).
- `agent.max_tool_iterations` — лимит итераций инструментов на сообщение.

---

## 5. Статусная модель и Redis

При включённой секции `[sidecar_status]` (и feature `status-redis`) агент публикует статус в Redis. Сайдкар может опрашивать ключ или подписаться на изменения и использовать это как сигнал возможности завершения пода.

### 5.1 Переходы статусов

| Статус | Описание |
|--------|----------|
| `starting` | Агент запущен, подключается к каналу |
| `working` | Обрабатывает сообщение(я) |
| `completed_awaiting` | Обработка завершена, новых сообщений нет в течение `idle_timeout_secs`; агент продолжает слушать канал |

Диаграмма переходов:

```
[start] → starting → working ⟷ completed_awaiting
                         ↑              │
                         └──────────────┘
                    (новое сообщение)
```

- При старте агент публикует `starting`.
- При получении сообщения — `working`. При завершении всех in-flight сообщений запускается таймер.
- Если в течение `idle_timeout_secs` новых сообщений нет — публикуется `completed_awaiting`.
- При новом сообщении — снова `working`, таймер idle отменяется.

### 5.2 Redis: ключ и формат

- **Ключ**: `{key_prefix}:{agent_id}:status` или `{key_prefix}:{agent_id}:{user_id}:status` (если задан `user_id`).
- **Значение**: JSON `{"status":"...", "updated_at":"2026-02-27T12:00:00Z"}`.

### 5.3 Использование сайдкаром

1. После запуска агента — опрашивать ключ или подписаться на изменения.
2. При появлении статуса `completed_awaiting` — можно считать, что агент завершил работу и ждёт; при отсутствии новых сообщений под можно корректно остановить.
3. Перед остановкой: сохранить workspace и состояние в хранилище, затем завершить процесс.

Параметры задаются в `[sidecar_status]` (см. [config-reference.md](config-reference.md)). Для включения Redis требуется feature `status-redis`: `cargo build --features status-redis`.

---

## 6. Сигнал завершения обработки

### 6.1 В каналах (reactions)

Когда обработка завершена:

1. С исходного сообщения снимается реакция «👀» (typing/processing).
2. Добавляется реакция «✅» (успех) или «⚠️» (ошибка).
3. Ответ отправляется в канал через `channel.send()`.

Каналы с поддержкой `add_reaction` / `remove_reaction` (Telegram, Discord и др.) визуально показывают завершение.

### 6.2 В Gateway (POST /api/chat)

- HTTP-ответ возвращается по завершении `process_message()`.
- Для сайдкара: **получение HTTP 200 + body = завершение обработки**.

### 6.3 Как сайдкару понять «готово»

| Режим | Как понять завершение |
|-------|------------------------|
| **Gateway /api/chat** | HTTP-ответ получен ✅ |
| **Gateway WebSocket /ws/chat** | Сообщение `type: "done"` в WebSocket |
| **Каналы (daemon) + Redis** | Статус `completed_awaiting` в Redis (см. раздел 5) ✅ |
| **Каналы (daemon)** | Daemon работает непрерывно; явного «готово» в стандартном API нет |

Для модели «под на один запрос» рекомендуется:

1. **Gateway /api/chat** — сайдкар дергает `POST /api/chat`, дожидается ответа и считает обработку завершённой.
2. **Redis `[sidecar_status]`** — сайдкар опрашивает/подписывается на ключ; при `completed_awaiting` считает обработку завершённой и может завершить под.
3. **Custom channel** — реализовать канал, который по одному сообщению вызывает agent и по завершении отправляет callback сайдкару.
4. **Файловый/сокетный контракт** — агент по завершении пишет в файл или сокет (потребует изменений в коде или wrapper).

---

## 7. После обработки: сохранение и выход

### 7.1 Что сохранять обратно в хранилище

После завершения обработки сайдкар должен сохранить:

| Артефакт | Комментарий |
|----------|-------------|
| `workspace/` | MEMORY.md, memory/*.md, изменённые файлы |
| `*.sqlite3` | База памяти (sqlite/lucid) |
| `config.toml` | При изменении конфига |
| `state/` | Runtime trace, costs.jsonl (если включены) |
| `.env` | Не рекомендуется — секреты через vault |

### 7.2 Жизненный цикл пода

1. Под стартует, сайдкар восстанавливает окружение.
2. Сайдкар собирает конфиг пользователя (в т.ч. `[sidecar_status]` при использовании Redis).
3. Запускается ZeroClaw (daemon или run) с каналом.
4. Агент публикует `starting`, затем при сообщениях — `working`.
5. Обработка сообщений (через канал или `/api/chat`).
6. После завершения всех сообщений и истечения `idle_timeout_secs` — агент публикует `completed_awaiting`.
7. Сайдкар фиксирует завершение (HTTP-ответ, статус Redis `completed_awaiting` и т.п.).
8. Сайдкар сохраняет workspace и состояние в хранилище.
9. Под завершает работу, окружение очищается.

### 7.3 Завершение процесса

- При использовании `POST /api/chat`: сайдкар может держать gateway живым для нескольких запросов или завершать процесс после первого.
- При каналах + Redis: сайдкар опрашивает ключ; при `completed_awaiting` вызывает shutdown, сохраняет состояние и завершает под.
- При каналах без Redis: daemon сам не завершается; сайдкар должен решить, когда вызвать shutdown (например, по таймауту, по счётчику сообщений или внешнему сигналу).

### 7.4 Трекинг потребления токенов

Сайдкар может измерять потребление токенов за сессию обработки для биллинга, лимитов и отчётности.

#### Runtime trace (рекомендуемый способ)

Включите runtime trace в `config.toml`:

```toml
[observability]
runtime_trace_mode = "rolling"   # или "full"
runtime_trace_path = "state/runtime-trace.jsonl"
runtime_trace_max_entries = 200
```

События `llm_response` содержат в payload `input_tokens` и `output_tokens` для каждого вызова LLM. Все вызовы в рамках одного пользовательского сообщения имеют общий `turn_id`.

**Агрегация по сессии (turn):**

```bash
# События llm_response
zeroclaw doctor traces --event llm_response --limit 50
```

Для выгрузки суммы по последним turn можно обработать JSONL вручную:

```bash
jq -s '[.[] | select(.event_type == "llm_response") | (.payload.input_tokens // 0) + (.payload.output_tokens // 0)] | add' workspace/state/runtime-trace.jsonl
```

При сохранении состояния включайте `state/runtime-trace.jsonl` в артефакты для последующей отчётности.

#### Cost tracker и API

Включите cost tracking в конфиге:

```toml
[cost]
enabled = true
daily_limit_usd = 10.0
monthly_limit_usd = 100.0
```

При использовании Gateway с аутентификацией (pairing) доступен endpoint:

```bash
curl -H "Authorization: Bearer <token>" http://localhost:8080/api/cost
```

Ответ содержит `session_cost_usd`, `total_tokens`, `request_count`, `by_model`. Данные сохраняются в `state/costs.jsonl`. При сохранении пода включайте этот файл, если cost tracking включён.

**Примечание:** в текущей реализации `record_usage` вызывается только при явной интеграции с agent loop; endpoint может возвращать нулевые значения до полной привязки.

#### Prometheus-метрики

При `[observability] backend = "prometheus"` доступны счётчики:

- `zeroclaw_tokens_input_total{provider, model}`
- `zeroclaw_tokens_output_total{provider, model}`

Это глобальные метрики процесса, не привязанные к конкретному turn. Для оценки потребления за интервал используйте `increase(...)` в PromQL.

Подробнее: [config-reference.md](config-reference.md) — секции `[observability]` и `[cost]`.

---

## 8. Ссылки

- [config-reference.md](config-reference.md) — конфиг ZeroClaw
- [channels-reference.md](channels-reference.md) — каналы
- [commands-reference.md](commands-reference.md) — CLI
- [operations-runbook.md](operations-runbook.md) — операционные процедуры
