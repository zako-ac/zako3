---
name: zako-mcp
description: >-
  How to connect to and use the zako3 HQ control-plane over MCP. The HQ backend
  exposes its full REST API — taps, playback, settings, notifications, guilds,
  API tokens, and the admin surface — as MCP tools at POST /mcp, plus a GET
  /mcp/sse stream for playback notifications. Use this skill whenever the user
  wants to connect an MCP client to zako / zako3 / HQ, mentions the HQ MCP
  server or the /mcp endpoint, asks how to authenticate to it, wants to call HQ
  tools (e.g. create a tap, control playback, manage settings, run admin
  actions), or asks about zako playback notifications over SSE — even if they
  don't say the word "MCP" explicitly.
---

# zako3 HQ over MCP

The zako3 **HQ backend** is the control plane for the zako platform (taps, audio
playback, user/guild settings, verification, mappers, admin). Every data and
action endpoint of its REST API is also exposed as an **MCP tool**, so an MCP
client (Claude, an agent, an IDE) can drive HQ with plain tool calls instead of
hand-rolling HTTP requests.

Transport: **Streamable HTTP**.
- `POST /mcp` — JSON-RPC: `initialize`, `tools/list`, `tools/call`.
- `GET /mcp/sse` — server→client notification stream (playback changes).

## Installation

`zako.ac` is a hosted MCP server for AI agents — there is nothing to install,
host, or run. Point your MCP client at the endpoint:

- JSON-RPC: **`https://zako.ac/mcp`**
- SSE stream: **`https://zako.ac/mcp/sse`**

You supply an access token (see [Authentication](#authentication)). No local
daemon or account setup is needed on the agent side.

## Authentication

Authentication is a **JWT bearer token that the user provides** — this skill
does not issue or fetch one. Configure it in your MCP client and send it on
every request to `/mcp`:

```
Authorization: Bearer <JWT>
```

Set the token through your client's secret/env mechanism (e.g. a `ZAKO_JWT`
environment variable) rather than hard-coding it. It is a zako HQ user JWT and
expires (`exp`), so refresh or replace it when calls start failing with auth
errors. **If you don't have a token, ask the user to provide or configure one.**

Tools enforce access per call:
- **public** — work with or without a token (e.g. `list_taps`, `get_tap`).
- **user** — require a valid bearer (e.g. `get_me`, playback, API tokens).
- **admin** — require a bearer **and** an admin account (the `admin_*` tools).

A missing token on a user tool, or a non-admin token on an admin tool, returns a
tool error like `authentication required` / `admin permissions required` rather
than crashing.

## Connecting an MCP client

For a client that speaks Streamable HTTP with custom headers, point it at `/mcp`
and set the `Authorization` header (use your client's env-var syntax for the
token rather than pasting it literally):

```json
{
  "mcpServers": {
    "zako-hq": {
      "type": "http",
      "url": "https://zako.ac/mcp",
      "headers": { "Authorization": "Bearer ${ZAKO_JWT}" }
    }
  }
}
```

Send the MCP protocol-version header on raw requests (clients usually do this for
you): `mcp-protocol-version: 2025-11-25`.

## Brief usage

Discover tools, then call them. With an MCP client, `tools/list` and
`tools/call` are normal client operations. With raw HTTP:

```bash
# List available tools
curl -s https://zako.ac/mcp \
  -H 'content-type: application/json' \
  -H 'mcp-protocol-version: 2025-11-25' \
  -H "Authorization: Bearer $ZAKO_JWT" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'

# Call a tool (get the current user)
curl -s https://zako.ac/mcp \
  -H 'content-type: application/json' \
  -H 'mcp-protocol-version: 2025-11-25' \
  -H "Authorization: Bearer $ZAKO_JWT" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"get_me","arguments":{}}}'
```

Tool results come back as JSON text (the same DTOs the REST API returns). Path
and body parameters are passed flat in `arguments` — e.g. `get_tap` takes
`{"tap_id": "..."}`, `update_my_guild_settings` takes `{"guild_id": "...", ...settings}`.

### Tool groups

| Group | Example tools | Tier |
|---|---|---|
| Users | `get_me`, `get_my_taps`, `get/update_my_settings`, `get_effective_settings` | user |
| Taps | `list_taps`, `get_tap`, `create_tap`, `update_tap`, `delete_tap`, `get_tap_stats` | public/user |
| API tokens | `create/list/update/delete/regenerate_tap_api_token` | user |
| Settings | `get/update_guild_settings`, `get/update_global_settings` | user |
| Guilds | `get_my_guilds`, `admin_get_user_guilds` | user/admin |
| Notifications | `list_notifications`, `get_unread_notification_count`, `mark_notification_read` | user |
| Playback | `get_playback_state`, `stop/pause/resume_track`, `skip_music`, `edit_queue`, `get_playback_history` | user |
| Admin | `admin_list_users`, `admin_ban_user`, `admin_update_user_role`, `admin_*_verification`, `admin_get_platform_stats` | admin |
| Mappers | `admin_list_mappers`, `admin_create_mapper`, `admin_set_mapper_pipeline`, `admin_evaluate_mapper` | admin |

Use `tools/list` for the authoritative, up-to-date set and each tool's input
schema.

### Playback notifications (SSE)

Open `GET /mcp/sse` to receive a server-pushed event whenever playback state
changes anywhere on the instance:

```json
{ "jsonrpc": "2.0", "method": "notifications/playback/changed" }
```

It is a **content-free "refetch" ping** — it carries no guild, track, or user
data and is broadcast to all listeners. On receiving it, call
`get_playback_state` (bearer-scoped to *your* guilds) to read the new state.

## Notes & gotchas

- There is no login/OAuth tool — `zako.ac` is purely an MCP endpoint. The token
  is obtained out-of-band and supplied by the user; MCP only consumes it.
- `create_mapper` / `update_mapper` take the WASM module as a JSON byte array
  (`wasm_bytes`), since MCP tool inputs are JSON, not multipart uploads.
- If a tool returns an auth error, re-check the `Authorization` header and that
  the token hasn't expired.
