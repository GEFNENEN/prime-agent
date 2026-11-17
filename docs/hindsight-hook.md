# Hindsight hooks (Rust bridge)

The Rust build does not load the JavaScript extension host, so the Hindsight
plugin's `before_agent_start` / `agent_end` hooks no longer run. This bridge
restores the same two behaviors without Node:

- **prompt**: one reflection per session, injected as the last block of the
  system prompt (`<hindsight_memory>…</hindsight_memory>`), plus a periodic
  knowledge-page roster.
- **stop**: writes the run's new turns back into the session document.

Both commands are hidden `prime-agent` subcommands that speak the existing
`turnHook` / `stopHook` stdin/stdout contracts. No session-engine change is
needed.

The roster names pages only. This bridge registers no Hindsight MCP tools, so
opening a roster page requires a separately configured Hindsight MCP server.
The default cadence stays at 10 turns; `0` disables the roster.

## Wiring

Add both hooks to the agent `settings.json` (use the absolute path of the
binary that runs the session; on Windows quote it inside the JSON string):

```json
"turnHook": {
  "command": "\"C:\\path\\to\\prime-agent.exe\" hindsight-hook prompt",
  "timeoutMs": 30000
},
"stopHook": {
  "command": "\"C:\\path\\to\\prime-agent.exe\" hindsight-hook stop",
  "timeoutMs": 70000
},
```

Do not keep the JavaScript Hindsight hooks wired for the same harness: two
producers would reflect and retain twice.

Both commands are fail-open: a missing server, a malformed payload, a
timeout, or a non-2xx response leaves the turn unchanged and the hook exits
0. The prompt command prints nothing when it has no context.

## Configuration

The bridge reads the same file as the JS plugin: `HINDSIGHT_CONFIG`, or
`~/.hindsight/coding-agent.json`. Unknown keys are ignored. Honored fields:

| field | default | meaning |
| --- | --- | --- |
| `apiUrl` | `http://127.0.0.1:8888` | server base URL; set it for cloud |
| `apiToken` | none | bearer token |
| `serverMode` / `apiPort` | `self-hosted` / `9077` | `daemon` maps to `http://127.0.0.1:<port>` |
| `bankId`, `dynamicBankId`, `bankIdTemplate` | — | fixed or templated bank |
| `mapPathToBank` | `{}` | longest path prefix → bank |
| `banks.<id>` | `{}` | `disabled`, `autoReflect`, `retainSessions`, timeouts, cadence, `bank` rename |
| `autoReflect` | `true` | `false` injects only the roster |
| `reflectTimeoutMs` | `120000` (capped at 20s in the hook) | reflect budget |
| `retainSessions` | `true` | enable stop write-back |
| `resolveWorktrees` | `true` | a worktree uses the main repo's bank |
| `pageRefreshEveryTurns` | `10` | roster cadence; `0` disables |
| `optInOnly`, `optInPaths` | `false`, `[]` | restrict which directories get a bank |
| `harness` | `prime-agent` | value for the `{harness}` placeholder |

Environment overrides: `HINDSIGHT_API_URL`, `HINDSIGHT_API_TOKEN`,
`HINDSIGHT_SERVER_MODE`, `HINDSIGHT_API_PORT`, `HINDSIGHT_BANK_ID`,
`HINDSIGHT_DYNAMIC_BANK_ID`, `HINDSIGHT_BANK_ID_TEMPLATE`,
`HINDSIGHT_AUTO_REFLECT`, `HINDSIGHT_REFLECT_TIMEOUT_MS`,
`HINDSIGHT_RETAIN_SESSIONS`, `HINDSIGHT_RESOLVE_WORKTREES`,
`HINDSIGHT_PAGE_REFRESH_EVERY_TURNS`, `HINDSIGHT_HARNESS`,
`HINDSIGHT_DISABLED`, `HINDSIGHT_DISABLE_HOOKS`. The default server is
local; set `apiUrl` (or `HINDSIGHT_API_URL`) for a hosted deployment.
The `harness` field (or `HINDSIGHT_HARNESS`) fills the `{harness}` bank
placeholder.

Banks are `coding-agent::<repo>` from the main worktree's directory name.
`mapPathToBank` wins over the template; a git probe failure falls back to the
working directory's name rather than guessing another repository.

## Session state

State lives under `<agent dir>/hindsight/sessions/<session id>/`:

- `prompt.json`: `turns`, `reflectAnswer` (present even when empty, which
  means "already attempted"), and the roster (`atTurn`, page titles).
- `retain.json`: retained turn count, prefix fingerprint, bank.

The cache is per `session_id`, so a worker restart resumes it instead of
reflecting again. Later turns do no network work unless the roster cadence
elapsed.

When reflection fails, the hook tries page search, then consolidated
observations, inside a 7s fallback budget. If all three fail it injects
nothing and caches the failed attempt (the JS semantics).

Write-back uses `document_id = conversation:<session id>` and an idempotent
`operation_id` (UUID v5 of bank + ref + mode + content). A transcript prefix
that still matches the cursor appends only the new turns; a rewritten or
truncated prefix replaces the whole document. The request is `async: true`,
so the hook returns after the server enqueues extraction.

## Disabling

Remove the two settings entries, or set `"disabled": true` in the config
file / `HINDSIGHT_DISABLE_HOOKS=1` in the environment.

## Verification

```bash
cargo test -p pa-core hindsight
cargo test -p pa-cli --test hindsight_hook_e2e
```

The tests use an in-process fake HTTP server and a temp agent directory;
they never read the user's settings or touch a real bank.
