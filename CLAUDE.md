# Tool guide
- use pnpm, never use npm
- use `tre` for file tree mapping, not `tree`
- never commit to git
- prefer `cargo add` for adding dependencies, over `Cargo.toml` edits

# Explore OpenCode agent Guide

This project uses an automated documentation system that maps Rust crates to specialized markdown files in `claude-docs/`.

## Analysis Modes

### 1. Entire Analysis
- **Trigger:** Use when the project is first indexed or after major refactors.
- **Process:** 1. Run `tre .` to map the file tree.
    2. Identify all crates via `Cargo.toml` workspace or local members.
    3. **Delegation:** Use the `opencode` MCP tool to spawn tasks for each crate.
    4. **Output:** Each crate gets its own `claude-docs/[crate-name].md`.

### 2. Change Analysis
- **Trigger:** Use for routine updates or after a `git commit`.
- **Process:**
    1. Run `git diff --name-only` to find modified files.
    2. Identify which crates are affected.
    3. Update only the relevant `claude-docs/[crate-name].md`.

## Mandatory Documentation Rules
- **One File per Crate:** Explain every file listed in `tre .` for that crate within its specific `.md`.
- **Summary Update:** Every analysis **MUST** update `claude-docs/summary.md` with:
    - Current tech stack (key crates).
    - Overall system architecture.
    - Global data flow.
- **Transaction Brief:** Every response must end with a list of "Changed Docs" and "Changed Code".

## Tool Usage: `opencode` MCP
When performing an **Entire Analysis**, do not analyze all crates in a single serial thread. 
- Use `opencode` to delegate the analysis of a specific crate directory.
- Example: `opencode "Analyze the Rust logic in ./crates/network-engine and update claude-docs/network-engine.md"`
- This prevents context overflow and allows for cleaner, parallelized crate summaries.
- Use `rustanalysis` as the agent name for all `opencode` tasks.
- Use "ollama" for providerID and "qwen3.5-code" for modelID in `opencode` calls.

## `opencode` MCP's other use
For tasks that require fast speed and repetition, use `build` agent with `opencode_run` for quick execution without the overhead of a full analysis. Also use it for exploring the codebase.
