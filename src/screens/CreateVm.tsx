import { vmConfigured } from "../lib/setup-state";
import { useEffect, useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import type { HostReport, ProgressEvent, SetupState } from "../lib/types";
import { ErrorBox, OkCard, Panel, ProgressView } from "../components/ui";

export function CreateVm({ api, report, state, refresh, onNext }: { api: Api; report: HostReport; state: SetupState; refresh: () => Promise<void>; onNext: () => void }) {
  const c = state.choices!;
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const done = vmConfigured(state);

  useEffect(() => {
    let off: (() => void) | undefined;
    api.onProgress((e) => e.operation === "create-vm" && setProgress(e)).then((f) => (off = f));
    return () => off?.();
  }, [api]);

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.createVm();
      await refresh();
    } catch (e) {
      setError(errorMessage(e));
      await refresh();
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  return (
    <>
      <h1>Create the virtual machine</h1>
      <ErrorBox message={error} />
      {done && !busy ? (
        <Panel>
          <OkCard title={`"${state.vm!.name}" is ready`}>
            <p>The virtual machine exists in VirtualBox with the Windows installation disc inserted. Nothing has been installed yet.</p>
          </OkCard>
          <div className="actions">
            <button className="primary" onClick={onNext}>
              Continue to Windows Setup
            </button>
          </div>
        </Panel>
      ) : (
        <Panel title="Summary of what will be created">
          <dl className="facts">
            <dt>Name</dt>
            <dd>{c.vm_name}</dd>
            <dt>Windows</dt>
            <dd>{c.guest_arch === "arm64" ? "Windows 11 on ARM (ARM64)" : "Windows 11 (x64)"}</dd>
            <dt>Memory</dt>
            <dd>{c.sizing.ram_mb} MB</dd>
            <dt>Processors</dt>
            <dd>{c.sizing.cpus}</dd>
            <dt>Virtual disk</dt>
            <dd>Up to {c.sizing.disk_gb} GB, grows as used</dd>
            <dt>Stored in</dt>
            <dd>
              <code>{c.base_folder}</code>
            </dd>
            <dt>Network</dt>
            <dd>Shares this computer's connection (NAT)</dd>
            <dt>Firmware</dt>
            <dd>UEFI with Secure Boot{c.guest_arch === "x64" ? " and TPM 2.0" : ""} (required by Windows 11)</dd>
            <dt>VirtualBox</dt>
            <dd>{report.virtualbox?.version_raw ?? "?"}</dd>
          </dl>
          <p className="hint">This only creates an empty virtual computer in VirtualBox. It does not change anything else on this computer. If a step fails, you can retry; completed steps are not repeated.</p>
          {state.vm && state.vm.completed_steps.length > 0 && <p className="hint">Partially created earlier ({state.vm.completed_steps.length} steps done). Retrying continues from where it stopped.</p>}
          {busy ? (
            <>
              <ProgressView ev={progress} label="Creating the virtual machine" />
              <div className="actions">
                <button onClick={() => api.cancelOperation()}>Cancel (keeps what was created)</button>
              </div>
            </>
          ) : (
            <div className="actions">
              <button className="primary" onClick={run}>
                {state.vm ? "Retry" : "Create the virtual machine"}
              </button>
            </div>
          )}
        </Panel>
      )}
    </>
  );
}
