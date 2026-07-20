# AGENTS.md

本项目绝不启动任何 AI CLI、绝不读取 Codex/Claude/OpenCode 登录目录、绝不写入 CC Switch 数据库、绝不持久化 Key 或诊断结果。

## Hard rules

1. Pure HTTP only (`reqwest`). No `std::process`, `tokio::process`, shell plugins.
2. Never read `.codex`, `.claude`, OpenCode home, `.gemini` login paths.
3. CC Switch SQLite: `mode=ro` + `query_only=ON` only.
4. Full API keys stay in Rust memory; frontend receives masked values only.
5. Auto URL/protocol variants must stay same-origin; block cross-host redirects with credentials.
6. Stateless: no localStorage, no app DB, no history files.
7. Managed OAuth / Copilot / ChatGPT backend / official subscription configs are hard-skipped with no bypass.
8. Before changing adapter logic, re-check `farion1231/cc-switch` latest release and update `compatibility/manifest.json`.

## Quality gates

```bash
npm ci
npm run format:check
npm run lint
npm run typecheck
npm run test
npm run build
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run security:verify
npm run tauri build
```

## Layout

- `src/` React UI
- `src-tauri/src/ccs_adapter` CC Switch readonly adapter
- `src-tauri/src/diagnostics` planner + engine + classifier
- `src-tauri/src/protocols` protocol builders + HTTP executor
- `src-tauri/src/security` redaction, origin, URL variants
- `scripts/` security and packaging gates
- `compatibility/` verified schema fingerprints and fixtures
