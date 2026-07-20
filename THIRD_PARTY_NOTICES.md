# Third-Party Notices

## CC Switch

This project independently implements a **read-only** adapter for the public SQLite schema and provider configuration shapes of [CC Switch](https://github.com/farion1231/cc-switch) (MIT License).

We do **not** copy substantial portions of CC Switch source code. Schema knowledge was derived from the official repository files such as:

- `src-tauri/src/database/schema.rs`
- `src-tauri/src/database/dao/providers.rs`
- `src-tauri/src/provider.rs`
- `src-tauri/src/app_config.rs`
- `src-tauri/src/codex_config.rs`
- `src-tauri/src/config.rs`

CC Switch is © its respective authors, MIT licensed.

## Other dependencies

Rust crates and npm packages are pulled via Cargo/npm and retain their own licenses (primarily MIT/Apache-2.0). See `src-tauri/Cargo.lock` and `package-lock.json` after install.
