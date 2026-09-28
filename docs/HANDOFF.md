# Handoff / progress log

This file is the durable record for anyone (human or another Claude session) continuing the
project. Keep it current: what is done, what is verified, what is blocked, and the exact next
action for each blocker. Do not repeat discovery that is already recorded here.

## Project facts

- Working name: **VM Setup Assistant** (binary `vm-setup-assistant`, bundle id `org.vm-setup-assistant.app`).
- Repository: https://github.com/BrooksMack/The-Bypass-Of-The-Beasts (development branch `claude/vm-setup-assistant-app-cu0hj9`, default branch `main`). The repository name predates the project; renaming it is an open question for the owner (see Blockers).
- Stack: Tauri 2 (Rust backend, `src-tauri/`) + React/TypeScript/Vite frontend (`src/`) + a plain Rust core library (`crates/vmsa-core/`) that holds all logic and is unit-tested without a GUI.
- Third-party facts verified on 2026-09-28 (sources in `docs/ARCHITECTURE.md` and code comments):
  - VirtualBox current stable: **7.2.20 build 175154** (released 2026-09-22). Package names: `VirtualBox-7.2.20-175154-Win.exe`, `-macOSArm64.dmg`, `-OSX.dmg`. Oracle publishes `LATEST-STABLE.TXT` and per-version `SHA256SUMS`.
  - Arm hosts (Apple Silicon, Windows/Arm) run **only Arm guests**; Windows on Arm hosts is "experimental" per Oracle; Arm hosts have no unattended install; all Arm VMs are EFI.
  - OS type ids: `Windows11_64` (x64) and `Windows11_arm64`. Oracle defaults for the Arm type: VBoxSVGA graphics, **UsbNet** (USB NCM) network adapter, SATA/AHCI storage, `armv8virtual` chipset, EFI + Secure Boot + TPM hints. x64 type: VBoxSVGA, Intel 82540EM NIC, SATA/AHCI, PIIX3, EFI + Secure Boot + TPM 2.0.
  - Supported macOS host versions listed by Oracle for 7.2: 13 Ventura, 14 Sonoma, 15 Sequoia (Intel and Apple Silicon).
  - GitHub-hosted runner labels (from actions/runner-images README): `windows-2025`/`windows-latest` (x64), `macos-15` (arm64), `macos-15-intel` (x64), `macos-26` (arm64), `macos-26-intel`.
  - Tauri: `tauri` 2.12, `tauri-build` 2.7, `@tauri-apps/cli` 2.12, `tauri-apps/tauri-action@v1`.

## Environment notes for cloud sessions

- The cloud container blocks `virtualbox.org`, `download.virtualbox.org`, `docs.oracle.com`, `v2.tauri.app`, `docs.github.com`, Wikipedia. Reachable: GitHub (API for this repo, raw.githubusercontent.com for any public repo, anonymous git clone of public repos), crates.io, npm, apt, oracle.com, microsoft.com.
- VirtualBox source (incl. the user manual in DITA and VBoxManage man pages) was read from a sparse clone of `github.com/VirtualBox/virtualbox` at `/home/user/virtualbox/virtualbox` (not part of this repo).
- Local rustc in the container is 1.94; `sysinfo` is pinned to 0.38 for that reason.
- No VirtualBox and no nested virtualization in the container: **no real VM smoke test can run here**. Real-hardware testing must be done by the owner (see `docs/MANUAL-TEST-CHECKLIST.md`).

## Status

| Milestone | State |
|---|---|
| 1. Core library (host checks, profiles, VBoxManage plans/parsers, downloads, ISO inspection, state machine, network diagnosis, redaction) | Done, 59 unit tests pass |
| 2. Tauri backend commands + setup orchestrator | Done (compiles on Linux; clippy clean) |
| 3. React guided flow + dashboard | Done (2 workflow tests on the labelled mock backend) |
| 4. CI (build + tests on Windows/macOS runners) | Written; first runs pending |
| 5. Release workflow (NSIS exe, Arm64 dmg, Intel dmg, checksums, SBOM) | Written; first tag pending |
| 6. Docs (README, compatibility matrix, troubleshooting, manual test checklist) | Done (first pass) |
| 7. Real hardware smoke test | Blocked: needs owner's machine |

## Evidence so far

- 2026-09-28: `cargo test --workspace` 60 tests pass; `cargo clippy -D warnings` clean; `npm test` 2 workflow tests (mock backend) pass; `npm run typecheck` clean.
- 2026-09-28: the Linux debug build launched under Xvfb, rendered the Welcome and Check screens, ran real host inspection and correctly blocked (unsupported host, no virtualization in the container). Screenshots in `docs/screenshots/`.

## Promotion note

The owner mentioned a promotion (about $250 in Claude credits tied to publishing work on GitHub). No link or
terms were provided in this session, so nothing was verified. When the link is available: read the actual
requirements, record the deadline and submission steps here, and do not assume that a public repository alone
qualifies.

## Blockers and next actions

| Blocker | Next action |
|---|---|
| Repository name `The-Bypass-Of-The-Beasts` does not describe the product | Owner decides: rename this repo (Settings > General > Repository name) or create `vm-setup-assistant` and push this branch there. Code and docs use the current URL until then. |
| Code signing / notarization credentials | Owner supplies GitHub Actions secrets listed in `docs/RELEASING.md`. Until then, releases are unsigned "preview" builds and the docs say so. |
| Real VM smoke test | Owner runs `docs/MANUAL-TEST-CHECKLIST.md` on the Apple Silicon Mac (and a Windows x64 PC if available) and records results in `docs/COMPATIBILITY.md`. |
