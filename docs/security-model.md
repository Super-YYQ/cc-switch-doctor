# Security Model

## Trust boundaries

1. **WebView UI** — untrusted for secrets. Only masked provider rows and redacted events.
2. **Rust core** — holds secrets in `secrecy::SecretString`.
3. **CC Switch DB** — external; opened read-only.
4. **Upstream HTTP** — user-selected hosts only; automatic variants cannot leave origin.

## Controls

| Control                    | Implementation                                             |
| -------------------------- | ---------------------------------------------------------- |
| No process spawn           | Code + `scripts/verify-no-process-spawn.mjs`               |
| No protected paths         | Code + `scripts/verify-no-protected-paths.mjs`             |
| DB readonly                | URI `mode=ro`, `PRAGMA query_only=ON`                      |
| Redirect safety            | `reqwest` redirect Policy::none + manual same-origin check |
| Key redaction              | `SecretRedactor` on errors/events                          |
| Managed auth               | Hard skip, no UI bypass                                    |
| Capability least privilege | `capabilities/default.json` without shell/fs write         |

## Threat notes

- Malicious provider URL in DB: requests only go to that host; still user must select and start test.
- Compromised UI: cannot read full keys from IPC payloads by design.
- Schema change: Unknown status stops testing rather than guessing columns.
