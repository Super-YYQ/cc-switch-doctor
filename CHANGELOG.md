# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] — 2026-07-20

### Added

- Initial Windows x64 release of CC Switch Doctor.
- Read-only CC Switch database discovery and schema fingerprinting (verified against v3.17.0 / schema 15).
- Provider listing with app filters; managed OAuth/Copilot hard-skip.
- Quick / Smart / Deep diagnosis modes.
- Protocols: OpenAI Chat Completions, OpenAI Responses, Anthropic Messages, Gemini Native.
- Streaming SSE and Tool Calling probes (deep mode).
- Same-origin URL variants, `/v1` normalize, cross-host redirect block.
- Security CI gates (no process spawn, no protected paths, version sync).
- GitHub Actions: CI, Release, daily upstream-watch.
- Portable ZIP + NSIS setup + SHA256SUMS release assets.
