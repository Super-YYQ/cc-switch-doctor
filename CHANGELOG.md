## [0.1.4] — 2026-07-21

### Fixed

- Schema v13 exact compatible fingerprint so real CC Switch DBs load Providers again
- Core app filter chips always visible; default filter is Claude (not All)
- Provider rows no longer auto-selected after scan; start requires explicit check
- Provider overflow menu closes on outside click / Esc / item action
- Async single-flight (no Condvar blocking Tokio workers)
- Classify HTTP 200 business errors (quota/auth/WAF) before protocol mismatch
- Cancel waits for run_finished before allowing a new run (no late event races)
- AUTH_INVALID / QUOTA stop strategy; Gemini assert without `|| true`

### Changed

- Clearer empty states for schema block / filter / search

## [0.1.3] — 2026-07-21

### Fixed

- Request cache key now includes final URL, method, auth scheme, UA/body fingerprints (no cross-variant reuse)
- Concurrent identical requests single-flight reuse
- CC Switch / discovery with / UNC expansion
- Run cancel by matching runId; complete_run isolation; frontend ignores late events
- Schema gate no longer treats wide user_version ranges as Compatible
- Anthropic AUTH_TOKEN uses Bearer; API_KEY uses x-api-key for current-config tests
- UTF-8 safe truncation; URL display masks all query values and path secrets
- Error classification priority (auth/quota/WAF before UNSUPPORTED_PROTOCOL)
- Gemini path de-duplication

### Changed

- Concurrency selectable 1/2/3 (default 1); mode tooltips and short descriptions
- Default-select CC Switch current providers after scan
- Chinese status badges; possible-causes for low-confidence protocol issues

## [0.1.2] — 2026-07-21

### Fixed

- Shared Host request budget (30) across the entire diagnosis session, not per provider.
- Consecutive `RATE_LIMITED` (HTTP 429) stops further requests to that Host for the run.
- In-session memory cache reuses identical request combinations (key fingerprint only; never stores full keys).
- OpenAI Chat truly falls back from `max_completion_tokens` to `max_tokens` when the field is explicitly unsupported.
- Refresh / select-DB / first scan clears selected, active, summaries, live log, and run state.
- Distinguish CC Switch `latestObservedRelease` vs Doctor `latestVerifiedRelease` in update messaging.
- Upstream Watch ensures `upstream-change` label exists and dedups open issues by exact title.
- Remove ineffective optional code-signing step; release notes state **This release is unsigned.**
- Pin all GitHub Actions to full 40-char commit SHAs; add `scripts/verify-actions-pinned.mjs`.
- Strong release version consistency checks (`package.json` / Cargo.toml / tauri.conf.json / manifest).
- Windows UNC SQLite path URI construction (`file://server/share/...`).
- Provider card accessibility: plain `<article>` + checkbox + explicit “查看详情”.
- Readonly DB test asserts SHA-256 file stability (not just size).

### Changed

- Move project design specs under `docs/project/`.
- Document future signing plans in `docs/code-signing.md` without affecting current releases.

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
