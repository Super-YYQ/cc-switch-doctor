# Architecture

```text
React UI  --opaque ids / masked data-->  Tauri commands
                                            |
                                            v
                              +---------------------------+
                              | Rust Application Core     |
                              |  ccs_adapter              |
                              |  security                 |
                              |  diagnostics planner      |
                              |  protocols + HTTP exec    |
                              |  result classifier        |
                              +---------------------------+
```

## Modules

### `ccs_adapter`

- Path discovery (`~/.cc-switch/cc-switch.db`, custom dir, legacy HOME, manual pick)
- Read-only SQLite
- Schema fingerprint → Verified / Compatible / Unknown / Unsupported
- Provider normalize per app_type
- Managed auth detection

### `security`

- Key masking & error redaction
- Same-origin policy
- URL variant generation (no cross-host)

### `diagnostics`

- Modes: quick / smart / deep
- Attempt budget & stop conditions
- Event stream to UI

### `protocols`

- Request builders for Chat / Responses / Anthropic / Gemini
- Streaming SSE parse
- Tool-call structure check (no tool execution)

## Non-goals encoded in structure

- No shell plugin in capabilities
- No write DAO
- No persistence layer
