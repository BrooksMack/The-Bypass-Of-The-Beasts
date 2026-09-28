# Manual test checklist (real hardware)

GitHub-hosted runners cannot run VirtualBox VMs, so these tests are done by a person on real hardware.
Record results in `docs/COMPATIBILITY.md` (date, app version, OS version, VirtualBox version, Windows build).
Never include personal paths, account names or product keys in what you record.

## A. Fresh machine, full path (the main scenario)

1. Download the build for this computer from Releases. Verify its SHA-256 against `*-SHA256SUMS.txt`.
2. Install/open the app. Note any OS warning (SmartScreen / Gatekeeper) and whether the documented workaround works.
3. Welcome › Set up Windows. **Check this computer** shows correct OS, processor type, memory, CPUs, free space, virtualization. On an Apple Silicon Mac the guest must be "Windows 11 on ARM (ARM64)". If the app is the wrong architecture, it must say so.
4. **Choose setup**: defaults are 8 GB / 4 CPUs / 100 GB when the host has 16 GB+ and 8+ cores; smaller on smaller hosts. Try a name with spaces and an accent (e.g. `Émilie's Windows`). Try a name that already exists in VirtualBox: the app must refuse.
5. **Get required files**: VirtualBox download shows size, speed and progress; Pause then Resume continues instead of restarting; the file is reported as verified. Select a wrong ISO (x64 on an ARM Mac, or a Linux ISO): the app must refuse with a clear message. Select the right ISO: label and language shown.
6. **Install VirtualBox**: official installer opens; OS permission prompt appears as described; after it finishes the app detects the version. On Windows, if a restart is requested: restart, reopen the app, confirm it resumes at this step and detects VirtualBox.
7. **Create the VM**: summary is correct; stages are shown by name; the VM appears in the VirtualBox app with EFI, Secure Boot on, (TPM 2.0 on x64), correct memory/CPUs, disk file small (dynamic), Windows ISO on port 1, Guest Additions ISO on port 2.
8. **Install Windows**: Start; "Press any key" appears and works; Windows Setup runs; "I don't have a product key" path works; OOBE (account, code, PIN) completed inside Windows. Click "I reached the Windows desktop": ISO ejected (check VirtualBox storage settings).
9. **Finish and verify**: Guest Additions installed from the inserted disc (`VBoxWindowsAdditions-arm64.exe` on ARM); after reboot the status turns to Installed with a version. Network link shows Connected with 10.0.2.x. Load a web page; confirm. Run Windows Update to completion.
10. **Dashboard**: Shut down normally (Windows shuts down); Start; Save state (window closes quickly); Resume (returns where it was); Open the VM folder; Force power off appears only when running and asks for confirmation.
11. Close the assistant while Windows runs: Windows keeps running. Reopen: dashboard shows "running".

## B. Interruption and recovery

- Quit the app during the VirtualBox download; reopen; Resume continues from the partial file.
- Disconnect the network mid-download: clear error; retry resumes.
- Cancel the VirtualBox installer: app reports "cancelled", nothing changed, retry works.
- Quit the app during VM creation (after "Register" but before the disk step): reopen; Retry completes the remaining steps without creating a second VM. `VBoxManage list vms` shows one VM.
- Delete the setup-state file while a VM exists: reopen; Create the VM adopts the existing VM (same name and marker) instead of failing or duplicating.
- Corrupt the setup-state file: app starts fresh and leaves a `.corrupt-*.json` backup.
- Start a second copy of the app: the first window is focused instead.
- Fill the disk below the requirement before "Create the VM": app blocks with amounts and a way to change location.

## C. Regression scenarios from the earlier manual ARM setup

- **UsbNcm Code 10 after Windows Update (ARM)**: after updates, if networking drops and Device Manager shows Code 10 on the USB network adapter, run Troubleshooting › Check the network with the error text. Expect: simple fixes first (restart, reinstall), then the virtio-win NetKVM instructions and a "Switch adapter to virtio" action that is disabled while Windows runs. After shutting down and switching, Windows must show Connected. "Restore the original adapter type" must put `usbnet` back.
- **Do not assume the workaround**: on a fresh VM with working networking, the troubleshooter must report "unverified" (until a page is confirmed) or "healthy", never suggest switching adapters.
- **Display after resume (ARM)**: save state, wait 10 minutes, resume; observe whether the console stays usable and whether a "Display failure" notification appears. Record exact VirtualBox and Windows versions. Test the documented recovery (Save state, start again; then normal shutdown, start).
- **Activation**: with no key, Windows shows "not activated"; the app shows Activation as "Not confirmed" until the user answers; confirming "Not activated" shows "Not yet".

## D. Safety

- The app must never delete a VM it did not create (create one in VirtualBox with the same name first, then try setup).
- "Delete the VM and its disk" requires typing the exact name, then a second confirmation, and refuses while running.
- Support report: create it, and search the preview for your user name, e-mail, MAC address and public IP: none may appear.
