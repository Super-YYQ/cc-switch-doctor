# Testing Strategy

## Rust

- Unit: path discovery, fingerprint, normalize, managed auth, URL variants, origin, classifier, planner
- Integration: fixture DB scan immutability (SHA-256 before/after), readonly write fail
- HTTP: wiremock/httpmock optional for protocol success paths (unit extractors covered without network)

## Frontend

- Vitest + Testing Library: filters, safety banner, no auto-start, no full key in DOM, mode default

## Security regression

```bash
npm run security:verify
```

## Quality gate

See AGENTS.md for the full command list required before release.
