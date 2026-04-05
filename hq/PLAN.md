# HQ Project Plan

## 1. Project Overview

The `hq` project is a modular Rust application designed to manage "Taps" (digital interactions/NFC points) and Users, integrating deeply with Discord. It follows a workspace-based monorepo structure to separate concerns between shared types, core business logic, the HTTP API backend, and the Discord bot.

**Key Technologies:**
-   **Language:** Rust (2021 edition)
-   **Web Framework:** Axum
-   **Database:** PostgreSQL (via SQLx)
-   **Caching/Queues:** Redis (via `redis-rs` or `deadpool-redis`)
-   **Discord Bot:** Serenity / Poise
-   **Serialization:** Serde
-   **Documentation:** Utoipa (OpenAPI)

---

## 2. Module Responsibilities

### `hq/types`
-   **Role:** Shared library containing domain entities and common data structures.
-   **Contents:**
    -   **Models:** `User`, `Tap`, `TapPermission`, `TapOccupation`, `TapRole`.
    -   **IDs:** Strongly typed IDs (`UserId`, `TapId`, `DiscordUserId`).
    -   **Enums:** Error types shared across crates (optional).
    -   **DTOs:** Request/Response structures for the API.

### `hq/core`
-   **Role:** The "Brain" of the application. Contains business logic, database interactions, and external service integrations.
-   **Contents:**
    -   **Database:** SQLx migrations, connection pool management.
    -   **Repositories:** Traits and implementations for data access (`UserRepository`, `TapRepository`).
    -   **Services:** `AuthService` (OAuth2), `TapService` (Logic), `RedisService` (Caching).
    -   **Utilities:** Hashing, ID generation (UUID/NanoID), centralized error handling.

### `hq/backend`
-   **Role:** HTTP REST API layer exposed to the frontend or external clients.
-   **Contents:**
    -   **Router:** Axum routes definition.
    -   **Handlers:** Controllers for Auth, Users, Taps.
    -   **Middleware:** Authentication (JWT), CORS, Tracing/Logging.
    -   **Docs:** OpenAPI (Swagger UI) generation.

### `hq/bot`
-   **Role:** Discord interface for interacting with the system.
-   **Contents:**
    -   **Commands:** Slash commands (`/tap`, `/profile`, `/admin`).
    -   **Events:** Handling `InteractionCreate`, `GuildMemberAdd`, etc.
    -   **Integration:** Calls into `hq/core` to fetch/modify data.

---

## 3. Data Persistence & Schema

### Database Schema (PostgreSQL)

**1. `users` Table**
| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `id` | UUID/TEXT | PK | Internal User ID |
| `discord_user_id` | TEXT | UNIQUE, NOT NULL | Discord User ID |
| `username` | TEXT | NOT NULL | Display name |
| `avatar_url` | TEXT | | Profile picture |
| `email` | TEXT | | Optional email |
| `permissions` | JSONB | DEFAULT '[]' | List of permissions |
| `created_at` | TIMESTAMPTZ | DEFAULT NOW() | |
| `updated_at` | TIMESTAMPTZ | DEFAULT NOW() | |

**2. `taps` Table**
| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `id` | UUID/TEXT | PK | Tap ID |
| `owner_id` | UUID/TEXT | FK(users.id) | Owner of the tap |
| `name` | TEXT | NOT NULL | Display name |
| `description` | TEXT | | |
| `occupation` | TEXT | NOT NULL | 'official', 'verified', 'base' |
| `permission` | JSONB | NOT NULL | Access control logic |
| `role` | TEXT | NOT NULL | 'music', 'tts', etc. |
| `created_at` | TIMESTAMPTZ | DEFAULT NOW() | |
| `updated_at` | TIMESTAMPTZ | DEFAULT NOW() | |

**3. `tap_usage` Table**
| Column | Type | Constraints | Description |
| :--- | :--- | :--- | :--- |
| `id` | UUID | PK | Usage Event ID |
| `tap_id` | UUID/TEXT | FK(taps.id) | |
| `user_id` | UUID/TEXT | FK(users.id) | Who tapped |
| `action` | TEXT | NOT NULL | Type of interaction |
| `created_at` | TIMESTAMPTZ | DEFAULT NOW() | |

