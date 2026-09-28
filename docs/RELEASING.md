# Releasing

## How a release is produced

1. Update `CHANGELOG.md` with a `## [X.Y.Z]` section, and set the same version in `package.json` and the
   `[workspace.package]` table of `Cargo.toml`. `npm run check:version` verifies they agree.
2. Push a tag `vX.Y.Z` (or `vX.Y.Z-preview.N` for a pre-release). The `Release` workflow:
   - verifies the version and runs the tests,
   - builds on `windows-2025` (NSIS `.exe`, x64), `macos-15` (`.dmg`, Apple Silicon) and `macos-15-intel`
     (`.dmg`, Intel),
   - signs and notarizes when the secrets below exist, otherwise labels the build as an unsigned preview,
   - renames artifacts to `VM-Setup-Assistant-<version>-<os>-<arch>…`, writes `…-SHA256SUMS.txt`,
     attaches an SPDX SBOM and publishes the GitHub release with notes from the changelog.

Pull-request CI (`ci.yml`) never sees the signing secrets; only tag pushes run `release.yml`.

## Signing credentials (GitHub Actions secrets)

### macOS (Developer ID)

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | Base64 of the Developer ID Application `.p12` (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | Password of that `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Apple ID used for notarization |
| `APPLE_PASSWORD` | App-specific password for that Apple ID |
| `APPLE_TEAM_ID` | 10-character team id |

Requires a paid Apple Developer Program membership. Without these, the workflow ad-hoc signs the app
(`APPLE_SIGNING_IDENTITY=-`) so it can run at all on Apple Silicon, and users must right-click › Open once.

### Windows (Authenticode)

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | Base64 of a code-signing `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | Its password |

The workflow imports the certificate into the runner's user store and passes its thumbprint to the Tauri
bundler (`bundle.windows.certificateThumbprint`, SHA-256 digest, DigiCert timestamp). Without it, SmartScreen
shows "Windows protected your PC" until the publisher builds reputation. An EV certificate or Azure Trusted
Signing removes that warning immediately; for Azure Trusted Signing set `bundle.windows.signCommand` instead.

Never commit certificates or passwords. They live only in repository secrets.

## Manual verification before calling a release stable

Run `docs/MANUAL-TEST-CHECKLIST.md` on each claimed platform and record the results in
`docs/COMPATIBILITY.md`. Until then, releases are marked as pre-releases with "preview" in the version.
