# Compatibility and testing matrix

Each cell is one of: **Yes** (done and recorded, with date and version), **No** (attempted and failed),
**Untested**, or **N/A**. A build artifact existing is not evidence that setup works; only the last two
columns say anything about Windows actually running.

| Host | Guest | App builds | App launches | Host checks and dependency detection | VM creation and boot | Windows installation and guest functionality |
|---|---|---|---|---|---|---|
| Windows 10/11, Intel/AMD x64 | Windows 11 x64 | Yes (CI, see Releases) | Untested | Untested | Untested | Untested |
| macOS 13+, Apple Silicon | Windows 11 ARM64 | Yes (CI) | Untested | Untested | Untested | Untested |
| macOS 13+, Intel | Windows 11 x64 | Yes (CI) | Untested | Untested | Untested | Untested |
| Windows 11 on ARM | any | Yes (same x64 build, runs emulated) | Untested | App refuses with explanation (by design) | N/A | N/A |
| Linux | any | Yes (dev only, not released) | Yes (dev build under Xvfb, 2026-09-28) | Yes: real host facts shown and the app refuses with an "unsupported host" blocker (screenshot in README, 2026-09-28) | N/A | N/A |

Update this table from `docs/MANUAL-TEST-CHECKLIST.md` results. Record: date, app version, OS version,
VirtualBox version, Windows build, and what exactly was observed.

## Basis for the defaults (verified 2026-09-28 against the VirtualBox 7.2 source and user guide)

- Arm hosts (Apple Silicon, Windows/Arm) run only Arm guests. Windows on Arm hosts is described by Oracle
  as experimental and requires contacting Oracle; this app therefore does not support Windows/Arm hosts.
- Arm VMs always use EFI. Unattended installation is not available on Arm hosts. The Arm System settings
  page exposes no TPM control, so the app relies on `createvm --default` for the Arm OS type's defaults.
- Oracle's defaults for `Windows11_arm64`: VBoxSVGA graphics, UsbNet (USB NCM) network adapter, SATA/AHCI
  storage, ARMv8 virtual chipset, EFI + Secure Boot. For `Windows11_64`: VBoxSVGA, Intel 82540EM NIC,
  SATA/AHCI, PIIX3 chipset, EFI + Secure Boot + TPM 2.0.
- Supported macOS host versions listed by Oracle for VirtualBox 7.2: 13 Ventura, 14 Sonoma, 15 Sequoia
  (both Intel and Apple Silicon). Newer macOS versions may work but were not listed at the time of writing.
- Windows hosts: 64-bit Windows 10/11 (VirtualBox ships one unified installer for Intel/AMD and Arm).

## Historical observations from a manual setup (Apple Silicon, VirtualBox 7.2.16, Windows 11 Home 25H2 ARM64)

These informed the design but are **not** results for this app. They are turned into regression scenarios
in `docs/MANUAL-TEST-CHECKLIST.md`.

- Host: 24 GB RAM, 12 cores. VM: 8 GB RAM, 4 vCPUs, 100 GB dynamic disk. Guest Additions installed.
- NAT networking worked initially. After Windows updates, Microsoft's UsbNcm driver reported Code 10 and
  networking was lost; restarting and reinstalling the same driver did not help.
- Installing the Microsoft-signed ARM64 NetKVM driver from virtio-win 0.1.302 and switching the adapter to
  virtio-net restored connectivity (adapter Up, address assigned, Windows "Connected").
- Windows stayed unactivated (no license supplied).
- A "Display failure" notification appeared later and was not investigated. Display behaviour after
  resume is therefore treated as unverified.
