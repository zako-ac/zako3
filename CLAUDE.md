# Tool guide
- When you have to install package, use pnpm, never use npm
- When you need to tree files, use `tre` for file tree mapping, not `tree`
- never commit to git
- prefer `cargo add` for adding dependencies, over `Cargo.toml` edits

# Config Addition Guide
1. Update the source code, of course.
2. Update corresponding .env.example
3. Update helm: Always grep for the config key.
