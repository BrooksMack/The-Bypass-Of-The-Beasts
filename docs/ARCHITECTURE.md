# Architecture

```
┌──────────────────────────────┐   invoke/events (typed, allow-listed)   ┌───────────────────────────────┐
│  src/  React + TypeScript    │ ─────────────────────────────────────▶ │ src-tauri/  Tauri 2 (Rust)     │
│  guided screens, dashboard   │ ◀───────────────────────────────────── │ commands.rs: thin command layer│
│  lib/api.ts (+ labelled mock)│          setup-progress events          │ setup.rs: orchestration        │
└──────────────────────────────┘                                          └──────────────┬────────────────┘
                                                                                         │ uses
                                                                          ┌──────────────▼────────────────┐
                                                                          │ crates/vmsa-core (plain Rust)  │
                                                                          │ host · profile · vbox · download│
                                                                          │ verify · iso · installer · state│
                                                                          │ network · diagnostics · paths   │
                                                                          └──────────────┬────────────────┘
                                                                                         │ argument arrays only
                                                                          ┌──────────────▼────────────────┐
                                                                          │ VBoxManage (Oracle VirtualBox) │
                                                                          └───────────────────────────────┘
```

## Why this stack

Tauri 2 with a Rust backend and a React/TypeScript frontend, as preferred. Rust gives typed process
execution and reliable file handling; Tauri produces the NSIS `.exe` and `.dmg` installers and is small.
Everything that can break an installation lives in `crates/vmsa-core`, a normal library with unit tests
that run on Linux CI without VirtualBox.

## Responsibilities

| Area | Where | Notes |
|---|---|---|
| Host inspection | `vmsa-core::host` | `sysinfo` plus platform calls: `sysctl` on macOS (`hw.optional.arm64`, `sysctl.proc_translated`, `kern.hv_support`), `IsWow64Process2` and one PowerShell CIM query on Windows. Reports Unknown rather than guessing. |
| Compatibility profiles | `vmsa-core::profile` | Architecture- and version-specific VM profiles, conservative resource sizing, blockers vs recommendations, storage estimates, VirtualBox version parsing. |
| Download management | `vmsa-core::download` | reqwest + rustls, `.part` files with ETag/Last-Modified sidecar, Range resume, cancellation, SHA-256 verification. VirtualBox release resolution via `LATEST-STABLE.TXT` + `SHA256SUMS`, pinned fallback labelled as such. |
| Verification | `vmsa-core::verify` | Streaming SHA-256, `SHA256SUMS` parsing. |
| ISO validation | `vmsa-core::iso` | Minimal ISO-9660/Joliet reader: volume label, `EFI/BOOT/BOOTX64.EFI` vs `BOOTAA64.EFI`, `sources/install.wim|esd`. |
| Dependency install | `vmsa-core::installer` | Windows: runs the official `.exe` (self-elevating via UAC), interprets MSI exit codes (0, 3010, 1602, 1618…). macOS: `hdiutil attach -plist`, `open -W VirtualBox.pkg`, detach; detection decides success. |
| VirtualBox integration | `vmsa-core::vbox` | `plan` builds argument arrays (pure, tested); `parse` handles `--machinereadable`, `list vms`, `guestproperty`, `createvm`, `hostinfo`; `client` runs them with timeouts, cancellation and `LANG=C`. |
| VM ownership | extra data `VMSetupAssistant/InstanceId` + state file | A matching name never implies ownership. Only VMs carrying the marker of this setup are adopted or deleted. |
| Setup state | `vmsa-core::state` | JSON, atomic writes, schema version, corrupt-file backup, `resume_stage()` derived from durable facts, bounded history, lock file. |
| Guest health | `setup::derive_guest_status` | Only observable facts (VM state, guest properties) are auto-verified; internet, updates and activation need user confirmation. |
| Diagnostics | `vmsa-core::diagnostics` | Regex redaction of secrets, keys, e-mails, user folders, MACs, public IPs; report preview before save; never uploaded. |
| UI | `src/` | Stage router with sidebar, "What to do next" panels, accessible contrast and focus styles, mock backend banner. |
| Packaging | `.github/workflows/release.yml` | Version consistency, tests, per-platform builds, optional signing, renamed artifacts, checksums, SBOM, release notes from CHANGELOG. |

## Security boundaries

- The web view can only call the commands listed in `src-tauri/src/lib.rs`; there is no shell, fs or
  http plugin exposed to it. External pages are opened only from a fixed allow-list (`open_official_page`).
- All process invocations pass argument arrays. User-provided names, paths and URLs are never interpolated
  into a shell string.
- Downloaded metadata (`LATEST-STABLE.TXT`, `SHA256SUMS`) is parsed strictly; no remote script is executed.
- The app runs unprivileged. Elevation happens only inside Oracle's own installer (UAC / macOS Installer).
- Support reports are redacted on creation and again on save.

## Setup state machine

Stages: Welcome → CheckComputer → ChooseSetup → ObtainFiles → InstallDependencies → CreateVm →
InstallWindows → FinishAndVerify → Dashboard. The saved stage only moves forward; going back to review does
not rewind it. On reopen the app resumes from `resume_stage()`, which is computed from facts: a recorded VM
means never re-running creation; a verified installer download is never re-downloaded; a partially created
VM continues from its recorded completed steps.

Single-instance protection: Tauri's single-instance plugin focuses the existing window; an in-process
operation guard (`AppState::begin`) refuses a second long operation with `Busy`; a pid lock file guards
against a stale second process.

## Data locations

- State, downloads and logs: the per-user local data directory (`%LOCALAPPDATA%` on Windows,
  `~/Library/Application Support` on macOS) under `org.vm-setup-assistant/VM Setup Assistant`.
- VM files: VirtualBox's own default machine folder (`~/VirtualBox VMs`) unless the user chooses otherwise.
  The app warns about cloud-synced locations.
