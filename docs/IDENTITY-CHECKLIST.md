# Identity checklist: what Windows actually sees

Use this after applying a [VM identity configuration](VM-IDENTITY.md) to confirm what the guest
reports, and to see what still identifies the VM. Run the commands **inside the Windows guest**. Most
need only a normal PowerShell or Command Prompt window; a few read raw SMBIOS and are noted.

The goal is honest verification, not a pass/fail score. Expect the "still reveals a VM" section to keep
finding things — that is the point.

## Before you start

- Apply the configuration with the VM **powered off**, then start Windows.
- SMBIOS/DMI values are read by Windows at boot; if you change them, restart the guest before checking.

## 1. Firmware / BIOS (SMBIOS type 0)

```powershell
Get-CimInstance Win32_BIOS | Format-List Manufacturer, SMBIOSBIOSVersion, ReleaseDate
```
- Confirm **Manufacturer**, **SMBIOSBIOSVersion** and **ReleaseDate** match what you set.
- `msinfo32` → *BIOS Version/Date* shows the same.

## 2. System, mainboard and chassis (SMBIOS types 1–3)

```powershell
Get-CimInstance Win32_ComputerSystem        | Format-List Manufacturer, Model
Get-CimInstance Win32_ComputerSystemProduct  | Format-List Vendor, Name, Version, IdentifyingNumber, UUID, SKUNumber
Get-CimInstance Win32_BaseBoard              | Format-List Manufacturer, Product, SerialNumber
Get-CimInstance Win32_SystemEnclosure        | Format-List Manufacturer, SMBIOSAssetTag
```
- **Manufacturer/Model** should be your system values (e.g. `Dell Inc.` / `OptiPlex 7090`), not
  `innotek GmbH` / `VirtualBox`.
- **UUID** should equal the system UUID you set.
- `wmic csproduct get UUID,Name,Vendor` is a quick cross-check.

## 3. Storage (ATA IDENTIFY)

```powershell
Get-CimInstance Win32_DiskDrive | Format-List Model, SerialNumber, FirmwareRevision
Get-PhysicalDisk | Format-List FriendlyName, SerialNumber
```
- **Model / SerialNumber / FirmwareRevision** should match what you set (e.g. a Samsung model string),
  not `VBOX HARDDISK`.

## 4. Network adapter

```powershell
Get-NetAdapter | Format-List Name, InterfaceDescription, MacAddress
getmac /v
ipconfig /all
```
- **MacAddress**: confirm your prefix, not `08-00-27-*` (VirtualBox's OUI).
- **InterfaceDescription** reflects the adapter model you chose (e.g. an Intel PRO/1000 variant).
- Confirm the **address range** matches the network mode you chose (NAT → `10.0.2.x`, bridged → your
  LAN, host-only → the host-only subnet) and that a web page loads if the mode is meant to have
  internet.

## 5. Paravirtualization / hypervisor hint

```powershell
Get-CimInstance Win32_ComputerSystem | Select-Object HypervisorPresent
systeminfo | findstr /i "Hyper-V"
```
- With the paravirtualization interface set to **none**, `HypervisorPresent` should read `False` and
  CPUID leaf `0x40000000` should no longer return a hypervisor signature. A tool such as
  Sysinternals **Coreinfo** (`coreinfo -v`) shows the hypervisor bit directly.
- With any other setting, the hypervisor is still advertised — this is expected.

## 6. SMBIOS OEM strings (type 11)

```powershell
Get-CimInstance -Namespace root/cimv2 -ClassName Win32_ComputerSystemProduct
```
- To read type 11 directly, dump SMBIOS (e.g. Sysinternals or a firmware tool) and confirm the
  `vboxVer_*` / `vboxRev_*` strings are gone if you enabled OEM-string neutralisation.

## 7. What still reveals a VM (expect hits here)

These are not fixed by the identity feature; check them so you know what remains.

- **Guest Additions:** `Get-Service VBoxService`, `Get-Process VBoxService`, and drivers/devices whose
  name or vendor contains `VBox` / `VirtualBox` in Device Manager. Present unless you remove Guest
  Additions (which breaks integration features).
- **Display:** `Get-CimInstance Win32_VideoController | Select Name` typically shows `VirtualBox
  Graphics Adapter (VBoxSVGA)`; the monitor EDID names Oracle/VirtualBox.
- **Devices:** in Device Manager, check *Details → Hardware Ids* for controllers, audio and USB — the
  PCI/USB vendor IDs are VirtualBox's.
- **Registry:** search `HKLM\HARDWARE\DESCRIPTION\System` and `HKLM\SYSTEM\CurrentControlSet\Services`
  for `VBox` / `Oracle` / `VirtualBox`.
- **Timing:** RDTSC-based and hypervisor-exit timing checks can still detect virtualization regardless
  of these settings.

## Recording results

For a support report or a test log, capture the section 1–6 command output and note which section 7
items still identify the VM. The app's redacted **Support report** (dashboard → Support report) already
includes the VM's `showvminfo`, which lists the applied identity settings.
