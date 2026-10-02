import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { createMockApi, mockHostReport } from "./lib/mock";
import { resumeStage, vmConfigured } from "./lib/setup-state";
import { CreateVm } from "./screens/CreateVm";
import { InstallDependencies } from "./screens/InstallDependencies";
import { MediaDevices } from "./components/MediaDevices";

afterEach(cleanup);

async function partialSetup() {
  const api = createMockApi({ delayMs: 0 });
  await api.setChoices({ vm_name: "Desktop", base_folder: "/tmp/vms", ram_mb: 8192, cpus: 4, disk_gb: 100, iso_path: null, iso_source: null });
  const state = await api.getState();
  state.stage = "create_vm";
  state.vm = { uuid: "test", name: "Desktop", config_file: null, disk_path: null, base_folder: "/tmp/vms", created_by_app: true, created_at: new Date().toISOString(), original_config: {}, install_iso_attached: false, guest_additions_iso_attached: false, completed_steps: ["Apply memory, processor, graphics and network settings"], configuration_complete: false };
  return { api, state };
}

it("keeps a failed partial VM retryable, including after reopening", async () => {
  const { api, state } = await partialSetup();
  expect(resumeStage(state)).toBe("create_vm");
  render(<CreateVm api={api} state={state} report={mockHostReport()} refresh={async () => {}} onNext={() => {}} />);
  expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
  expect(screen.queryByText(/is ready/)).toBeNull();
  expect(screen.queryByRole("button", { name: "Continue to Windows Setup" })).toBeNull();
});

it("recognizes completed and legacy configurations without recreating them", async () => {
  const { state } = await partialSetup();
  state.vm!.configuration_complete = true;
  expect(resumeStage(state)).toBe("install_windows");
  state.vm!.configuration_complete = false;
  state.vm!.completed_steps.push("Record the setup id");
  expect(vmConfigured(state)).toBe(true);
  expect(resumeStage(state)).toBe("install_windows");
  state.guest.windows_installed = "yes";
  state.stage = "dashboard";
  expect(resumeStage(state)).toBe("dashboard");
});

it("shows required restart even when VirtualBox is detected", async () => {
  const { api, state } = await partialSetup();
  state.virtualbox.reboot_pending = true;
  const report = mockHostReport();
  report.virtualbox = { version_raw: "7.2.20", version: { major: 7, minor: 2, patch: 20, build: null }, vboxmanage: "/VBoxManage", install_dir: "/", guest_additions_iso: null };
  report.virtualbox_version_ok = true;
  render(<InstallDependencies api={api} report={report} state={state} refresh={async () => {}} onNext={() => {}} />);
  expect(screen.getByText("Restart this computer to finish installing VirtualBox")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
});

it("changes capture only on request and surfaces device failures", async () => {
  const api = createMockApi({ delayMs: 0 });
  const change = vi.spyOn(api, "mediaAction");
  render(<MediaDevices api={api} running={true} isMac={true} />);
  expect(change).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Enable microphone" }));
  await waitFor(() => expect(change).toHaveBeenCalledWith({ kind: "microphone", enabled: true }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Find cameras" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Find cameras" }));
  await screen.findByRole("option", { name: /FaceTime/ });
  change.mockRejectedValueOnce(new Error("Camera permission denied"));
  fireEvent.click(screen.getByRole("button", { name: "Connect camera" }));
  await screen.findByText("Camera permission denied");
  expect(change).toHaveBeenLastCalledWith({ kind: "attach_camera", alias: ".1" });
});

it("does not offer camera attachment before Windows is running", () => {
  render(<MediaDevices api={createMockApi()} running={false} isMac={true} />);
  expect(screen.getByRole("button", { name: "Connect camera" })).toBeDisabled();
});
