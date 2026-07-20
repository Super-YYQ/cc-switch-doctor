# Release Process

1. Ensure `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` versions match.
2. Pass local quality gate + security verify.
3. Push `main`.
4. Tag `vX.Y.Z` and push tag → `release.yml`.
5. Workflow builds on `windows-latest`, packages:
   - `CC-Switch-Doctor-vX.Y.Z-Windows-x64-setup.exe`
   - `CC-Switch-Doctor-vX.Y.Z-Windows-x64-portable.zip`
   - `SHA256SUMS.txt`
6. Creates non-draft GitHub Release with unsigned SmartScreen notice when no cert secrets.
7. Verify assets exist and size > 0 via GitHub API.

Optional signing secrets: `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`.
