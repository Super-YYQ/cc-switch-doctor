# Protocol Fixture Corpus

Sanitized, key-free fixtures for source-grounded parser tests.

Rules:

- No real API keys, tokens, or private URLs.
- Native fixtures must classify as Native for their target protocol.
- Cross-protocol fixtures must not classify as Native for the wrong target.
- Error envelopes must not become GENERATE_OK / native success text.
- New parser branches require a new fixture.

See `docs/research/v0.1.7-source-review.md` and `docs/project/CC-Switch-Doctor-v0.1.7-*.md`.
