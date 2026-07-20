# Implementation Audit — 2026-07-20

## Context

Repository already contains a working **v0.1.0** unsigned Windows release
(`Super-YYQ/cc-switch-doctor`, tag `v0.1.0`). This audit drives the **v0.1.1**
productization goal (UI redesign + remaining spec gaps).

## Keep (do not rewrite)

| Area                 | Assessment                                                                                          |
| -------------------- | --------------------------------------------------------------------------------------------------- |
| Rust `ccs_adapter`   | Readonly SQLite, schema fingerprint v15, managed-auth skip, credential extract per app_type — solid |
| `security/*`         | Redactor, same-origin, URL variants, redirect block — meets hard rules                              |
| `protocols/*`        | Chat / Responses / Anthropic / Gemini builders + SSE + tool-call parse                              |
| `diagnostics/*`      | Quick/Smart/Deep planner + engine + classifier                                                      |
| Security scripts     | process-spawn / protected-path / version-sync gates                                                 |
| CI/Release workflows | Windows-first CI, NSIS + portable + SHA256SUMS release                                              |
| Fixture SQL          | Synthetic keys only                                                                                 |

## Must fix

| Gap                                                                 | Severity       |
| ------------------------------------------------------------------- | -------------- |
| Monolithic `App.tsx` table UI — looks like a debug console          | High (product) |
| Safety banner occupies first screen                                 | High (UX)      |
| Right pane is raw log-first                                         | High (UX)      |
| OpenAI uses only `max_tokens` (needs `max_completion_tokens` first) | Medium         |
| No per-host session request budget (30)                             | Medium         |
| Chinese conclusions are technical codes, not product copy           | Medium         |
| No synthetic screenshots / visual regression                        | Medium         |
| `v0.1.0` already shipped → next version **v0.1.1**                  | Process        |

## Security risks reviewed

- No process spawn / shell plugin in production code (CI gate green)
- No protected login path access
- Keys never leave Rust for IPC list items
- Cross-host redirects blocked without credentials
- Residual: ensure ResultCard copy/debug log still redacted (already backend-redacted)

## UI/UX main problems

1. Header overcrowded with long safety card
2. Hard HTML table with broken long-URL wrap
3. Equal-weight buttons; primary CTA unclear
4. Results = log terminal
5. No design tokens / inconsistent spacing

## Refactor scope for v0.1.1

- **Rewrite frontend layout and components** (keep API layer)
- **Keep Rust core**; surgical patches for token field + host budget + conclusion helpers
- **Add design tokens + component CSS**
- **Synthetic screenshots** for README
- **Ship v0.1.1** (do not overwrite v0.1.0)

## Why not full Rust rewrite

Backend already passes 35 unit tests, security gates, and production Windows build.
Rewriting would burn schedule without safety gain. UI is the main product deficit.
