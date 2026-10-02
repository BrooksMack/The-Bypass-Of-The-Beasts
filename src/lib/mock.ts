// MOCK backend for browser development and UI tests. It simulates the Rust backend with a
// plausible Apple Silicon host. Nothing here talks to VirtualBox; the UI shows a "MOCK" banner.

import type { Api } from "./api";
import type {
  Diagnosis,
  DownloadRecord,
  GuestStatus,
  HostReport,
  ProgressEvent,
  SetupState,
  Stage,
  VmRecord,
  VmStatusReport,
} from "./types";

const GIB = 1024 ** 3;

export function mockHostReport(): HostReport {
  const host: HostReport["host"] = {
    os: "mac_os",
    os_name: "macOS",
    os_version: "15.6",
    arch: "aarch64",
    app_arch: "aarch64",
    translated: false,
    total_ram_bytes: 24 * GIB,
    available_ram_bytes: 12 * GIB,
    logical_cpus: 12,
    cpu_brand: "Apple M2 Pro",
    virtualization: "yes",
    hypervisor_conflict: "no",
    hypervisor_conflict_detail: null,
    hostname_hash: "abc123",
  };
  const limits = {
    min_ram_mb: 4096,
    max_ram_mb: 18432,
    recommended_ram_mb: 8192,
    min_cpus: 2,
    max_cpus: 11,
    recommended_cpus: 4,
    min_disk_gb: 64,
    max_disk_gb: 2000,
    recommended_disk_gb: 100,
  };
  return {
    host,
    assessment: { guest_arch: "arm64", support_level: "untested", limits, findings: [] },
    virtualbox: null,
    virtualbox_version_ok: null,
    virtualbox_min_version: "7.2.0",
    virtualbox_error: null,
    existing_vms: [],
    default_base_folder: "/Users/[you]/VirtualBox VMs",
    disk_at_base: { path: "/Users/[you]/VirtualBox VMs", mount_point: "/", available_bytes: 300 * GIB, total_bytes: 1000 * GIB },
    limits,
    storage_estimate: { windows_iso_bytes: 7 * GIB, virtualbox_installer_bytes: 250 * 1024 ** 2, initial_windows_install_bytes: 30 * GIB, working_space_bytes: 8 * GIB, total_bytes: 45.25 * GIB },
    guest_arch: "arm64",
    downloads_dir: "/Users/[you]/Library/Application Support/org.vm-setup-assistant.VM Setup Assistant/downloads",
  };
}

function freshState(): SetupState {
  const now = new Date().toISOString();
  return {
    schema_version: 1,
    instance_id: "mock-instance",
    app_version: "0.0.0-mock",
    created_at: now,
    updated_at: now,
    stage: "welcome",
    host_summary: null,
    choices: null,
    virtualbox: { detected_version: null, installer_download: null, installer_outcome: null, reboot_pending: false },
    vm: null,
    guest: {
      windows_installed: "pending",
      windows_running: "pending",
      guest_additions: "pending",
      guest_additions_version: null,
      network_link: "pending",
      internet_access: "pending",
      windows_updates_complete: "pending",
      windows_activated: "pending",
      evidence: {},
    },
    last_error: null,
    history: [],
  };
}

export interface MockOptions {
  /** Speed multiplier for simulated delays (tests use 0). */
  delayMs?: number;
}

