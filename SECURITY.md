# Security Policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | Yes       |

## Reporting a vulnerability

Please **do not** open a public issue for key-leak or RCE class bugs.

Email or private GitHub security advisory to the repository maintainers (`Super-YYQ/cc-switch-doctor`).

Include:

- Doctor version
- OS version
- Steps to reproduce
- Impact (e.g. key exfiltration, DB write, process spawn)

## Hard guarantees (v0.1)

- No process spawn / AI CLI launch in production code (CI gate).
- No protected login path reads (CI gate).
- CC Switch DB read-only.
- Full API keys never cross the Tauri IPC boundary.
- Same-origin only for automatic variants; cross-host redirects blocked without credentials.
- Managed OAuth / Copilot / ChatGPT backend configs cannot be force-tested.

## Please do not

- Paste logs that may contain API keys into public issues.
- Ask for a “bypass managed auth” feature.
- Disable TLS verification.

## Disclosure

We aim to acknowledge reports within 7 days and ship fixes as soon as practical for confirmed issues.
