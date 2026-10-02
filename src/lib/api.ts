// The only bridge between the UI and the desktop backend. In a plain browser (no Tauri runtime)
// a clearly labelled MOCK backend is used so the workflow can be developed and tested.

import type {
  AppInfo,
  ChoicesInput,
  Diagnosis,
  DownloadRecord,
  GuestItem,
  GuestStatus,
  HostReport,
  InstallReport,
  IsoInfo,
  OfficialPage,
  ProgressEvent,
  RepairRequest,
  SetupChoices,
  SetupState,
  Stage,
  VirtualBoxInstall,
  VirtualBoxRelease,
  VmRecord,
  VmStatusReport,
} from "./types";

export interface Api {
  readonly isMock: boolean;
  getAppInfo(): Promise<AppInfo>;
  acknowledgeStartupWarning(): Promise<void>;
  getState(): Promise<SetupState>;
  setStage(stage: Stage): Promise<SetupState>;
  inspectHost(): Promise<HostReport>;
  inspectIso(path: string): Promise<IsoInfo>;
  setChoices(input: ChoicesInput): Promise<SetupChoices>;
  resolveVirtualBoxRelease(): Promise<VirtualBoxRelease>;
  downloadVirtualBox(): Promise<DownloadRecord>;
  useExistingVirtualBoxInstaller(path: string): Promise<DownloadRecord>;
  runVirtualBoxInstaller(): Promise<InstallReport>;
  redetectVirtualBox(): Promise<VirtualBoxInstall | null>;
  cancelOperation(): Promise<boolean>;
  createVm(): Promise<VmRecord>;
  vmStatus(): Promise<VmStatusReport>;
  listCameras(): Promise<{ alias: string; name: string }[]>;
  mediaAction(action: { kind: "microphone"; enabled: boolean } | { kind: "attach_camera" | "detach_camera"; alias: string }): Promise<string>;
  startVm(): Promise<string>;
  shutdownVm(): Promise<string>;
  saveStateVm(): Promise<string>;
  powerOffVm(): Promise<string>;
  confirmGuestItem(item: GuestItem, yes: boolean): Promise<GuestStatus>;
  ejectInstallIso(): Promise<string>;
  networkDiagnose(userError: string | null): Promise<Diagnosis>;
  networkApplyRepair(request: RepairRequest): Promise<string>;
  buildSupportReport(): Promise<string>;
  saveSupportReport(path: string, text: string): Promise<void>;
  openOfficialPage(page: OfficialPage): Promise<void>;
  openVmFolder(): Promise<void>;
  deleteVm(confirmName: string): Promise<string>;
  forgetSetup(): Promise<SetupState>;
  // OS dialogs
  pickIso(): Promise<string | null>;
  pickFolder(): Promise<string | null>;
  pickInstaller(): Promise<string | null>;
  pickSavePath(defaultName: string): Promise<string | null>;
  // events
  onProgress(handler: (e: ProgressEvent) => void): Promise<() => void>;
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "kind" in e) {
    const err = e as Record<string, unknown>;
    switch (err.kind) {
      case "virtual_box_not_found":
        return "VirtualBox is not installed or could not be found.";
      case "v_box_manage_failed":
        return `VirtualBox reported a problem: ${String(err.stderr || err.stdout || "").trim() || "unknown error"}`;
      case "v_box_manage_timeout":
        return `VirtualBox did not respond within ${String(err.seconds)} seconds.`;
      case "parse":
        return `Could not understand VirtualBox's response: ${String(err[0] ?? "")}`;
      case "unsupported":
      case "invalid_input":
      case "download":
      case "other":
        return String(err[0] ?? "Something went wrong.");
      case "insufficient_disk":
        return `Not enough free disk space at ${String(err.path)}: ${fmtBytes(Number(err.available_bytes))} free, ${fmtBytes(Number(err.required_bytes))} needed.`;
      case "checksum_mismatch":
        return `The downloaded file did not match its official checksum and was discarded (${String(err.file)}). Download it again.`;
      case "cancelled":
        return "Cancelled.";
      case "busy":
        return "Another setup operation is still running. Wait for it to finish or cancel it.";
      case "vm_name_conflict":
        return `A virtual machine named "${String(err.name)}" already exists in VirtualBox and was not created by this app. Choose a different name.`;
      case "io":
        return `File problem${err.path ? ` (${String(err.path)})` : ""}: ${String(err.message)}`;
      default:
        return JSON.stringify(e);
    }
  }
  if (e instanceof Error) return e.message;
  return String(e);
}