export function createMockApi(opts: MockOptions = {}): Api {
  const delayMs = opts.delayMs ?? 300;
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  let state = freshState();
  let vbInstalled = false;
  let running = false;
  let gaInstalled = false;
  const listeners = new Set<(e: ProgressEvent) => void>();
  const emit = (e: ProgressEvent) => listeners.forEach((l) => l(e));
  let cancelled = false;

  const report = mockHostReport();

  const vmStatus = (): VmStatusReport => {
    const guest: GuestStatus = { ...state.guest };
    guest.windows_running = running ? "yes" : "no";
    if (running && gaInstalled) {
      guest.guest_additions = "yes";
      guest.guest_additions_version = "7.2.20";
      guest.windows_installed = "yes";
      guest.network_link = "yes";
    } else if (running) {
      guest.guest_additions = "no";
    }
    state.guest = guest;
    return {
      vm: state.vm,
      info: state.vm
        ? {
            name: state.vm.name,
            uuid: state.vm.uuid,
            state: running ? "running" : "powered_off",
            state_raw: running ? "running" : "poweroff",
            memory_mb: state.choices?.sizing.ram_mb ?? null,
            cpus: state.choices?.sizing.cpus ?? null,
            nic1: "nat",
            nic1_type: "usbnet",
            nic1_cable_connected: true,
            guest_additions_version: gaInstalled ? "7.2.20 r175154" : null,
            guest_additions_run_level: gaInstalled ? 3 : null,
            config_file: `${state.vm.base_folder}/${state.vm.name}/${state.vm.name}.vbox`,
            log_folder: `${state.vm.base_folder}/${state.vm.name}/Logs`,
            platform_architecture: "ARM",
            ostype: "Windows11_arm64",
          }
        : null,
      guest_properties: running && gaInstalled ? { "/VirtualBox/GuestInfo/Net/0/V4/IP": "10.0.2.15", "/VirtualBox/GuestInfo/Net/0/Status": "Up" } : {},
      guest,
      running,
      state_label: state.vm ? (running ? "Windows is running" : "Windows is off") : "No VM yet",
      virtualbox_available: vbInstalled,
    };
  };

  return {
    isMock: true,
    async getAppInfo() {
      return { version: "0.0.0-mock", app_arch: "aarch64", data_dir: "/mock/data", downloads_dir: report.downloads_dir, logs_dir: "/mock/logs", startup_warning: null, busy_with: null };
    },
    async acknowledgeStartupWarning() {},
    async getState() {
      return structuredClone(state);
    },
    async setStage(stage: Stage) {
      state.stage = stage;
      return structuredClone(state);
    },
    async inspectHost() {
      await sleep(delayMs);
      if (state.stage === "welcome") state.stage = "check_computer";
      state.host_summary = "macOS 15.6 on ARM 64-bit, 24.0 GB RAM, 12 CPUs";
      return { ...report, virtualbox: vbInstalled ? { vboxmanage: "/Applications/VirtualBox.app/Contents/MacOS/VBoxManage", install_dir: "/Applications/VirtualBox.app/Contents/MacOS", version: { major: 7, minor: 2, patch: 20, build: 175154 }, version_raw: "7.2.20r175154", guest_additions_iso: "/Applications/VirtualBox.app/Contents/MacOS/VBoxGuestAdditions.iso" } : null, virtualbox_version_ok: vbInstalled ? true : null };
    },
    async inspectIso(path) {
      await sleep(delayMs);
      const arm = !/x64/i.test(path);
      return { path, size_bytes: 6.4 * GIB, volume_id: arm ? "CCCOMA_A64FRE_EN-US_DV9" : "CCCOMA_X64FRE_EN-US_DV9", has_efi_x64_boot: !arm, has_efi_arm64_boot: arm, has_install_image: true, arch: arm ? "arm64" : "x64", looks_like_windows: true, language_hint: "EN-US" };
    },
    async setChoices(input) {
      if (!input.vm_name.trim()) throw { kind: "invalid_input", 0: "Please give the VM a name." };
      if (input.ram_mb < 4096) throw { kind: "invalid_input", 0: "Windows 11 needs at least 4096 MB of memory." };
      if (input.iso_path && /x64/i.test(input.iso_path)) throw { kind: "invalid_input", 0: "This is a Windows x64 ISO, but this computer needs the Windows 11 ARM64 ISO." };
      state.choices = { guest_arch: "arm64", vm_name: input.vm_name, base_folder: input.base_folder ?? report.default_base_folder, sizing: { ram_mb: input.ram_mb, cpus: input.cpus, disk_gb: input.disk_gb }, iso_path: input.iso_path, iso_source: input.iso_source, iso_volume_id: input.iso_path ? "CCCOMA_A64FRE_EN-US_DV9" : null };
      if (state.stage === "check_computer") state.stage = "choose_setup";
      return state.choices;
    },
    async resolveVirtualBoxRelease() {
      return { version: "7.2.20", file_name: "VirtualBox-7.2.20-175154-macOSArm64.dmg", url: "https://download.virtualbox.org/virtualbox/7.2.20/VirtualBox-7.2.20-175154-macOSArm64.dmg", sha256: "186eb4734234bcf20bc71c577060045507aa0769912e7664735ad18cd7458d8d", checksum_source: "pinned_fallback", sha256sums_url: "https://www.virtualbox.org/download/hashes/7.2.20/SHA256SUMS" };
    },
    async downloadVirtualBox() {
      cancelled = false;
      const total = 250 * 1024 ** 2;
      for (let i = 0; i <= 10; i++) {
        if (cancelled) throw { kind: "cancelled" };
        emit({ operation: "download", step: i < 10 ? "Downloading VirtualBox 7.2.20 (MOCK)" : "Verifying VirtualBox 7.2.20 (SHA-256)", detail: null, bytes_done: (total * i) / 10, bytes_total: total, bytes_per_sec: 12 * 1024 ** 2, step_index: null, step_count: null });
        await sleep(delayMs / 2);
      }
      const rec: DownloadRecord = { url: "https://download.virtualbox.org/virtualbox/7.2.20/VirtualBox-7.2.20-175154-macOSArm64.dmg", path: `${report.downloads_dir}/VirtualBox-7.2.20-175154-macOSArm64.dmg`, sha256: "186eb473...", verified: true, completed: true, checksum_source: "PinnedFallback" };
      state.virtualbox.installer_download = rec;
      return rec;
    },
    async useExistingVirtualBoxInstaller(path) {
      const rec: DownloadRecord = { url: "", path, sha256: "186eb473...", verified: true, completed: true, checksum_source: "PinnedFallback" };
      state.virtualbox.installer_download = rec;
      return rec;
    },
    async runVirtualBoxInstaller() {
      emit({ operation: "install", step: "Follow the VirtualBox installer (MOCK)", detail: "macOS will ask for your Mac login password.", bytes_done: null, bytes_total: null, bytes_per_sec: null, step_index: null, step_count: null });
      await sleep(delayMs * 2);
      vbInstalled = true;
      state.virtualbox.detected_version = "7.2.20r175154";
      state.stage = "install_dependencies";
      return { outcome: { kind: "unknown" }, detected: (await this.inspectHost()).virtualbox, reboot_pending: false, message: "VirtualBox 7.2.20 is installed." };
    },
    async redetectVirtualBox() {
      return (await this.inspectHost()).virtualbox;
    },
    async cancelOperation() {
      cancelled = true;
      return true;
    },
    async createVm() {
      if (!state.choices?.iso_path) throw { kind: "invalid_input", 0: "Select the Windows 11 ISO first." };
      const steps = ["Register the new virtual machine", "Apply memory, processor, graphics and network settings", "Create the virtual hard disk (grows as Windows uses it)", "Attach the virtual hard disk", "Insert the Windows installation disc", "Insert the VirtualBox Guest Additions disc", "Prepare UEFI Secure Boot keys (required by Windows 11)", "Record that this app created the VM"];
      for (let i = 0; i < steps.length; i++) {
        emit({ operation: "create-vm", step: steps[i], detail: null, bytes_done: null, bytes_total: null, bytes_per_sec: null, step_index: i + 1, step_count: steps.length });
        await sleep(delayMs / 2);
      }
      const vm: VmRecord = { uuid: "3f2504e0-4f89-41d3-9a0c-0305e82c3301", name: state.choices.vm_name, config_file: null, disk_path: null, base_folder: state.choices.base_folder, created_by_app: true, created_at: new Date().toISOString(), original_config: {}, install_iso_attached: true, guest_additions_iso_attached: true, completed_steps: steps, configuration_complete: true };
      state.vm = vm;
      state.stage = "install_windows";
      return vm;
    },
    async vmStatus() {
      return vmStatus();
    },
    async listCameras() { return [{ alias: ".1", name: "FaceTime HD Camera (simulated)" }]; },
    async mediaAction(action) { return `Applied ${action.kind} (MOCK). Test inside Windows to verify.`; },
    async startVm() {
      running = true;
      setTimeout(() => {
        if (state.guest.windows_installed === "yes") gaInstalled = true;
      }, delayMs * 4);
      return "Starting Windows in its own window (MOCK).";
    },
    async shutdownVm() {
      running = false;
      return "Asked Windows to shut down (MOCK).";
    },
    async saveStateVm() {
      running = false;
      return "Saved (MOCK).";
    },
    async powerOffVm() {
      running = false;
      return "Powered off (MOCK).";
    },
    async confirmGuestItem(item, yes) {
      const v = yes ? "yes" : "no";
      state.guest = { ...state.guest, [item]: v, evidence: { ...state.guest.evidence, [item]: `User confirmed ${v}` } };
      if (item === "windows_installed" && yes) {
        state.stage = "finish_and_verify";
        if (running) gaInstalled = true;
      }
      return state.guest;
    },
    async ejectInstallIso() {
      if (state.vm) state.vm.install_iso_attached = false;
      return "The Windows installation disc was ejected (MOCK).";
    },
    async networkDiagnose(userError): Promise<Diagnosis> {
      if (!running) return { problem: "vm_not_running", summary: "Windows is not running, so its network cannot be checked yet.", actions: [{ kind: "start_vm" }] };
      if (userError && /code 10/i.test(userError)) return { problem: "driver_failed", summary: "Windows' USB network driver (UsbNcm) failed to start (Code 10).", actions: [{ kind: "guest_manual", title: "Try the simple fixes first", steps: ["Restart Windows once."] }, { kind: "switch_nic_type", from: "usbnet", to: "virtio", requires_guest_driver: true }] };
      if (state.guest.internet_access === "yes") return { problem: "healthy", summary: "Network link is up, an address is assigned, and you confirmed a web page loads.", actions: [{ kind: "none" }] };
      return { problem: "internet_unverified", summary: "Windows has a network address. An address alone does not prove internet access, so please open a web page inside Windows to confirm.", actions: [{ kind: "confirm_in_guest" }] };
    },
    async networkApplyRepair(request) {
      return `Applied ${request.kind} (MOCK).`;
    },
    async buildSupportReport() {
      return `VM Setup Assistant support report (MOCK)\nApp version: 0.0.0-mock\n\n===== Host =====\n${JSON.stringify(report.host, null, 2)}\n\n===== Setup state =====\n${JSON.stringify({ ...state, history: undefined }, null, 2)}`;
    },
    async saveSupportReport() {},
    async openOfficialPage(page) {
      console.info("[MOCK] would open official page", page);
    },
    async openVmFolder() {},
    async deleteVm(confirmName) {
      if (!state.vm || confirmName !== state.vm.name) throw { kind: "invalid_input", 0: "The name you typed does not match the VM name." };
      state.vm = null;
      running = false;
      gaInstalled = false;
      state.stage = "choose_setup";
      return "Deleted (MOCK).";
    },
    async forgetSetup() {
      state = freshState();
      return structuredClone(state);
    },
    async pickIso() {
      return window.prompt("[MOCK] Path to the Windows 11 ISO", "/Users/you/Downloads/Win11_25H2_English_Arm64.iso");
    },
    async pickFolder() {
      return window.prompt("[MOCK] Folder", report.default_base_folder);
    },
    async pickInstaller() {
      return window.prompt("[MOCK] Installer path", `${report.downloads_dir}/VirtualBox-7.2.20-175154-macOSArm64.dmg`);
    },
    async pickSavePath(defaultName) {
      return window.prompt("[MOCK] Save as", `/Users/you/Desktop/${defaultName}`);
    },
    async onProgress(handler) {
      listeners.add(handler);
      return () => listeners.delete(handler);
    },
  };
}
