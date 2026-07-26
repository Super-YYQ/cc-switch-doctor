## [Unreleased]

### Fixed

- **P0:** `DiagnosisEvent` field names now serialize as camelCase (`runId`, `opaqueId`, …) so frontend progress, run isolation, and cancel/restart no longer break on live Tauri events
- **P0:** Structured/HTTP error `error_evidence[].message` is redacted via `redact_result` before reaching the UI
- **P0:** Managed-auth skip also inspects `provider_endpoints` base URLs (Copilot / ChatGPT backend no longer miss skip when only endpoints hold the host)

### Security

- Full keys in structured error envelopes cannot reach the DOM via evidence messages
- Managed host detection covers endpoint table URLs in addition to settings_config

## [0.1.11] — 2026-07-26

### Fixed

- Quick validation is now a true low-impact path: at most one current-configuration generate request per Provider
- Removed the product-specific `CCS_DOCTOR_OK` marker from normal Generate / Stream requests
- Native protocol 2xx responses with non-empty valid text count as `GENERATE_OK`
- Quick no longer auto-retries Gemini Query-key or OpenAI `max_tokens` field fallbacks
- Quick does not send CCS route business requests (status/disposition only)

### Changed

- Default diagnosis mode is Quick (concurrency fixed at 1)
- Smart / Deep UI copy discloses multi-request automated diagnostic impact
- Safety Drawer states that automation may still be recognized and that Doctor never spoofs official clients

### Security

- Kept transparent `CC-Switch-Doctor/<version>` User-Agent
- No official-client spoofing, UA rotation, prompt randomization, jitter, or proxy/IP rotation
- SQLite remains read-only; full keys still never enter the DOM

### Added

- Regression tests for Quick request count, non-empty native text success, UI risk notices, and non-evasion boundaries
- Spec: `docs/project/CC-Switch-Doctor-v0.1.11-Low-Impact-Compliant-Diagnosis-Spec.md`

## [0.1.10] — 2026-07-26

### Fixed

- Removed the redundant Provider “查看详情” action that duplicated card activation
- Provider content navigation is enabled only when a diagnosis result exists
- ResultCard no longer repeats the Primary conclusion under a separate “诊断结论” block
- Simple Direct / Route states collapse into a single channel summary line
- Copy buttons, accordion summaries, and selects no longer trigger cross-pane provider jumps
- Clicking a Provider whose result is hidden by the right-side filter reveals and focuses it

### Changed

- Compacted Provider rows to a three-line metadata layout (name/status, app·key·protocol, host/model)
- Reduced Provider card min-height / padding for higher list density without shrinking global fonts
- Confidence badge sits with Primary status in the ResultCard header meta row
- Model semantics and success combinations use single-line compact copy when unambiguous

### Added

- UI density and interaction regression tests for Provider navigation, ResultCard de-duplication, and event filtering
- Spec: `docs/project/CC-Switch-Doctor-v0.1.10-Compact-Provider-and-Result-UI-Regression-Safe-Spec.md`

### Security

- No Rust/backend changes; SQLite remains read-only; full keys still never enter the DOM
- Frontend-only UI density and interaction fixes

## [0.1.9] — 2026-07-25

### Fixed

- **P0:** Claude/CC Switch `[1M]` local context marker is stripped before upstream requests and treated as current-config success (`CURRENT_CONFIG_OK`), not `MODEL_VARIANT_OK`
- Structured 5xx bodies with `error.code=model_not_found` / “No available channel for model” classify as `MODEL_NOT_FOUND` before generic 5xx fallback
- Configured Provider role mappings succeed as `CONFIGURED_MODEL_MAPPING_OK` (not “更换模型后可用”)
- Doctor-guessed models remain `MODEL_GUESS_OK` with `current_config_ok=false`
- Final success evidence ranked by semantic quality (current / local-marker / role mapping before first-in-time success)
- Provider card `safe_base_url` registers the real API key into `SecretRedactor` (path / query / non-sk keys)

### Added

- `ModelCandidate` + `ModelCandidateSource` (display / wire / source / equivalent_to_current)
- Attempt + summary fields: `configuredModelDisplay`, `outboundModel`, `modelTransform`
- Result Card “模型语义” section (配置值 / 上游值 / 规则)
- Research gate: `docs/research/v0.1.9-model-semantics-review.md` (upstream `878c26f3…`)
- Fixtures under `tests/fixtures/models/`

### Security

- SQLite remains read-only; no CLI spawn; no login-directory reads
- Full keys still never enter frontend / logs / cache keys
- Cross-host redirects still blocked; CCS route still loopback-only

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
