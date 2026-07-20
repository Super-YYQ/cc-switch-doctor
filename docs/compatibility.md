# Compatibility

## Check record

| Field                        | Value                                      |
| ---------------------------- | ------------------------------------------ |
| Checked at                   | 2026-07-20                                 |
| CC Switch latest release     | **v3.17.0** (published 2026-07-13)         |
| Baseline commit (tag target) | `3d176b98cc0bfd151a42882e88ab59b62083b92f` |
| main HEAD at check           | `613fef70bc7d5e35299b4131935f738c85765b35` |
| SCHEMA_VERSION               | **15**                                     |
| Doctor version               | 0.1.1                                      |

Source: https://github.com/farion1231/cc-switch

## Schema fingerprint (verified)

- Tables: `providers`, `provider_endpoints`, `settings` (+ others ignored)
- `providers` columns include: id, app_type, name, settings_config, meta, is_current, …
- Credential extraction mirrors CC Switch `resolve_usage_credentials` shapes per app_type

## Supported app_type values

`claude`, `claude-desktop`, `codex`, `gemini`, `opencode`, `openclaw`, `hermes`, `grokbuild`

## Managed skip

- `meta.providerType = codex_oauth | github_copilot`
- Base URL contains `chatgpt.com/backend-api/codex` or `githubcopilot.com`
- Empty static API key

## Re-validation process

1. Read latest CC Switch release notes + `database/mod.rs` SCHEMA_VERSION
2. Diff `schema.rs` / provider credential paths
3. Update fixtures + `compatibility/manifest.json`
4. Run cargo tests + security gates
5. Only then mark verified

Machine-readable: [`../compatibility/manifest.json`](../compatibility/manifest.json)
