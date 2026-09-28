# Troubleshooting

Plain-language answers first, technical detail after. The app's Troubleshooting tab performs the checks
that can be automated; this page covers the rest.

## Before you start

**"This computer type is not supported"**: the app supports Windows PCs with Intel/AMD processors and Macs.
Windows on ARM PCs (Snapdragon) are not supported because Oracle calls VirtualBox on those experimental.

**"Hardware virtualization is turned off"**: Intel VT-x / AMD-V is disabled in your PC's firmware. Restart,
enter the BIOS/UEFI setup (often F2, Del or F10 at power-on), enable "Intel Virtualization Technology" /
"SVM Mode", save, and run the check again. The app never changes firmware settings for you.

**"Another virtualization feature is active"** (Windows): Hyper-V, WSL 2, Windows Sandbox, Virtual Machine
Platform, or Core isolation / Memory integrity is on. VirtualBox 7 still works but Windows in the VM runs
slower. Turning these off trades away security features (Memory integrity) or other tools (WSL 2). The app
does not change them; if you decide to, use Windows Security › Device security › Core isolation and
Windows Features, and understand what you are giving up.

**Not enough disk space**: the message shows what is free and what is needed. Free space, or choose another
drive in Choose setup › Advanced › storage folder. Avoid cloud-synced folders.

## Downloads

**Download fails or stops**: click Resume; the partial file is kept. If it fails repeatedly with a checksum
error, the file on the server changed or the download was corrupted; the app deletes the bad file, try again.
If Oracle's site is unreachable, the app uses a checksum built into the app for VirtualBox 7.2.20 and says so.

**Microsoft's page shows no ISO option**: choose "Download Windows 11 Disk Image (ISO)" further down the
page (for Arm64 on the Arm page), pick "Windows 11 (multi-edition ISO)", then the language. The link expires
after 24 hours; that only matters if you wait to click it.

**"This is a Windows x64 ISO but this computer needs ARM64"** (or the reverse): you downloaded from the
wrong Microsoft page. Apple Silicon Macs need the Arm64 page; Windows PCs and Intel Macs need the x64 page.

## Installing VirtualBox

**Windows asks to restart**: do it, then reopen the assistant. It continues at the same step.

**macOS: "System Extension Blocked" or a security prompt**: VirtualBox 7.2 on Apple Silicon uses Apple's
built-in virtualization and normally needs no extension approval. On Intel Macs older versions needed
approval in System Settings › Privacy & Security. Follow Oracle's installer messages.

**Installer says another installation is in progress**: wait for Windows Update or another installer to
finish, then retry.

## Creating the VM

**"A VM named X already exists and was not created by this app"**: pick another name. The app never
modifies or deletes VMs it did not create.

**Creation failed half-way**: click Retry. Completed steps are skipped; the VM is not created twice. If the
disk file exists but is unusable, delete the VM from the Settings tab (only possible for VMs the app created)
and create again.

## Installing Windows

**Nothing happens / black window**: click in the window and press a key when "Press any key to boot from
CD or DVD" appears. If you missed it, close the window with Power off and start again. Leave video memory at
its default 128 MB; other values are known to cause black screens on VirtualBox 7.2.

**"This PC can't run Windows 11"**: the VM was created with UEFI + Secure Boot (+ TPM 2.0 on x64). If you
changed VM settings in VirtualBox, restore them; otherwise create the VM again and export a support report.

**Setup reboots back into the installer**: press nothing at "Press any key"; Windows continues from the disk.
After you reach the desktop, tell the app so it ejects the disc.

**Mouse stuck in the window**: press the Host key (Right Ctrl on Windows, Left Cmd on Mac).

## After installation

**Guest Additions not detected**: inside Windows, open the "VirtualBox Guest Additions" CD in File Explorer
and run `VBoxWindowsAdditions.exe` (x64) or `VBoxWindowsAdditions-arm64.exe` (ARM). Restart Windows. If the
disc is missing, use the VirtualBox window's Devices menu › Insert Guest Additions CD image.

**Network: which problem is it?** The app's network check tells apart: VM not running; no virtual adapter;
cable disconnected; Guest Additions missing (cannot observe); adapter missing inside Windows; driver failed;
link down; no address from NAT DHCP; address but internet unverified; address but pages do not load.
An address such as 10.0.2.15 alone never counts as "internet works".

**ARM: networking stops after Windows Update, Device Manager shows Code 10 on the USB network adapter**
(observed with VirtualBox 7.2.16 / Windows 11 25H2 ARM64). Steps, in order:

1. Restart Windows once. In Device Manager, uninstall the adapter and scan for hardware changes.
2. If it still fails: get the stable virtio-win ISO (0.1.302 or newer) from the official virtio-win project
   (link in the app). Inside Windows, update the failed adapter's driver from `NetKVM\w11\ARM64` on that ISO.
   Windows must show the driver as signed by Microsoft. Do **not** disable driver signature enforcement or
   Secure Boot.
3. Shut Windows down. In the app's Troubleshooting tab, use "Switch adapter to virtio". The previous setting
   is saved; "Restore the original adapter type" puts it back.
4. Start Windows; the adapter should show Up with an address, and a web page should load.

This workaround is only offered when the diagnosis matches; it is not applied to every VM.

**Pages do not load but Windows has an address**: check that this computer itself is online; disconnect
VPNs; some corporate firewalls block VirtualBox's NAT. Try "Unplug and replug the cable" in Troubleshooting.

**Display: "Display failure" notification, frozen picture after resume, tiny text**: see the Display section
in the app's Troubleshooting tab. Prefer a normal shutdown over Save state before long sleeps on ARM Macs
until this is verified. Install Guest Additions before changing scaling. 3D acceleration is intentionally off.

**Windows says it is not activated**: expected without a license. Activate in Settings › System ›
Activation when you have one.

**Windows feels slow**: shut it down and check memory/processors in the Settings tab; on Windows PCs see the
Hyper-V note above; keep the VM on an internal SSD, not an external or cloud-synced drive.

## The assistant itself

**"Another copy seems to be running"**: close the other window. If none exists, delete the stale lock file
`setup.lock` in the app's data folder (shown in the Support report) and reopen.

**Setup state unreadable**: the app moves it aside as `setup-state.corrupt-<date>.json` and starts fresh; your
VM is untouched and can be adopted by creating with the same name.

**Getting help**: Dashboard › Support report › Create report. Review the preview (it is redacted), save it,
and attach it to a GitHub issue. Nothing is uploaded automatically.
