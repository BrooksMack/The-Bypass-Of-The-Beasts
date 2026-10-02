import { STAGES, type SetupState, type Stage } from "./types";

export function vmConfigured(state: SetupState): boolean {
  return !!state.vm && (state.vm.configuration_complete || state.vm.completed_steps.includes("Record the setup id") || state.guest.windows_installed === "yes");
}

export function resumeStage(state: SetupState): Stage {
  if (state.vm) {
    if (state.guest.windows_installed === "yes") return state.stage === "dashboard" ? "dashboard" : "finish_and_verify";
    return vmConfigured(state) ? "install_windows" : "create_vm";
  }
  const order = STAGES.map((s) => s.id);
  if (order.indexOf(state.stage) >= order.indexOf("create_vm")) return "create_vm";
  return state.stage === "welcome" ? "check_computer" : state.stage;
}
