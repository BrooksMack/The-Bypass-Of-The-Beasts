// Types mirroring the Rust structs in crates/vmsa-core and src-tauri (serde snake_case).

export type Tri = "yes" | "no" | "unknown";
export type Arch = "x86_64" | "aarch64" | "other";
export type HostOs = "windows" | "mac_os" | "linux" | "other";
export type GuestArch = "x64" | "arm64";
export type Verified = "yes" | "no" | "pending";
export type SupportLevel = "tested" | "untested" | "experimental" | "unsupported";
export type Severity = "blocker" | "warning" | "info";

export type Stage =
  | "welcome"
  | "check_computer"
  | "choose_setup"
  | "obtain_files"
  | "install_dependencies"
  | "create_vm"
  | "install_windows"
  | "finish_and_verify"
  | "dashboard";

export const STAGES: { id: Stage; label: string }[] = [
  { id: "welcome", label: "Welcome" },
  { id: "check_computer", label: "Check this computer" },
  { id: "choose_setup", label: "Choose setup" },
  { id: "obtain_files", label: "Get required files" },
  { id: "install_dependencies", label: "Install VirtualBox" },
  { id: "create_vm", label: "Create the VM" },
  { id: "install_windows", label: "Install Windows" },
  { id: "finish_and_verify", label: "Finish and verify" },
  { id: "dashboard", label: "Everyday use" },
];

export interface HostInfo {
  os: HostOs;
  os_name: string;
  os_version: string;
  arch: Arch;
  app_arch: Arch;
  translated: boolean;
  total_ram_bytes: number;
  available_ram_bytes: number;
  logical_cpus: number;
  cpu_brand: string;
  virtualization: Tri;
  hypervisor_conflict: Tri;
  hypervisor_conflict_detail: string | null;
  hostname_hash: string;
}

export interface Finding {
  code: string;
  severity: Severity;
  title: string;
  detail: string;
  action: string | null;
}

export interface ResourceLimits {
  min_ram_mb: number;
  max_ram_mb: number;
  recommended_ram_mb: number;
  min_cpus: number;
  max_cpus: number;
  recommended_cpus: number;
  min_disk_gb: number;
  max_disk_gb: number;
  recommended_disk_gb: number;
}

export interface Assessment {
  guest_arch: GuestArch | null;
  support_level: SupportLevel;
  limits: ResourceLimits | null;
  findings: Finding[];
}

export interface VboxVersion {
  major: number;
  minor: number;
  patch: number;
  build: number | null;
}

export interface VirtualBoxInstall {
  vboxmanage: string;
  install_dir: string;
  version: VboxVersion;
  version_raw: string;
  guest_additions_iso: string | null;
}

export interface ExistingVm {
  name: string;
  uuid: string;
  owned_by_this_setup: boolean;
  created_by_app: boolean;
}

export interface DiskSpace {
  path: string;
  mount_point: string;
  available_bytes: number;
  total_bytes: number;
}

export interface StorageEstimate {
  windows_iso_bytes: number;
  virtualbox_installer_bytes: number;
  initial_windows_install_bytes: number;
  working_space_bytes: number;
  total_bytes: number;
}

export interface HostReport {
  host: HostInfo;
  assessment: Assessment;
  virtualbox: VirtualBoxInstall | null;
  virtualbox_version_ok: boolean | null;
  virtualbox_min_version: string | null;
  virtualbox_error: string | null;
  existing_vms: ExistingVm[];
  default_base_folder: string;
  disk_at_base: DiskSpace | null;
  limits: ResourceLimits | null;
  storage_estimate: StorageEstimate | null;
  guest_arch: GuestArch | null;
  downloads_dir: string;
}

export interface VmSizing {
  ram_mb: number;
  cpus: number;
  disk_gb: number;
}

export type IsoSource = "existing" | "microsoft_download_page";

export interface SetupChoices {
  guest_arch: GuestArch;
  vm_name: string;
  base_folder: string;
  sizing: VmSizing;
  iso_path: string | null;
  iso_source: IsoSource | null;
  iso_volume_id: string | null;
  identity: IdentityConfig;
}

