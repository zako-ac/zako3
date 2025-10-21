# Architecture Examples

Here are some inspirational code snippets for each architectural layer, based on the suggested architecture.

## 1. Domain (`src/domain/user.rs`)

The domain entity is a plain data structure with no dependencies on other layers.

```rust
// src/domain/user.rs

pub struct User {
    pub id: u64,
    pub username: String,
    pub email: String,
}

// You could also have rich domain models with methods
impl User {
    pub fn new(id: u64, username: String, email: String) -> Self {
        // Domain logic for creation, validation, etc. can go here
        Self { id, username, email }
    }
}
```

## 2. Repository (`src/repository/user_repository.rs`)

This is the abstraction (a trait) that the application core uses to interact with data persistence.

```rust
// src/repository/user_repository.rs

use crate::domain::user::User;
use async_trait::async_trait;

// Using async_trait for async methods in traits
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: u64) -> Result<Option<User>, anyhow::Error>;
    async fn save(&self, user: &User) -> Result<(), anyhow::Error>;
}
```

## 3. Infrastructure (`src/infrastructure/postgres/user_repository.rs`)

This is the concrete implementation of the repository trait, specific to a database like PostgreSQL.

```rust
// src/infrastructure/postgres/user_repository.rs

use crate::domain::user::User;
use crate::repository::user_repository::UserRepository;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn find_by_id(&self, id: u64) -> Result<Option<User>, anyhow::Error> {
        // sqlx query to fetch a user from postgres
        let user = sqlx::query_as!(
            User,
            "SELECT id, username, email FROM users WHERE id = $1",
            id as i64 // Example of type casting if needed
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    async fn save(&self, user: &User) -> Result<(), anyhow::Error> {
        // sqlx query to save a user
        // ...
        Ok(())
    }
}
```

## 4. Service (`src/service/user_service.rs`)

A service contains shared business logic and orchestrates repositories.

```rust
// src/service/user_service.rs

use crate::domain::user::User;
use crate::repository::user_repository::UserRepository;
use std::sync::Arc;

pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
}

impl UserService {
    pub fn new(user_repo: Arc<dyn UserRepository>) -> Self {
        Self { user_repo }
    }

    pub async fn find_user(&self, id: u64) -> Result<Option<User>, anyhow::Error> {
        // Maybe there's some complex logic here before/after fetching
        self.user_repo.find_by_id(id).await
    }
}
```

## 5. Usecase (`src/usecase/get_user_by_id.rs`)

A use case represents a single, specific user interaction.

```rust
// src/usecase/get_user_by_id.rs

use crate::domain::user::User;
use crate::service::user_service::UserService;
use std::sync::Arc;

pub struct GetUserByIdUsecase {
    user_service: Arc<UserService>,
}

impl GetUserByIdUsecase {
    pub fn new(user_service: Arc<UserService>) -> Self {
        Self { user_service }
    }

    pub async fn execute(&self, id: u64) -> Result<Option<User>, anyhow::Error> {
        // The use case logic is often simple, just calling the service
        self.user_service.find_user(id).await
    }
}
```

## 6. HTTP Handler (`src/http/user_handler.rs`)

The handler's job is to parse requests, call the use case, and return a response. It knows about HTTP, but not business logic.

```rust
// src/http/user_handler.rs

use crate::usecase::get_user_by_id::GetUserByIdUsecase;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;

// The AppState would hold your application-wide state, like use cases
struct AppState {
    get_user_by_id_usecase: GetUserByIdUsecase,
}

pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<u64>,
) -> impl IntoResponse {
    match state.get_user_by_id_usecase.execute(user_id).await {
        Ok(Some(user)) => (StatusCode::OK, Json(user)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    }
}
```

## 7. Wiring it all together (`src/lib.rs` or `src/bin/server.rs`)

Somewhere in your application's entry point, you'll perform dependency injection to wire all the components together.

```rust
// src/lib.rs or src/bin/server.rs

use std::sync::Arc;
use sqlx::PgPool;

// Imports for all the components
use crate::repository::user_repository::UserRepository;
use crate::infrastructure::postgres::user_repository::PostgresUserRepository;
use crate::service::user_service::UserService;
use crate::usecase::get_user_by_id::GetUserByIdUsecase;
use crate::http::user_handler;

// AppState holds the final, concrete types
struct AppState {
    get_user_by_id_usecase: GetUserByIdUsecase,
}

pub async fn run() {
    // 1. Create database connection pool
    let db_pool = PgPool::connect("...").await.unwrap();

    // 2. Create repository implementation
    let user_repo = Arc::new(PostgresUserRepository::new(db_pool.clone()));

    // 3. Create service
    let user_service = Arc::new(UserService::new(user_repo));

    // 4. Create use case
    let get_user_by_id_usecase = GetUserByIdUsecase::new(user_service);

    // 5. Create application state
    let app_state = Arc::new(AppState {
        get_user_by_id_usecase,
    });

    // 6. Create router and run server
    let app = axum::Router::new()
        .route("/users/:id", axum::routing::get(user_handler::get_user))
        .with_state(app_state);

    // ... run the server
}
```