export function fmtBytes(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n)) return "unknown";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i >= 3 ? 1 : 0)} ${units[i]}`;
}

function hasTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const dialog = await import("@tauri-apps/plugin-dialog");
  const single = (v: string | string[] | null): string | null => (Array.isArray(v) ? (v[0] ?? null) : v);
  return {
    isMock: false,
    getAppInfo: () => invoke("get_app_info"),
    acknowledgeStartupWarning: () => invoke("acknowledge_startup_warning"),
    getState: () => invoke("get_state"),
    setStage: (stage) => invoke("set_stage", { stage }),
    inspectHost: () => invoke("inspect_host"),
    inspectIso: (path) => invoke("inspect_iso", { path }),
    setChoices: (input) => invoke("set_choices", { input }),
    resolveVirtualBoxRelease: () => invoke("resolve_virtualbox_release"),
    downloadVirtualBox: () => invoke("download_virtualbox"),
    useExistingVirtualBoxInstaller: (path) => invoke("use_existing_virtualbox_installer", { path }),
    runVirtualBoxInstaller: () => invoke("run_virtualbox_installer"),
    redetectVirtualBox: () => invoke("redetect_virtualbox"),
    cancelOperation: () => invoke("cancel_operation"),
    createVm: () => invoke("create_vm"),
    vmStatus: () => invoke("vm_status"),
    listCameras: () => invoke("list_cameras"),
    mediaAction: (action) => invoke("media_action", { action }),
    startVm: () => invoke("start_vm"),
    shutdownVm: () => invoke("shutdown_vm"),
    saveStateVm: () => invoke("save_state_vm"),
    powerOffVm: () => invoke("power_off_vm"),
    confirmGuestItem: (item, yes) => invoke("confirm_guest_item", { item, yes }),
    ejectInstallIso: () => invoke("eject_install_iso"),
    networkDiagnose: (userError) => invoke("network_diagnose", { userError }),
    networkApplyRepair: (request) => invoke("network_apply_repair", { request }),
    buildSupportReport: () => invoke("build_support_report"),
    saveSupportReport: (path, text) => invoke("save_support_report", { path, text }),
    openOfficialPage: (page) => invoke("open_official_page", { page }),
    openVmFolder: () => invoke("open_vm_folder"),
    deleteVm: (confirmName) => invoke("delete_vm", { confirmName }),
    forgetSetup: () => invoke("forget_setup"),
    pickIso: async () =>
      single(await dialog.open({ multiple: false, directory: false, title: "Choose the Windows 11 ISO", filters: [{ name: "Disc image", extensions: ["iso"] }] })),
    pickFolder: async () => single(await dialog.open({ multiple: false, directory: true, title: "Choose where to store the virtual machine" })),
    pickInstaller: async () =>
      single(await dialog.open({ multiple: false, directory: false, title: "Choose the VirtualBox installer you downloaded", filters: [{ name: "VirtualBox installer", extensions: ["exe", "dmg"] }] })),
    pickSavePath: async (defaultName) => (await dialog.save({ defaultPath: defaultName, title: "Save support report", filters: [{ name: "Text", extensions: ["txt"] }] })) ?? null,
    onProgress: async (handler) => listen<ProgressEvent>("setup-progress", (ev) => handler(ev.payload)),
  };
}

let cached: Promise<Api> | null = null;

/** Resolve the backend once. Tests inject their own via `setApiForTests`. */
export function getApi(): Promise<Api> {
  if (!cached) {
    cached = hasTauri() ? tauriApi() : import("./mock").then((m) => m.createMockApi());
  }
  return cached;
}

export function setApiForTests(api: Api) {
  cached = Promise.resolve(api);
}
