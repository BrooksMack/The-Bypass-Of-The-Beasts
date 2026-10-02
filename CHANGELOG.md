# Changelog

All notable changes to VM Setup Assistant are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.1-preview.1] - 2026-10-02

### Fixed

- Keep interrupted VM configuration retryable, and resume configuration before Windows Setup.
- Reject malformed ISO directory records without crashing.
- Preserve required host restarts until a different boot is observed and VirtualBox is detected.
- Replace the nonfunctional "Open the Windows window" action with accurate window-switching guidance.

### Added

- Camera discovery, attach/detach controls with a 30 FPS limit, microphone enable/disable controls,
  and macOS/Windows permission and recording-test guidance.
- Neutral default VM name (Desktop); explicit disabled clipboard and drag-and-drop sharing for new VMs.
- Apple Silicon GitHub preview workflow with Rust and frontend tests and a downloadable DMG/checksums.
- Regression coverage for partial setup, restart gating, malformed ISO input and media actions.

### Limitations

- Virtual hardware remains identifiable. Guest Additions and supported ARM devices are retained for usability.
- Camera, microphone, host permissions and complete Windows installation still require a physical Mac test.
- Apple Silicon preview artifacts are ad-hoc signed, not notarized.

## [0.1.0] - 2026-09-28 (preview)

First preview. Nothing in this release has been verified end-to-end on real hardware by the project;
the compatibility matrix in `docs/COMPATIBILITY.md` records what has actually been tested.

### Added

- Guided setup flow: Welcome, Check this computer, Choose setup, Get required files, Install VirtualBox,
  Create the VM, Install Windows, Finish and verify, Everyday dashboard.
- Host checks: OS and version, real processor architecture (including translated execution), memory,
  logical CPUs, free disk space at the chosen location, hardware virtualization, Hyper-V / Core isolation
  presence on Windows, existing VirtualBox and VMs.
- Architecture-specific VM profiles for Windows 11 x64 and Windows 11 ARM64 based on VirtualBox 7.2 defaults
  (UEFI + Secure Boot, TPM 2.0 on x64, VBoxSVGA graphics, NAT networking, SATA storage).
- Verified VirtualBox download from Oracle with resume support; Oracle's SHA256SUMS are used when reachable,
  otherwise a pinned checksum for VirtualBox 7.2.20 (labelled as such).
- Windows ISO handoff to Microsoft's official page and ISO validation from the disc's metadata.
- Official installer handoff on Windows (UAC) and macOS (Installer.app), with reboot-required handling.
- Persistent, resumable setup state; partially created VMs are completed rather than re-created; VMs the app
  did not create are never modified or deleted.
- Everyday controls: start/resume, normal shutdown, save state, force power off (separately confirmed),
  open VM folder, eject installation disc.
- Network troubleshooting with diagnosis-driven repairs, including the ARM "UsbNcm Code 10" recovery path
  (virtio-win NetKVM driver + adapter switch, with the original setting recorded for restore).
- Redacted support report with preview before saving; nothing is uploaded.
- CI on Windows, macOS (Apple Silicon and Intel) and Linux; release workflow producing an NSIS `.exe`,
  Apple Silicon `.dmg`, Intel `.dmg`, SHA-256 checksums and an SPDX SBOM; signing/notarization when secrets exist.

### Known limitations

- Preview installers are unsigned unless signing secrets are configured.
- Windows installation is guided, not unattended.
- No real-hardware smoke test has been recorded yet.
