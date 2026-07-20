## [0.1.1] — 2026-07-20

### Changed

- Productized UI: AppHeader, SessionControlBar, card Provider list, structured ResultCards.
- Safety explanation moved to drawer; no longer blocks the full first screen.
- Right pane is conclusion-first; attempt chain and debug logs are collapsed by default.
- Design tokens, light-first theme, CC Switch companion visual language.
- OpenAI Chat requests prefer `max_completion_tokens`.
- Per-host request budget (30) enforced during diagnosis runs.

### Fixed

- Long URL/model wrapping in provider rows (ellipsis + tooltip).
- Primary CTA hierarchy (开始诊断 is the only strong action).

## [0.1.0] — 2026-07-20

### Added

- Initial Windows x64 release of CC Switch Doctor.
- Read-only CC Switch database discovery and schema fingerprinting (verified against v3.17.0 / schema 15).
- Provider listing with app filters; managed OAuth/Copilot hard-skip.
- Quick / Smart / Deep diagnosis modes.
- Protocols: OpenAI Chat Completions, OpenAI Responses, Anthropic Messages, Gemini Native.
- Streaming SSE and Tool Calling probes (deep mode).
- Same-origin URL variants, `/v1` normalize, cross-host redirect block.
- Security CI gates, upstream-watch, and Windows release workflow.
- Portable ZIP + NSIS setup + SHA256SUMS release assets.
