# VM identity configuration (compatibility testing)

This is an **optional, off-by-default** feature for testers whose software behaves differently, or
refuses to run, inside a virtual machine. It lets you change the hardware identifiers the guest reads
so a VM can be exercised as if it were a specific physical machine. It is intended for compatibility
testing and security research on VMs you own.

> **It does not make a VM undetectable.** Many signals remain (see
> [What cannot be hidden](#what-cannot-be-hidden-reliably)). The app never claims otherwise, always
> shows what a configuration cannot hide, and every change is reversible.

## Hypervisor and where this lives

The app drives **Oracle VirtualBox** through its supported command-line tool, `VBoxManage`. No GUI
automation and no shell interpolation: every call is an argument array. The identity feature uses only
documented, first-party VBoxManage mechanisms.

| Concern | File |
|---|---|
| Config types, validation, command generation, reversal, preview | `crates/vmsa-core/src/identity.rs` |
| Base VM creation the identity builds on | `crates/vmsa-core/src/vbox/plan.rs`, `crates/vmsa-core/src/profile.rs` |
| Orchestration (apply at creation, apply/revert later, capture originals) | `src-tauri/src/setup.rs` |
| Tauri commands exposed to the UI | `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` |
| Persisted configuration and captured originals | `crates/vmsa-core/src/state.rs` (`SetupChoices.identity`, `VmRecord.identity_*`) |
| UI panel | `src/screens/Identity.tsx` (Dashboard → "VM identity" tab) |
| Types / backend bridge / mock | `src/lib/types.ts`, `src/lib/api.ts`, `src/lib/mock.ts` |

## What is technically possible

All of the following are set through documented VBoxManage settings and are verified by the guest with
standard Windows tools (see [the checklist](IDENTITY-CHECKLIST.md)).

### 1. Firmware, system, mainboard and chassis (SMBIOS / DMI)

Set through `VBoxManage setextradata <vm> "VBoxInternal/Devices/pcbios/0/Config/Dmi*"`
(VirtualBox manual, *Configuring the BIOS DMI Information*).

- **Firmware (SMBIOS type 0):** BIOS vendor, version, release date.
- **System (type 1):** manufacturer, product, version, serial, SKU, family, UUID.
- **Mainboard (type 2):** manufacturer, product, serial.
- **Chassis (type 3):** manufacturer, asset tag.

The system UUID is also applied to `modifyvm --hardwareuuid` so the SMBIOS UUID and the VM's hardware
UUID agree.

### 2. Storage identity (ATA IDENTIFY)

Set through `VBoxManage setextradata <vm> "VBoxInternal/Devices/<controller>/0/Config/Port0/…"`, where
`<controller>` is the emulated device (`ahci` for the SATA controller this app uses) and `Port0` is
the disk:

- `SerialNumber` (≤ 20 chars), `ModelNumber` (≤ 40 chars), `FirmwareRevision` (≤ 8 chars).

### 3. Network adapter identity and mode

Set through `modifyvm`:

- **MAC address** (`--macaddress1`). By default VirtualBox uses the `08:00:27` OUI, which names the
  vendor; setting a MAC changes the prefix a fingerprint would read.
- **Adapter model** (`--nic-type1`): `82540EM`, `82543GC`, `82545EM`, `Am79C973`, `virtio`, `usbnet`.
- **Network mode** (`--nic1`): `nat` (default), `natnetwork`, `bridged`, `hostonly`. See
  [network presentation](#network-presentation) for the connectivity trade-offs.

### 4. Branding reduction

- **Paravirtualization interface** (`--paravirtprovider`). `none` removes the hypervisor CPUID leaf
  (`0x40000000`, the `VBoxVBoxVBox` signature) and the hypervisor-present bit — the most direct
  "I am a VM" hint. **Trade-off:** it disables paravirtual time synchronisation and can slow the VM,
  so it is off by default.
- **VirtualBox SMBIOS OEM strings.** VirtualBox fills SMBIOS type 11 with `vboxVer_<version>` and
  `vboxRev_<revision>`, a well-known tell. The feature can overwrite these with a neutral value.

## Network presentation

Each mode is explained in the UI as you pick it. In short:

| Mode | Connectivity | Guest address | Notes |
|---|---|---|---|
| NAT (default) | Outbound internet; not reachable from the LAN | `10.0.2.x` | The safe default; nothing changes unless you pick another mode. |
| NAT network | Outbound internet; VMs on the named network reach each other | from the NAT network | Needs a named NAT network. |
| Bridged | Appears as a peer on the physical LAN | from LAN DHCP | Needs a working host adapter; some Wi-Fi/corporate networks block bridging. |
| Host-only | Reaches the host only; no outbound internet | host-only subnet | Use to isolate a test VM. |

Connectivity is preserved: the adapter's cable stays connected, and NAT (the profile default) is kept
unless you deliberately choose another mode.

## Validation

`identity::validate` rejects a configuration before anything is applied:

- MAC must be 12 hex digits, unicast (not multicast), and not all zero.
- System UUID must be a valid UUID.
- DMI strings must have no control characters and stay within a practical length.
- Storage strings respect the ATA field widths (20 / 40 / 8).
- Adapter model and paravirtualization provider must be from the supported lists.
- Bridged / host-only / NAT-network modes require the adapter or network name they need.

A disabled configuration is always valid and generates no commands.

## Reversibility

- The feature is **off by default**. With it off, the app behaves exactly as before and the VM keeps
  the profile defaults.
- Before applying, the app captures the values it is about to change (the managed extra-data keys and
  the `modifyvm` fields) into the VM record.
- **Revert** restores those captured values and deletes any extra-data key that had no prior value,
  returning the VM to its original identity. Apply and revert require the VM to be powered off.

## How to use it

1. Complete the normal setup at least through **Choose setup** (identity is stored on your setup
   choices).
2. Open the app's **VM identity** tab (on the everyday dashboard), turn the feature on, and fill in
   only the fields you need. The panel previews what each setting does, the network effect, and what
   it cannot hide.
3. **Save**. If the VM does not exist yet, the settings are applied when it is created. If it already
   exists, shut Windows down and choose **Apply to this VM now**.
4. Verify inside Windows using [the identity checklist](IDENTITY-CHECKLIST.md).
5. **Revert identity changes** at any time (VM powered off) to undo everything.

## What cannot be hidden reliably

This feature changes identifiers; it does not remove the machinery of virtualization. The following
remain visible and are surfaced in the UI:

- **Guest Additions**, if installed, expose `VBoxService.exe`, VirtualBox kernel drivers, and
  `\VirtualBox\GuestInfo` guest properties. Removing them defeats this detection but breaks clipboard,
  display resizing and network-status reporting, so the app does not remove them.
- **The emulated GPU and monitor** still report VirtualBox (VBoxSVGA / "Oracle VirtualBox"), visible
  in Device Manager and the display EDID.
- **PCI/USB device vendor and revision IDs** of the emulated chipset, audio and USB controllers are
  VirtualBox's and are not changed by these settings.
- **Timing and behavioural checks** (RDTSC deltas, hypervisor exits, CPUID leaf `0x40000000`) can
  still reveal virtualization; some persist even with the paravirtualization interface set to `none`.
- **ACPI tables and assorted registry/driver artifacts** still carry VirtualBox-specific values.

If you do not set a MAC, the adapter keeps VirtualBox's `08:00:27` prefix. If you do not set the
paravirtualization interface to `none`, the hypervisor is still advertised through CPUID. The app
points both of these out in the preview.

## Scope and intent

This is for compatibility testing and research on machines you control. It does not defeat any
specific anti-cheat, anti-fraud or DRM system, and — as the list above makes clear — it cannot make a
VM indistinguishable from physical hardware.