export interface ChoicesInput {
  vm_name: string;
  base_folder: string | null;
  ram_mb: number;
  cpus: number;
  disk_gb: number;
  iso_path: string | null;
  iso_source: IsoSource | null;
}

export interface IsoInfo {
  path: string;
  size_bytes: number;
  volume_id: string;
  has_efi_x64_boot: boolean;
  has_efi_arm64_boot: boolean;
  has_install_image: boolean;
  arch: GuestArch | null;
  looks_like_windows: boolean;
  language_hint: string | null;
}

export interface DownloadRecord {
  url: string;
  path: string;
  sha256: string | null;
  verified: boolean;
  completed: boolean;
  checksum_source: string | null;
}

export interface VirtualBoxRelease {
  version: string;
  file_name: string;
  url: string;
  sha256: string;
  checksum_source: "oracle_sha256_sums" | "pinned_fallback";
  sha256sums_url: string;
}

export interface VirtualBoxState {
  detected_version: string | null;
  installer_download: DownloadRecord | null;
  installer_outcome: string | null;
  reboot_pending: boolean;
}

export interface VmRecord {
  uuid: string;
  name: string;
  config_file: string | null;
  disk_path: string | null;
  base_folder: string;
  created_by_app: boolean;
  created_at: string;
  original_config: Record<string, string>;
  install_iso_attached: boolean;
  guest_additions_iso_attached: boolean;
  completed_steps: string[];
  identity_applied: boolean;
  identity_original_extradata: Record<string, string>;
  identity_original_modifyvm: Record<string, string>;
}

export interface GuestStatus {
  windows_installed: Verified;
  windows_running: Verified;
  guest_additions: Verified;
  guest_additions_version: string | null;
  network_link: Verified;
  internet_access: Verified;
  windows_updates_complete: Verified;
  windows_activated: Verified;
  evidence: Record<string, string>;
}

export interface CoreError {
  kind: string;
  [k: string]: unknown;
}

export interface SetupState {
  schema_version: number;
  instance_id: string;
  app_version: string;
  created_at: string;
  updated_at: string;
  stage: Stage;
  host_summary: string | null;
  choices: SetupChoices | null;
  virtualbox: VirtualBoxState;
  vm: VmRecord | null;
  guest: GuestStatus;
  last_error: { at: string; stage: Stage; error: CoreError } | null;
  history: { at: string; stage: Stage; message: string }[];
}

export interface ProgressEvent {
  operation: string;
  step: string;
  detail: string | null;
  bytes_done: number | null;
  bytes_total: number | null;
  bytes_per_sec: number | null;
  step_index: number | null;
  step_count: number | null;
}

export type InstallOutcome =
  | { kind: "succeeded" }
  | { kind: "reboot_required" }
  | { kind: "cancelled" }
  | { kind: "another_install_running" }
  | { kind: "failed"; code: number | null; message: string }
  | { kind: "unknown" };

export interface InstallReport {
  outcome: InstallOutcome;
  detected: VirtualBoxInstall | null;
  reboot_pending: boolean;
  message: string;
}

export type VmState =
  | "powered_off"
  | "saved"
  | "running"
  | "paused"
  | "aborted"
  | "starting"
  | "stopping"
  | "saving"
  | "restoring"
  | "other";

export interface VmInfo {
  name: string;
  uuid: string;
  state: VmState;
  state_raw: string;
  memory_mb: number | null;
  cpus: number | null;
  nic1: string | null;
  nic1_type: string | null;
  nic1_cable_connected: boolean | null;
  guest_additions_version: string | null;
  guest_additions_run_level: number | null;
  config_file: string | null;
  log_folder: string | null;
  platform_architecture: string | null;
  ostype: string;
}

export interface VmStatusReport {
  vm: VmRecord | null;
  info: VmInfo | null;
  guest_properties: Record<string, string>;
  guest: GuestStatus;
  running: boolean;
  state_label: string;
  virtualbox_available: boolean;
}

export type GuestItem =
  | "windows_installed"
  | "internet_access"
  | "windows_updates_complete"
  | "windows_activated"
  | "guest_additions";

export type NetworkProblem =
  | "vm_not_running"
  | "no_guest_additions"
  | "no_virtual_adapter"
  | "cable_disconnected"
  | "adapter_missing_in_guest"
  | "driver_failed"
  | "link_down"
  | "no_dhcp_address"
  | "internet_unverified"
  | "internet_unavailable"
  | "healthy";

