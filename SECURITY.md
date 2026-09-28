# Security policy

## Reporting a vulnerability

Please do not open a public issue for security problems. Use GitHub's private vulnerability reporting
("Report a vulnerability" under the Security tab of the repository). If that is unavailable, open an issue
titled "Security contact request" without details and a maintainer will provide a private channel.

Please include the app version, operating system, and steps to reproduce. You should receive an
acknowledgement within 7 days.

## Scope and design

- The app runs without privileges; elevation happens only inside Oracle's official installer.
- The web view has no shell, file-system or network plugin access; it can only call the commands listed in
  `src-tauri/src/lib.rs`.
- Downloads are verified against SHA-256 checksums published by Oracle (or a pinned value that is labelled
  as such). Windows media is obtained by the user from Microsoft; the app validates its metadata.
- No telemetry, accounts or backend. Support reports are redacted, previewed, and saved locally only.
- Signing keys and tokens are never stored in the repository; see `docs/RELEASING.md`.
