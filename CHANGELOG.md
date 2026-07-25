## [0.1.8] — 2026-07-25

### Fixed

- **P0:** Unknown CC Switch `user_version` no longer empties Provider list or blocks direct diagnosis when core structures remain compatible
- Version verification separated from runtime capability detection (Verified label ≠ can-run gate)
- Routing structure failures degrade routing only; Provider scan and Direct Diagnosis continue
- Single Provider `settings_config` parse failure no longer blocks other Providers
- Missing optional provider columns degrade instead of failing the whole schema allowlist match
- Endpoint table absence falls back to `settings_config` Base URL extraction (Degraded)

### Added

- Capability model: `VersionVerification`, `SchemaCapabilities`, `CapabilityStatus` (`supported` / `degraded` / `disabled`)
- CC Switch **3.18.0 / schema v16** verified entry (upstream commit `878c26f31e012ba32b9772bd080bd4fa9e7d495e`)
- Future-version structure compatibility path (`UnverifiedStructureCompatible`)
- Synthetic fixtures: v16, future v17 same-core, extra columns, required-column missing, endpoints missing, routing unknown, one-provider-invalid
- Header UI shows version verification and Provider / Direct / Routing capability badges separately

### Security

- Required core columns still required; missing `settings_config` disables sensitive reads
- Capability fail-closed for affected sensitive features only
- SQLite remains read-only; no `user_version` writes; no full keys in frontend

## [0.1.7] — 2026-07-21

### Fixed

- **P0:** `CCS_ROUTE_NOT_APPLICABLE` / `CCS_ROUTE_NOT_RUNNING` / DirectOnly Skip no longer override real direct outcomes (`NETWORK_UNREACHABLE`, `AUTH_INVALID`, etc.)
- Provider primary status equals `direct_status` when no real CCS route business request was sent (`CcsLocalRoute && http_sent`)
- Route target and failover counts refreshed via `GET /status` before and after route probes
- Per-app route probe single-flight (async Leader/Waiter) so concurrency 2/3 cannot double-send
- Claude route model aliases sourced from compatibility `routingProfiles` (`claude-sonnet-5` etc.); removed hard-coded `claude-sonnet-4-20250514`
- UI primary badge uses `primaryOutcome` only; neutral “路由未验证” chip for disposition-only cases
- Attempt chain groups cache reuse vs real sends; top real-request count uses `http_sent` only

### Added

- Layered outcome model: `primary_outcome` + `DirectChannelSummary` + `RouteChannelSummary` / `RouteDisposition`
- Source review gate: `docs/research/v0.1.7-source-review.md` (CC Switch / Codex / Anthropic SDK / Gemini CLI / OpenCode)
- Protocol adapter registry + sanitized fixture corpus under `tests/fixtures/protocols/`

### Security

- Route probes still use `PROXY_MANAGED` only; non-loopback blocked; no CLI spawn; proxy_config SELECT-only

## [0.1.6] — 2026-07-21

### Added

- Dual-channel diagnosis: Direct Upstream vs CCS Local Route
- Read-only `proxy_config` discovery + loopback `/health` `/status` probes
- Verify mode: Auto / Direct only / Direct + CCS route
- Client-protocol route attempts with `PROXY_MANAGED` placeholder only
- Provider ↔ Result bidirectional navigation and result index control
- Routing status chips and dual-channel ResultCard sections

### Fixed

- Cross-protocol / loose-field success no longer becomes CURRENT_CONFIG_OK
- `error: null/false/""/{}/[]` no longer short-circuits success parse
- Streaming cross-protocol deltas + 2MB raw buffer fallback
- Streaming non-2xx bodies bounded (no `response.text()`)
- Broader Anthropic/OpenAI/Gemini text extractors
- Default window size 1100×740

### Security

- Route requests never carry provider real keys
- Non-loopback listen addresses blocked from auto probe
- proxy_config remains SELECT-only

## [0.1.5] — 2026-07-21

### Fixed

- Success responses containing `billing_usage` / `usage` no longer misclassified as `QUOTA_EXHAUSTED`
- Free-text error heuristics only run after native and cross-protocol success parsing fails
- Structured 2xx error envelopes (`error`, `success:false`) still detect quota/auth correctly
- Cross-protocol response parsing (Anthropic↔OpenAI↔Responses↔Gemini) with `RESPONSE_PROTOCOL_VARIANT_OK`
- Stream responses that return full JSON/NDJSON instead of SSE deltas now parse successfully
- URL path secrets redacted for display, events, cache keys, and excerpts
- Cache keys distinguish non-secret query values and never embed path keys
- Provider-level real send budgets (Quick 2 / Smart 12 / Deep 16) in addition to Host 30
- Incremental non-stream body reads stop at 2MB without full download
- Content-Type `text/html` on 2xx classified as gateway/WAF before JSON parse
- Gemini supports Header `x-goog-api-key` and Query `?key=` auth variants
- Guessed models no longer reported as current-config success (`MODEL_GUESS_OK`)
- Frontend `ErrorEvidence` type and ResultCard “判定依据” panel
- Compact UI density so more providers/results fit on 768–900px height windows

### Tests

- Anthropic success + `billing_usage` regression
- Cross-protocol / wrapper / stream-fallback parser tests
- Path key redaction and cache-key hardening
- synthetic-v13 end-to-end scan + SHA256 stability
- Manifest schema fingerprints vs runtime allowlist consistency
- ResultCard evidence + collapsed debug log

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