**Migration Strategy:**
-   Use `sqlx-cli`.
-   Path: `hq/core/migrations/`.
-   Format: `YYYYMMDDHHMMSS_description.sql`.

---

## 4. Core Architecture

### Traits (Repositories)
Abstract data access to allow for easier testing and swapping implementations.

```rust
// in hq/core/src/repo/user.rs
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: &User) -> Result<User, CoreError>;
    async fn find_by_discord_id(&self, id: &DiscordUserId) -> Result<Option<User>, CoreError>;
    async fn update(&self, user: &User) -> Result<User, CoreError>;
}

// in hq/core/src/repo/tap.rs
#[async_trait]
pub trait TapRepository: Send + Sync {
    async fn create(&self, tap: &Tap) -> Result<Tap, CoreError>;
    async fn find_by_id(&self, id: &TapId) -> Result<Option<Tap>, CoreError>;
    async fn list_by_owner(&self, owner_id: &UserId) -> Result<Vec<Tap>, CoreError>;
}
```

### Services
-   **`AuthService`**: Handles Discord OAuth2 code exchange, user creation/lookup (via `UserRepository`), and JWT token generation.
-   **`TapService`**: Business logic for Taps. Validates permissions before creation/updates.
-   **`AppState`**: A struct holding the connection pool and service instances, shared across Axum and Poise.

### Error Handling
-   **`CoreError`**: Enum for DB errors, Logic errors, Not Found, etc.
-   **`ApiError`**: Maps `CoreError` to HTTP Status Codes (impl `IntoResponse`).

---

## 5. API Design (Backend)

**Base URL:** `/api/v1`

### Authentication
-   `GET /auth/login`: Redirects to Discord OAuth2.
-   `GET /auth/callback`: Handles the code return from Discord. Returns JWT + Refresh Token.
-   `POST /auth/refresh`: Refreshes access token.

### Users
-   `GET /users/me`: Get current authenticated user profile.
-   `PATCH /users/me`: Update profile settings.
-   `GET /users/:id`: Get public profile of another user.

### Taps
-   `POST /taps`: Create a new Tap (Auth required).
-   `GET /taps`: List public taps (Pagination).
-   `GET /taps/:id`: Get specific tap details.
-   `PATCH /taps/:id`: Update tap (Owner/Admin only).
-   `DELETE /taps/:id`: Delete tap.

### Admin
-   `POST /admin/taps/:id/verify`: Set tap occupation to 'verified'/'official'.

### Middleware
1.  **Tracing:** `tower_http::trace::TraceLayer` for logging requests.
2.  **CORS:** Allow requests from frontend domains.
3.  **AuthGuard:** Extracts JWT from `Authorization` header, validates it, and injects `Claims` into the request extension.

---

## 6. Bot Design

**Framework:** `poise` (built on top of `serenity`).

### Commands
-   `/ping`: Health check.
-   `/profile [user]`: View Discord profile + HQ stats (taps owned, etc.).
-   `/tap create <name>`: Quick create a tap via Discord.
-   `/tap list`: List your taps.

### Event Handlers
-   **`ready`**: Register slash commands globally/guild-specific.
-   **`interaction_create`**: Handle button clicks or selects if Taps have interactive elements in Discord messages.

### Architecture
-   The Bot will initialize the **same** `hq/core` services as the Backend.
-   It will share the database connection pool.

---

## 7. Implementation Steps

1.  **Setup & Types:**
    -   Initialize workspace `Cargo.toml`.
    -   Flesh out `hq/types` with all structs (User, Tap) and Serde derives.

2.  **Core & Database:**
    -   Setup `hq/core` dependencies (`sqlx`, `tokio`).
    -   Create `docker-compose.yml` for Postgres + Redis.
    -   Write SQL migrations in `hq/core/migrations`.
    -   Implement `UserRepository` and `TapRepository` in `hq/core`.

