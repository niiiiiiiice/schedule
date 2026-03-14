# Rust CQRS Clean Architecture — REST API

## Архитектура

```
┌─────────────────────────────────────────────────────────────────┐
│                         API Layer                                │
│  axum handlers → Request DTOs → Command/Query → Response         │
│  Аналог C#: ASP.NET Controllers + middleware                     │
├─────────────────────────────────────────────────────────────────┤
│                     Application Layer                            │
│  Commands (write) │ Queries (read) │ Ports (traits/interfaces)   │
│  Аналог C#: MediatR handlers + IRepository interfaces            │
├─────────────────────────────────────────────────────────────────┤
│                       Domain Layer                               │
│  Entities │ Value Objects │ Domain Events │ Business Rules        │
│  Аналог C#: Domain models, no dependencies                      │
├─────────────────────────────────────────────────────────────────┤
│                   Infrastructure Layer                            │
│  PostgreSQL repos │ Redis cache │ RabbitMQ publisher              │
│  Аналог C#: EF Core + StackExchange.Redis + MassTransit          │
└─────────────────────────────────────────────────────────────────┘
```

## CQRS: разделение чтения и записи

```
POST/PATCH/DELETE  ──→  Command Handler  ──→  WriteRepository (Task entity)
                                         ──→  EventPublisher (RabbitMQ)
                                         ──→  Cache invalidation (Redis DEL)

GET                ──→  Query Handler    ──→  Cache (Redis GET)
                                         ──→  ReadRepository (DTO напрямую)
                                         ──→  Cache (Redis SET)
```

## Маппинг C# → Rust

| C# концепт                     | Rust аналог в проекте              |
|---------------------------------|------------------------------------|
| `IServiceCollection` / DI       | `AppState` + `Arc<dyn Trait>`      |
| `IRepository<T>`                | `TaskWriteRepository` trait        |
| MediatR `IRequest` + Handler    | `CreateTaskCommand` + Handler      |
| `ProblemDetails` middleware      | `impl IntoResponse for AppError`   |
| Entity Framework `DbContext`     | `sqlx::PgPool` + raw SQL           |
| `IDistributedCache`             | `CachePort` trait                  |
| MassTransit `IBus.Publish<T>`   | `EventPublisher` trait + lapin     |
| `record` DTO                    | `struct` + `#[derive(Serialize)]`  |
| `private set` + factory method  | private fields + `Task::create()`  |
| `implicit operator` / ctor      | `From<T>` / `TryFrom<T>` traits   |
| `throw new ValidationException` | `Err(DomainError::Validation(..))` |
| `where T : IComparable`         | `T: PartialOrd` (trait bound)      |

## Зависимости между слоями (направление стрелок = "зависит от")

```
api ──→ application ──→ domain
 │                        ↑
 └──→ infrastructure ─────┘
```

- **domain** — ноль внешних зависимостей (только serde, uuid, chrono)
- **application** — зависит от domain, определяет порты (трейты)
- **infrastructure** — реализует порты, зависит от domain + application
- **api** — composition root, связывает всё

## Запуск

```bash
# 1. Поднять инфраструктуру
docker compose up -d

# 2. Скопировать env
cp .env.example .env

# 3. Запуск
cargo run -p api

# 4. Проверить
curl http://localhost:3000/health
```

## API эндпоинты

```bash
# Создать задачу
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{"title": "Learn Rust", "description": "CQRS + Clean Architecture"}'

# Список задач
curl http://localhost:3000/tasks
curl "http://localhost:3000/tasks?status=pending"

# Получить задачу
curl http://localhost:3000/tasks/{id}

# Обновить статус
curl -X PATCH http://localhost:3000/tasks/{id}/status \
  -H "Content-Type: application/json" \
  -d '{"status": "in_progress"}'

# Удалить
curl -X DELETE http://localhost:3000/tasks/{id}
```
Для проверки работы сообщений:

1. Открой http://localhost:15672 (логин/пароль: guest/guest)
2. Queues → Add a new queue
   - Name: tasks.all
   - Остальное по умолчанию → Add queue
3. После создания кликни на очередь → Bindings → Add binding from exchange
   - From exchange: domain_events
   - Routing key: task.# (поймает все события Task — task.created, task.status_changed, task.deleted)
   - Bind

## Структура проекта

```
rust-cqrs-api/
├── Cargo.toml                 # Workspace
├── docker-compose.yml         # PostgreSQL + Redis + RabbitMQ
├── .env.example
│
├── domain/                    # Ядро — бизнес-логика
│   └── src/
│       ├── entities.rs        # Task (aggregate root)
│       ├── value_objects.rs   # TaskTitle, TaskDescription, TaskStatus
│       ├── events.rs          # DomainEvent enum
│       └── errors.rs          # DomainError
│
├── application/               # Use cases — оркестрация
│   └── src/
│       ├── commands/          # Write operations
│       │   ├── create_task.rs
│       │   ├── update_task_status.rs
│       │   └── delete_task.rs
│       ├── queries/           # Read operations (через кеш)
│       │   ├── get_task.rs
│       │   └── list_tasks.rs
│       ├── ports.rs           # Traits = интерфейсы для infrastructure
│       ├── dto.rs             # Read models
│       └── errors.rs          # AppError (агрегирует все ошибки)
│
├── infrastructure/            # Реализации портов
│   └── src/
│       ├── postgres/
│       │   ├── mod.rs         # Pool + migrations
│       │   ├── write_repo.rs  # TaskWriteRepository impl
│       │   └── read_repo.rs   # TaskReadRepository impl
│       ├── redis/
│       │   └── cache.rs       # CachePort impl
│       └── rabbitmq/
│           └── publisher.rs   # EventPublisher impl
│
└── api/                       # HTTP layer
    └── src/
        ├── main.rs            # Composition root
        ├── state.rs           # AppState (DI container)
        ├── errors.rs          # AppError → HTTP response
        └── handlers/
            └── tasks.rs       # Route handlers
```

## Ключевые паттерны Rust в проекте

### 1. Dependency Inversion через traits + Arc<dyn Trait>
```rust
// Application определяет ЧТО нужно (порт):
#[async_trait]
pub trait TaskWriteRepository: Send + Sync { ... }

// Infrastructure реализует КАК:
impl TaskWriteRepository for PgTaskWriteRepository { ... }

// API связывает:
let repo: Arc<dyn TaskWriteRepository> = Arc::new(PgTaskWriteRepository::new(pool));
```

### 2. Value Objects — невалидное состояние невозможно
```rust
pub struct TaskTitle(String);  // приватное поле — нельзя создать напрямую

impl TaskTitle {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        // валидация здесь
    }
}
```

### 3. Error propagation через ? оператор
```rust
// Цепочка: DomainError → AppError → HTTP Response
// Каждый переход — через From<T> трейт
let title = TaskTitle::new(cmd.title)?;  // DomainError → AppError автоматически
self.repo.save(&task).await?;            // AppError пробрасывается дальше
```

### 4. Send + Sync — потокобезопасность на уровне типов
```rust
// Компилятор ГАРАНТИРУЕТ что всё внутри Arc можно безопасно
// шарить между потоками Tokio. В C# это проверяется только в runtime.
pub trait TaskWriteRepository: Send + Sync { ... }
```
