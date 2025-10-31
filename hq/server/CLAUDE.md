# Claude Code Generation Notes

## Patterns and Best Practices for This Codebase

### Database Repository Implementation

1. **PostgreSQL Repository Pattern**:
   - Implement async trait from feature module
   - Use `sqlx::query` for simple queries with bind parameters
   - Use `QueryBuilder` for dynamic UPDATE queries with optional fields
   - Cast snowflake IDs to `i64` for PostgreSQL BIGINT compatibility
   - Check `rows_affected()` for UPDATE/DELETE to return `AppError::NotFound` when appropriate

2. **Migration Files**:
   - Up migrations: Create tables with BIGINT PRIMARY KEY for IDs
   - Add UNIQUE constraint for fields that should be unique (e.g., name)
   - Down migrations: Simply DROP TABLE

3. **Type Conversions**:
   - Snowflake IDs: `*id as i64` for binding, `id as u64` after fetching
   - For newtype wrappers like `TapName(String)`: Use `&**name` or `&*name` for bind, direct assignment for fetch
   - LazySnowflake: Use `id.into()` to convert from `u64`

4. **Query Patterns**:
   - CREATE: `INSERT INTO table (fields) VALUES ($1, $2)`
   - READ: `SELECT * FROM table WHERE id = $1` with `fetch_optional`
   - UPDATE: Use QueryBuilder with macro for comma separation, check rows_affected
   - DELETE: Standard delete with rows_affected check

5. **Test Structure**:
   - Use test helper `init_postgres()` from common module
   - Test CRUD operations in order: create, read, update, delete
   - Test error cases: NotFound for missing IDs, constraint violations
   - Create entity with `Snowflake::new_now()` for unique IDs
   - Use `.into()` for newtype conversions in assertions

6. **Module Registration**:
   - Add module to `src/infrastructure/postgres.rs` with `pub mod name;`
   - No need to explicitly add to PostgresDb impl - trait impl is automatic

### Common Pitfalls

- Don't forget to dereference newtype wrappers when binding (`&**name` not `&name`)
- Remember to check `rows_affected() > 0` for UPDATE/DELETE operations
- Cast IDs properly: `i64` for database, `u64` for Snowflake
- Use `fetch_optional` for single results, not `fetch_one`
- In tests, use `.clone()` on entities to avoid move issues

### Code Generation Checklist

When implementing a new feature with PostgreSQL:
- [ ] Create migration files (.up.sql and .down.sql)
- [ ] Implement repository trait in src/infrastructure/postgres/
- [ ] Register module in src/infrastructure/postgres.rs
- [ ] Create test file in tests/ directory
- [ ] Ensure test uses common::postgres::init_postgres
- [ ] Test all CRUD operations and error cases