3.  **Backend Skeleton:**
    -   Setup `hq/backend` with `axum`.
    -   Implement basic `health` endpoint.
    -   Setup `AppState` to hold the DB pool.

4.  **Authentication:**
    -   Implement Discord OAuth2 flow in Backend.
    -   Implement JWT generation/validation.
    -   Create Auth Middleware.

5.  **Feature Implementation:**
    -   Implement CRUD for Taps in Backend.
    -   Connect `hq/core` logic to Axum handlers.

6.  **Bot Implementation:**
    -   Setup `hq/bot` with `poise`.
    -   Connect Bot to DB pool.
    -   Implement basic commands.

---

## 8. Dependencies

### `hq/types`
-   `serde`, `serde_json`
-   `derive_more`
-   `uuid` (or `nanoid`)
-   `chrono` (or `time`)

### `hq/core`
-   `sqlx` (features: `runtime-tokio-rustls`, `postgres`, `uuid`, `chrono`, `macros`)
-   `thiserror`
-   `async-trait`
-   `redis`
-   `tracing`
-   `reqwest` (for Discord API calls)
-   `jsonwebtoken`
-   `oauth2`

### `hq/backend`
-   `axum`
-   `tokio`
-   `tower`, `tower-http` (cors, trace)
-   `tracing`, `tracing-subscriber`
-   `utoipa`, `utoipa-swagger-ui` (Documentation)
-   `serde`

### `hq/bot`
-   `poise`
-   `serenity`
-   `tokio`
-   `tracing`

---

## 9. First Implementation: The Tap Loop (MVP)

This section outlines the immediate next steps to build a "Vertical Slice" of the application. The goal is to prove the integration of `hq/core`, `hq/backend`, and `hq/bot` by implementing a single feature: **Creating and Viewing a Tap**.

### 9.1. Objective
Enable a user to:
1.  Authenticate with the Backend using Discord OAuth2 (obtaining a JWT).
2.  Create a "Tap" (a named digital entity) via the Backend API using that JWT.
3.  List their created Taps via the Backend API.
4.  Create and List Taps using the Discord Bot (sharing the same database/logic).

### 9.2. Database Schema (MVP)
We will start with a simplified schema to reduce friction.

**1. `users` Table**
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    discord_user_id TEXT NOT NULL UNIQUE,
    username TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

