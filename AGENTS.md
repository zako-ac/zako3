# Agent Working Guidelines for Zako3 Monorepo

Welcome to the `zako3` repository. This file serves as a comprehensive guide for AI agents and coding assistants operating in this codebase.

## Repository Structure Overview
This is a polyglot monorepo containing both Rust and TypeScript code:
- **Rust**: Uses Cargo workspaces. Directories include `audio-engine/`, `hq/`, `taphub/`, `zakoctl/`, `zakofish/`, `types/`, and `emoji-matcher/`.
- **TypeScript/Node.js**: Uses `pnpm` workspaces. Directories include `web/`, `packages/`, and `hq/` (some TS components may live here based on pnpm-workspace.yaml).

## 1. Build, Lint, and Test Commands

### Rust Ecosystem
We use standard Cargo commands, formatted by `rustfmt.toml`.
- **Build**: `cargo build` (or `cargo build --package <pkg_name>`)
- **Lint (Clippy)**: `cargo clippy --workspace --all-targets -- -D warnings`
- **Format**: `cargo fmt --all` (check style with `cargo fmt --all -- --check`)
- **Run all tests**: `cargo test --workspace`
- **Run tests for a specific package**: `cargo test --package <pkg_name>`
- **Run a single test**: `cargo test --package <pkg_name> <test_function_name> -- --nocapture`
  *Note: Always use `--nocapture` when running individual tests to ensure log outputs (tracing) are visible for debugging.*

### TypeScript Ecosystem
We use `pnpm` as the package manager.
- **Install dependencies**: `pnpm install`
- **Build all**: `pnpm -r build`
- **Build specific package**: `pnpm --filter <pkg_name> build`
- **Lint**: `pnpm -r lint` (or filter by package: `pnpm --filter <pkg_name> lint`)
- **Run all tests**: `pnpm -r test`
- **Run a single test (Vitest/Jest common setup)**: `pnpm --filter <pkg_name> test -t "<test_name_pattern>"`

## 2. Code Style & Architectural Guidelines

### General Principles
- **Idiomatic Code**: Follow idiomatic patterns for the language you are writing in. Do not write TypeScript-flavored Rust or Rust-flavored TypeScript.
- **Minimal Dependencies**: Do not introduce new dependencies (`Cargo.toml` or `package.json`) unless absolutely necessary. If required, favor widely used, well-maintained libraries.
- **Keep it Simple**: Avoid over-engineering. Prefer simple, readable, and maintainable solutions over complex abstractions.
- **Leave No Trace**: Clean up any temporary files or debug artifacts after you complete a task.

### Rust Style Guidelines
- **Formatting**: Strictly rely on `rustfmt`. Do not manually debate spacing; just run `cargo fmt`.
- **Error Handling**: 
  - Use `thiserror` for library error definitions (`#[derive(Error)]`).
  - Use `anyhow` for application-level error handling where appropriate.
  - Never use `.unwrap()` or `.expect()` in production code unless you can absolutely guarantee the invariant statically. Propagate errors using `?`.
- **Typing**: Use strong typing. Favor enums to represent mutually exclusive states.
- **Asynchrony**: We use `tokio` for async runtimes. Be mindful of blocking the async thread; use `tokio::task::spawn_blocking` for CPU-heavy or synchronous I/O operations.
- **Logging**: Use the `tracing` ecosystem (`tracing::info!`, `tracing::error!`, etc.) over standard `println!`.
- **Naming Conventions**:
  - `snake_case` for variables, functions, macros, and modules.
  - `PascalCase` for Types, Traits, and Enums.
  - `SCREAMING_SNAKE_CASE` for statics and constants.
- **Imports**: Group imports. Standard library first, then external crates, then crate-local modules.

### TypeScript Style Guidelines
- **Typing**: Use strict TypeScript. Avoid `any` at all costs. Use `unknown` if the type is truly dynamic, and type-narrow it.
- **Interfaces vs Types**: Prefer `interface` for object shapes that might be extended. Use `type` for unions, intersections, and primitives.
- **Error Handling**: Use `try/catch` blocks for async code or promises. Return explicit `Result`-like types or throw meaningful custom error classes.
- **Naming Conventions**:
  - `camelCase` for variables, functions, and object properties.
  - `PascalCase` for Classes, Interfaces, and Type Aliases.
  - `SCREAMING_SNAKE_CASE` for global constants.
- **Exports**: Prefer named exports over default exports for better refactoring and discoverability.
- **Formatting**: Rely on `Prettier` (if installed) or ESLint auto-fix for formatting.

## 3. Workflow for Autonomous Agents
When you receive a task:
1. **Analyze**: Use `grep` or `glob` to locate the relevant files. Read configuration files (e.g., `Cargo.toml`, `pnpm-workspace.yaml`) to understand the module structure.
2. **Plan**: Write a concise plan (1-3 sentences) detailing the files you will modify and the approach you will take.
3. **Execute**: Make the necessary edits using the `edit` or `write` tool.
4. **Verify**: Run the respective linter (`cargo clippy` or `pnpm lint`) and tests for the modified module to ensure no regressions were introduced. Fix any compilation or linting errors immediately.
5. **Document**: If making significant architectural changes or adding new functionality, update the relevant `README.md` or `docs/` folder appropriately.

*Follow these instructions closely to maintain a healthy and consistent codebase.*
