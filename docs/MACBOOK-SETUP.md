# MacBook setup (Apple Silicon preview)

Use the Apple Silicon DMG from a successful **Apple Silicon preview** Actions run. The artifact includes the exact source commit and SHA-256 checksums. This is an ad-hoc signed preview, not a notarized release. It requires an Apple Silicon Mac, VirtualBox 7.2 or later, and a Windows 11 ARM64 ISO from Microsoft.

## Smooth everyday use

Keep the recommended memory/CPU settings so macOS retains resources. Keep the supported graphics controller and Guest Additions for automatic resizing and mouse integration. After Guest Additions, restart Windows. Use View > Full-screen Mode for a clean view. Prefer a normal Windows shutdown before long periods of Mac sleep; suspend/resume display behavior still needs physical-device testing.

## Microphone

In Finish and verify or Everyday use, choose **Enable microphone**. For a saved or paused VM, resume Windows first. On macOS, select the desired device under System Settings > Sound > Input and grant VirtualBox microphone access under Privacy & Security when prompted. In Windows, allow microphone access, select the input device and record/play back a short Sound Recorder clip. A successful command does not prove audio capture works. **Disable microphone** turns input off again.

## Webcam

Start Windows, close Mac applications using the camera, then choose **Find cameras**, select a device and **Connect camera**. Allow VirtualBox camera access in macOS Privacy & Security and camera access in Windows Privacy & security. Test Windows Camera before opening a calling app. **Disconnect camera** detaches the selected device. Reconnect after a restart if necessary. If permissions change, restart the VM and retry.

Camera attachment is limited to 30 FPS to avoid unnecessary CPU load. Devices and permissions vary, so the app reports errors instead of treating a command as a successful video test. If the built-in camera is unavailable, try an external USB webcam through VirtualBox's Devices menu and test it in Windows.

Oracle's 7.2 release notes say the virtual USB webcam moved into the base package. The user guide still contains older Extension Pack wording. The app does not download or install an Extension Pack, accept its license, or promise that every Mac camera works.

## Privacy and presentation

New VMs use the neutral name **Desktop** and explicitly disable clipboard sharing, clipboard file transfers and drag-and-drop. The assistant does not configure shared folders. Existing VMs retain their settings: shut Windows down and use VirtualBox Settings to disable sharing if desired. Eject installation media after setup. Use full-screen mode or share only the Windows application you intend to show.

These settings reduce accidental host-data exposure and visible host interface clutter. They do not make a VM undetectable: firmware, hardware, display and guest drivers still identify virtual devices. Removing drivers or spoofing unsupported hardware would undermine display, networking and camera reliability. The app preserves supported ARM defaults and Guest Additions.

## Physical Mac validation still needed

- Install the DMG, detect the real host architecture and install VirtualBox.
- Create and boot Windows 11 ARM64; interrupt configuration and confirm Retry resumes it.
- Install Guest Additions, resize the window, test full-screen mode, networking and updates.
- Test microphone recording/playback and webcam video in the intended calling app.
- Test disconnect/reconnect, Mac permission denial/retry, Windows restart and Mac sleep/wake.

GitHub build success verifies compilation and automated tests, not these physical-device checks.

Sources: [Oracle 7.2 release notes](https://docs.oracle.com/en/virtualization/virtualbox/7.2/relnotes/ChangeLog.html), [webcam passthrough](https://docs.oracle.com/en/virtualization/virtualbox/7.2/user/AdvancedTopics.html), [audio controls](https://docs.oracle.com/en/virtualization/virtualbox/7.2/user/vboxmanage.html).
