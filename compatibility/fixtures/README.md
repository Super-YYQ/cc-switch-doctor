# Compatibility fixtures

Sanitized SQLite fixtures used by Rust tests. They contain **no real API keys**.

| File                 | Purpose                                                                    |
| -------------------- | -------------------------------------------------------------------------- |
| `sanitized-v317.sql` | Minimal CC Switch schema v15 + sample third-party + managed-auth providers |

## Rules

- Never commit real credentials.
- Keys in fixtures must be fake placeholders such as `sk-test-fake-key-for-unit-tests-only`.
- Fixtures are loaded into temporary in-memory or tempfile databases opened with `mode=ro` after seed.
