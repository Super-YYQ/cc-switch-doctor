# Compatibility

## Check record

| Field                     | Value                                      |
| ------------------------- | ------------------------------------------ |
| Checked at                | 2026-07-25                                 |
| CC Switch latest verified | **v3.18.0**                                |
| Also verified             | v3.17.0 (`user_version=15`)                |
| Known compatible          | `user_version=13` (observed core shape)    |
| Upstream baseline commit  | `878c26f31e012ba32b9772bd080bd4fa9e7d495e` |
| SCHEMA_VERSION            | **16**                                     |
| Doctor version            | 0.1.9                                      |

Source: https://github.com/farion1231/cc-switch

## Architecture (v0.1.8)

```text
Exact version allowlist + upstream commit manifest
→ Verified label only (regression, release notes, upstream watch)

Runtime structural capability detection
→ Gates Provider Scan / Endpoint Scan / Direct Diagnosis /
  Routing Discovery / Routing Diagnosis
```

Rules:

- Unknown `user_version` ≠ structure incompatible
- New optional columns / unrelated tables → continue
- Missing required columns → disable only the affected capability
- Routing structure unknown → disable routing only
- Single Provider parse failure → skip that Provider only

## Schema fingerprint (verified v16 / v15)

- Tables: `providers`, `provider_endpoints`, `settings` (+ others ignored)
- `providers` required: id, app_type, name, settings_config, meta, is_current
- `provider_endpoints` required: provider_id, app_type, url
- v15 → v16 migration rebuilds Codex session usage only; Provider core unchanged

## Capability shapes

See `capabilityShapes` in [`../compatibility/manifest.json`](../compatibility/manifest.json).

## Supported app_type values

`claude`, `claude-desktop`, `codex`, `gemini`, `opencode`, `openclaw`, `hermes`, `grokbuild`

## Managed skip

- `meta.providerType = codex_oauth | github_copilot`
- Base URL contains `chatgpt.com/backend-api/codex` or `githubcopilot.com`
- Empty static API key

## Re-validation process

1. Read latest CC Switch release notes + `database/mod.rs` SCHEMA_VERSION
2. Diff `schema.rs` / provider credential paths
3. Update fixtures + `compatibility/manifest.json` (Verified entry only)
4. Confirm capability shapes still cover required core columns
5. Run cargo tests + security gates
6. Only then mark verified — unknown versions with compatible structure keep working without a Doctor release

Machine-readable: [`../compatibility/manifest.json`](../compatibility/manifest.json)