**2. `taps` Table**
```sql
CREATE TABLE taps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### 9.3. `hq/core` Implementation

**Structs:**
-   `struct User { id: Uuid, discord_user_id: String, username: String }`
-   `struct Tap { id: Uuid, owner_id: Uuid, name: String }`
-   `struct CreateTapDto { name: String }`

**Services:**
1.  **`AuthService`**:
    -   `authenticate(code: String) -> Result<String, CoreError>`
        -   Exchanges Discord OAuth2 code for an access token.
        -   Fetches user info from Discord API.
        -   Upserts user into `users` table (find by `discord_user_id` or create).
        -   Generates and returns a JWT containing the internal `user_id`.

2.  **`TapService`**:
    -   `create(owner_id: Uuid, dto: CreateTapDto) -> Result<Tap, CoreError>`
        -   Inserts a new Tap into the `taps` table.
    -   `list_by_user(user_id: Uuid) -> Result<Vec<Tap>, CoreError>`
        -   Selects all Taps where `owner_id = user_id`.

### 9.4. `hq/backend` Implementation (API)

**Endpoints:**
1.  **`POST /auth/login`**
    -   Input: `{ "code": "discord_oauth_code" }`
    -   Action: Calls `AuthService::authenticate`.
    -   Output: `{ "token": "jwt_string" }`

2.  **`POST /api/v1/taps`**
    -   Header: `Authorization: Bearer <token>`
    -   Input: `{ "name": "My Cool Tap" }`
    -   Action: Extracts `user_id` from JWT, calls `TapService::create`.
    -   Output: JSON representation of the created Tap.

3.  **`GET /api/v1/taps`**
    -   Header: `Authorization: Bearer <token>`
    -   Action: Extracts `user_id` from JWT, calls `TapService::list_by_user`.
    -   Output: JSON list of Taps.

### 9.5. `hq/bot` Implementation (Commands)

**Commands:**
1.  **`/tap create <name>`**
    -   Action:
        -   Resolves Discord User ID to internal `user_id` (via `UserRepository`).
        -   If user doesn't exist, create them on the fly? (Or tell them to register via web). *Decision for MVP: Create on fly.*
        -   Calls `TapService::create`.
    -   Response: "Tap 'My Cool Tap' created! ID: ..."

2.  **`/tap list`**
    -   Action:
        -   Resolves Discord User ID to internal `user_id`.
        -   Calls `TapService::list_by_user`.
    -   Response: Embed listing all taps owned by the user.

### 9.6. Guide for Expansion (Post-MVP)

Once this "Tap Loop" is working, expand in this order:
1.  **Fields:** Add `description`, `occupation`, and `permission` to the `taps` table and structs.
2.  **Validation:** Add logic to `TapService` to enforce naming constraints or limits on number of taps.
3.  **Usage Tracking:** Implement the `tap_usage` table and `TapService::record_usage`.
4.  **Permissions:** Implement `TapPermission` logic to control who can use/edit a tap.
5.  **Frontend:** Build a simple web UI to consume the API.

## 10. Frontend Integration Expansion Plan

The frontend (`web/`) has been developed ahead of the backend (`hq/`) and currently relies on MSW mocks for several features. The following plan outlines the steps to build out the missing backend functionality to fully integrate with the existing frontend.

### 10.1. API Key Management (API Tokens)
- **Database:** Create an `api_keys` table (`id`, `tap_id`, `name`, `key_hash`, `scopes`, `last_used_at`, `created_at`).
- **Core Service:** Implement `ApiKeyService` for CRUD operations and key hashing.
- **API Routes:** Mount `GET`, `POST`, `PATCH`, `DELETE`, and `POST /regenerate` on `/api/v1/taps/:id/api-tokens`.

### 10.2. Tap Management Extensions
- **Core Service:** Implement `update` and `delete` operations in `TapService`, ensuring robust permission checks (verifying `user_id` has owner/admin rights).
- **API Routes:** Mount `PATCH /api/v1/taps/:id` and `DELETE /api/v1/taps/:id`.

### 10.3. Audit Logs
- **Database:** Create an `audit_logs` table (`id`, `tap_id`, `actor_id`, `action_type`, `metadata`, `created_at`).
- **Core Service:** Add an `AuditService` to write logs. Wrap state-mutating functions in `TapService` and `ApiKeyService` to automatically emit audit records.
- **API Routes:** Add a paginated `GET /api/v1/taps/:id/audit-log` endpoint.

### 10.4. Metrics & Analytics
- **Database:** Create a `tap_metrics` (or `tap_usage`) table for recording time-series data (e.g., requests, cache hits, errors).
- **Core Service:** Update `get_tap_stats` to query aggregated metrics instead of returning mock arrays. Implement a mechanism to ingest metrics.

### 10.5. Auth Enhancements
- **API Routes:** Implement `POST /api/v1/auth/logout` (invalidating the session/token on the server or instructing the client to clear cookies) and `GET /api/v1/auth/refresh` to rotate JWTs or session tokens.

### 10.6. Admin Panel & Moderation
- **Auth Middleware:** Create an `AdminRequired` middleware/extractor in Rust to protect these routes.
- **Core Service:** Add admin-specific service functions (e.g., `verify_tap`, `list_all_platform_users`).
- **API Routes:** Mount new routes under `/api/v1/admin/`.

### 10.7. Notifications
- **Database:** Create a `notifications` table (`id`, `user_id`, `type`, `title`, `message`, `read_at`, `created_at`).
- **Core Service/API:** Implement CRUD operations and a `GET /api/v1/notifications` endpoint.
