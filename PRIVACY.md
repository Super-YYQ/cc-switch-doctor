# Privacy Policy — CC Switch Doctor

Last updated: 2026-07-20

## Summary

CC Switch Doctor is designed to be **stateless** and **local-first**.

## What we do NOT do

- We do **not** create an application database, config file, history file, or cache of providers.
- We do **not** use localStorage, sessionStorage, IndexedDB, Tauri Store, or telemetry SDKs.
- We do **not** upload your API keys, provider names, database paths, or device identifiers to our servers (there is no Doctor backend).
- We do **not** read Codex/Claude/OpenCode/Gemini login directories.

## What stays in memory

- Full API keys are loaded from the CC Switch SQLite database into **Rust process memory only**.
- Keys are used to call the **same host** as the configured Base URL.
- Keys are zeroized/dropped when the process exits or the provider snapshot is replaced.
- The UI only receives masked keys (e.g. `sk-abcd…wxyz`).

## Network destinations

1. **Your configured provider Base URLs** — diagnostic HTTP requests you explicitly start.
2. **GitHub API** (`api.github.com`) — optional update checks for CC Switch / Doctor releases. No secrets are sent.
3. System proxy environment variables (`HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY`) may route traffic if set.

## What third parties may log

- Upstream model providers may log requests, prompts, IPs, and usage.
- Corporate proxies, antivirus HTTPS inspection, and Windows itself may keep network or crash records outside this app.

## Clipboard

If you click **复制诊断摘要**, text is written to the **system clipboard** (external state). The summary is redacted and must not contain full keys.

## Contact

Security issues: see [SECURITY.md](SECURITY.md).