export type RepairAction =
  | { kind: "start_vm" }
  | { kind: "install_guest_additions" }
  | { kind: "attach_nat" }
  | { kind: "connect_cable" }
  | { kind: "reconnect_cable" }
  | { kind: "switch_nic_type"; from: string | null; to: string; requires_guest_driver: boolean }
  | { kind: "guest_manual"; title: string; steps: string[] }
  | { kind: "confirm_in_guest" }
  | { kind: "none" };

export interface Diagnosis {
  problem: NetworkProblem;
  summary: string;
  actions: RepairAction[];
}

export type RepairRequest =
  | { kind: "attach_nat" }
  | { kind: "connect_cable" }
  | { kind: "reconnect_cable" }
  | { kind: "switch_nic_type"; to: string }
  | { kind: "restore_nic_type" };

// ----- VM identity configuration (compatibility testing) -----

export type NetworkModeCfg =
  | { mode: "nat" }
  | { mode: "nat_network"; name: string }
  | { mode: "bridged"; host_adapter: string }
  | { mode: "host_only"; host_adapter: string };

export interface FirmwareIdentity {
  bios_vendor: string | null;
  bios_version: string | null;
  bios_release_date: string | null;
}

export interface SystemIdentity {
  manufacturer: string | null;
  product_name: string | null;
  version: string | null;
  serial_number: string | null;
  sku: string | null;
  family: string | null;
  uuid: string | null;
  board_manufacturer: string | null;
  board_product: string | null;
  board_serial: string | null;
  chassis_manufacturer: string | null;
  chassis_asset_tag: string | null;
}

export interface StorageIdentity {
  disk_serial: string | null;
  disk_model: string | null;
  disk_firmware_revision: string | null;
}

export interface BrandingReduction {
  paravirt_provider: string | null;
  clear_vbox_oem_strings: boolean;
}

export interface IdentityConfig {
  enabled: boolean;
  firmware: FirmwareIdentity;
  system: SystemIdentity;
  storage: StorageIdentity;
  mac_address: string | null;
  adapter_model: string | null;
  network_mode: NetworkModeCfg;
  branding: BrandingReduction;
}

export interface IdentityEffect {
  area: string;
  change: string;
  visible_as: string;
}

export interface IdentityPreview {
  enabled: boolean;
  effects: IdentityEffect[];
  connectivity_note: string;
  remaining_indicators: string[];
  command_count: number;
}

/** Adapter models the backend accepts (mirrors identity::ADAPTER_MODELS). */
export const ADAPTER_MODELS = ["82540EM", "82543GC", "82545EM", "Am79C973", "virtio", "usbnet"] as const;
export const PARAVIRT_PROVIDERS = ["default", "none", "legacy", "minimal", "hyperv", "kvm"] as const;

/** A fresh, disabled identity configuration (matches Rust `IdentityConfig::default()`). */
export function defaultIdentityConfig(): IdentityConfig {
  return {
    enabled: false,
    firmware: { bios_vendor: null, bios_version: null, bios_release_date: null },
    system: {
      manufacturer: null,
      product_name: null,
      version: null,
      serial_number: null,
      sku: null,
      family: null,
      uuid: null,
      board_manufacturer: null,
      board_product: null,
      board_serial: null,
      chassis_manufacturer: null,
      chassis_asset_tag: null,
    },
    storage: { disk_serial: null, disk_model: null, disk_firmware_revision: null },
    mac_address: null,
    adapter_model: null,
    network_mode: { mode: "nat" },
    branding: { paravirt_provider: null, clear_vbox_oem_strings: false },
  };
}

export interface AppInfo {
  version: string;
  app_arch: Arch;
  data_dir: string;
  downloads_dir: string;
  logs_dir: string;
  startup_warning: string | null;
  busy_with: string | null;
}

export type OfficialPage =
  | "windows11_x64"
  | "windows11_arm64"
  | "virtualbox_downloads"
  | "virtualbox_docs"
  | "virtio_win"
  | "project"
  | "project_issues"
  | "project_troubleshooting";
