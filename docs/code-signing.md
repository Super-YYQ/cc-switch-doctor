# Code signing (future)

CC Switch Doctor **v0.1.2 ships unsigned**.

There is intentionally no optional signing step in the Release workflow.
Previous attempts only decoded a Base64 PFX into a temp file and never:

- imported the certificate
- obtained a thumbprint
- invoked `signtool`
- configured Tauri signing
- verified the resulting Authenticode signature

## Current policy

- Release Notes must say: **This release is unsigned.**
- README documents SmartScreen behavior for unsigned builds.
- SHA256SUMS.txt remains mandatory for integrity checking.
- Do not claim “auto-signs when secrets are set” until a complete, tested pipeline exists.

## Future plan (not implemented)

When a real Authenticode certificate is available:

1. Store PFX (or cloud HSM credentials) in repository secrets.
2. Import cert on the Windows runner and export thumbprint.
3. Pass thumbprint / sign command into Tauri bundle config.
4. Run `signtool verify /pa` on setup + portable EXE.
5. Only then mention signing in Release Notes.

Until then, keep the release path unsigned and honest.
