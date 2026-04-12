# Tool guide
- NEVER use sub-agent for explore task. Your sub-agent is only allowed to be used for code modification, never for exploring. For exploring, use `plan` agent with `opencode` MCP.
- When you have to install package, use pnpm, never use npm
- When you need to tree files, use `tre` for file tree mapping, not `tree`
- never commit to git
- prefer `cargo add` for adding dependencies, over `Cargo.toml` edits

- When user says "oc explore" or similar thing, it means to use `plan` agent with `opencode` MCP to explore the codebase, not to spawn an "explore" agent. Always use `plan` agent for exploring tasks, and never use your own agent for that purpose.
- When you need to spawn "explore" agent, don't use your own one. Instead, use "plan" agent with `opencode` MCP.

## Tool Usage: `opencode` MCP
- Use `plan` as the agent name for all `opencode` tasks.
- Use "ollama" for providerID and `gemma4-code` (exact gemma4-code NOT -coder.) for modelID in `opencode` calls.

For tasks that require fast speed and repetition, use `build` agent with `opencode_run` for quick execution.
For task that require fast speed and repetition and is READ ONLY, use `plan` agent with `opencode`. Also use it for exploring codebase.

# Config Addition Guide
1. Update the source code, of course.
2. Update corresponding .env.example
3. Update helm: Always grep for the config key.
