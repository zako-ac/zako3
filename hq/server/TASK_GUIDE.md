# Task Guide: Renaming `name` to `username` in User Feature

## Quick Rust Primer

### Rust Basics You Need to Know
- **Ownership**: Rust tracks who "owns" data. When you pass data to a function, ownership moves unless you use `&` (borrow).
- **Structs**: Like TypeScript interfaces or Python classes, but with no inheritance. Example:
  ```rust
  pub struct User {
      pub id: LazySnowflake,
      pub name: String,  // ← You'll change this!
  }
  ```
- **Traits**: Like TypeScript interfaces or Python protocols - they define behavior contracts.
- **Option<T>**: Like TypeScript's `T | null` or Python's `Optional[T]`. Use `Some(value)` or `None`.
- **Result<T, E>**: For functions that can fail. Use `Ok(value)` or `Err(error)`.
- **Macros**: Code that generates code, ending with `!` like `println!()` or `sqlx::query!()`.

### Common Patterns in This Codebase
- `pub` = public (accessible from other modules)
- `async fn` = async function (like TypeScript's `async function`)
- `.await?` = await the result and propagate errors up (like TypeScript's `await` + `throw`)
- `&self` = borrow self (like Python's `self`)
- `String` vs `&str`: `String` is owned, `&str` is borrowed (like reference)

## HQ Codebase Architecture

HQ follows a **clean architecture** pattern with clear separation of concerns:

```
src/
├── feature/          # Business logic organized by domain
│   ├── user/         # ← YOU'LL WORK HERE
│   │   ├── mod.rs    # The User struct definition
│   │   ├── types.rs  # Request/response DTOs (CreateUser, UpdateUserInfo, etc.)
│   │   ├── service.rs    # Business logic layer
│   │   ├── repository.rs # Data access trait (interface)
│   │   └── error.rs  # Domain-specific errors
│   ├── auth/
│   └── ...
├── infrastructure/   # Concrete implementations of repositories
│   ├── postgres/
│   │   └── user.rs   # ← SQL queries for User
│   └── redis/
├── controller/       # HTTP layer (API routes)
│   └── routes/
│       └── user.rs   # ← User API endpoints
├── core/             # App initialization and dependency injection
└── util/             # Shared utilities
```

### Key Architectural Concepts

**1. Feature Modules** (`feature/user/`)
- **Types** (`types.rs`): DTOs for API requests (CreateUser, UpdateUserInfo)
- **Service** (`service.rs`): Business logic orchestration
- **Repository** (`repository.rs`): Trait defining data access interface (like a TS interface)
- **Errors** (`error.rs`): Domain-specific error types

**2. Infrastructure Layer** (`infrastructure/postgres/user.rs`)
- Implements the repository traits
- Contains actual SQL queries
- Handles database-specific logic and error mapping

**3. Controller Layer** (`controller/routes/user.rs`)
- HTTP endpoint handlers
- Uses `axum` web framework (like Express.js for Node)
- Validates permissions and calls services

**4. Dependency Injection** (`core/app.rs`)
- Wires up services with their dependencies
- AppState holds config and service instances

### How Data Flows

```
HTTP Request → Controller → Service → Repository → Database
                   ↓            ↓          ↓
              (validation) (business) (SQL query)
```

Example: Creating a user
1. `POST /api/v1/user` → `controller/routes/user.rs::create_user()`
2. Controller validates permissions
3. Calls `service.user_service.create_user(CreateUser)`
4. Service generates ID and creates User struct
5. Calls `user_repo.create_user(User)` (trait method)
6. PostgresDb implements it with SQL INSERT
7. Response flows back up

## Your Task: Rename `name` → `username`

You need to change the `name` field to `username` across the entire user feature. Here's what to update:

### Files You'll Modify

**1. Core Type Definition**
- `src/feature/user.rs`: The User struct itself (line 14)

**2. Database Layer**
- `migrations/20251003225658_user.up.sql`: Database schema
- `src/infrastructure/postgres/user.rs`: SQL queries and column names

**3. Business Logic**
- `src/feature/user/types.rs`: CreateUser and UpdateUserInfo DTOs
- `src/feature/user/service.rs`: Service logic using the field
- `src/feature/user/repository.rs`: UpdateUser struct

**4. Tests**
- `tests/user.rs`: Test assertions

### Step-by-Step Approach

**Step 1: Update the Core Struct**
```rust
// In src/feature/user.rs
pub struct User {
    pub id: LazySnowflake,
    pub username: String,  // Changed from 'name'
    pub permissions: PermissionFlags,
}
```

**Step 2: Update DTOs**
```rust
// In src/feature/user/types.rs
pub struct CreateUser {
    pub username: String,  // Changed from 'name'
    // ...
}

pub struct UpdateUserInfo {
    pub username: Option<String>,  // Changed from 'name'
}
```

**Step 3: Update Repository**
```rust
// In src/feature/user/repository.rs
pub struct UpdateUser {
    pub username: Option<String>,  // Changed from 'name'
    // ...
}
```

**Step 4: Update Service Logic**
```rust
// In src/feature/user/service.rs
// When creating a user:
let user = User {
    id: user_id,
    username: data.username,  // Changed from 'name'
    permissions: data.permissions,
};

// When updating:
UpdateUser {
    username: data.username,  // Changed from 'name'
    permissions: None,
}
```

**Step 5: Update Database Layer**

First, create a new migration:
```bash
sqlx migrate add rename_user_name_to_username
```

In the new migration file (`.up.sql`):
```sql
ALTER TABLE users RENAME COLUMN name TO username;
```

In the rollback (`.down.sql`):
```sql
ALTER TABLE users RENAME COLUMN username TO name;
```

Then update `src/infrastructure/postgres/user.rs`:
```rust
// Change SQL queries:
"INSERT INTO users (id, username, permissions) ..."  // was 'name'
.bind(&data.username)  // was 'data.name'

// In update_user:
if let Some(ref username) = data.username {  // was 'name'
    handle_sep!();
    qb.push("username = ").push_bind(username);  // was 'name'
}

// In find_user:
let ident = User {
    id: id.into(),
    username: item.try_get("username")?,  // was 'name'
    permissions: ...
};
```

Also update the error mapping:
```rust
// In map_user_query_error:
if pg_err.constraint() == Some("users_username_key") {  // was 'users_name_key'
    return AppError::Business(BusinessError::User(UserError::DuplicateName));
}
```

**Step 6: Update Tests**
```rust
// In tests/user.rs
let ident = User {
    id: id.as_lazy(),
    username: "hi".into(),  // was 'name'
    permissions: perm.clone(),
};

// In assertions:
assert_eq!(ident_found.username, "hi".to_string());  // was 'name'
```

### Validation Checklist

After making changes:

1. **Build the project**: `cargo build`
   - This checks for type errors and compilation issues

2. **Run tests**: `cargo test`
   - Ensures your changes work correctly

3. **Check the database**:
   - Make sure you have PostgreSQL running
   - Run migrations: `sqlx migrate run`

4. **Look for compiler errors**:
   - Rust compiler is extremely helpful!
   - Read error messages carefully - they tell you exactly what's wrong

### Common Pitfalls

1. **Forgetting database constraint names**: The unique constraint name in SQL error handling
2. **Missing validation function names**: `validate_username` function is fine to keep
3. **SQL column names**: Must match database schema exactly
4. **Option vs direct field**: `UpdateUser` uses `Option<String>`, but `User` uses `String`

### Tips for Success

- **Let the compiler guide you**: Run `cargo build` frequently. Rust's compiler errors are very helpful.
- **Use your editor's LSP**: rust-analyzer will show you errors as you type
- **Search across files**: Use `rg username` to find what you might have missed
- **One file at a time**: Make changes methodically, don't rush
- **Read existing patterns**: The codebase is consistent - follow the patterns you see

### Understanding the Error Flow

When a user tries to create duplicate usernames:
```
1. PostgresDb.create_user() executes SQL INSERT
2. Database rejects (unique constraint violation)
3. map_user_query_error() catches error
4. Checks constraint name: "users_username_key"  ← Update this!
5. Returns UserError::DuplicateName
6. Controller returns 400 Bad Request
```

### Questions to Consider While Working

1. Why does `User` have `String` but `UpdateUser` has `Option<String>`?
   - `User` always has a username, but when updating, you might not want to change it

2. Why separate `repository.rs` (trait) from `postgres/user.rs` (implementation)?
   - Allows swapping database implementations (e.g., for testing with MockUserRepository)

3. What's the difference between `String` and `&str`?
   - `String` owns its data, `&str` borrows it (like a reference)

4. Why use `async/await`?
   - Database operations are I/O-bound, async allows handling many requests concurrently

Good luck! Remember: the Rust compiler is your friend. It catches bugs at compile time that would be runtime errors in TypeScript or Python.
