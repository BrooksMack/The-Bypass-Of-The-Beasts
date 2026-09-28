import { useEffect, useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import type { HostReport, InstallReport, ProgressEvent, SetupState } from "../lib/types";
import { ErrorBox, OkCard, Panel, ProgressView } from "../components/ui";

export function InstallDependencies({ api, report, state, refresh, onNext }: { api: Api; report: HostReport; state: SetupState; refresh: (withHost?: boolean) => Promise<void>; onNext: () => void }) {
  const vb = report.virtualbox;
  const ok = !!vb && report.virtualbox_version_ok !== false;
  const isMac = report.host.os === "mac_os";
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [result, setResult] = useState<InstallReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    api.onProgress((e) => e.operation === "install" && setProgress(e)).then((f) => (off = f));
    return () => off?.();
  }, [api]);

  const run = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const r = await api.runVirtualBoxInstaller();
      setResult(r);
      await refresh(true);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  return (
    <>
      <h1>Install VirtualBox</h1>
      <ErrorBox message={error} />
      {ok ? (
        <Panel>
          <OkCard title={`VirtualBox ${vb!.version_raw} is installed and working`}>
            {vb!.guest_additions_iso ? <p className="hint">Guest Additions disc found: it will be inserted into the VM for later.</p> : <p className="hint">The Guest Additions disc was not found in the installation; you can add it later from the VirtualBox window (Devices menu).</p>}
          </OkCard>
          <div className="actions">
            <button className="primary" onClick={onNext}>
              Continue
            </button>
          </div>
        </Panel>
      ) : state.virtualbox.reboot_pending ? (
        <Panel>
          <div className="finding warning">
            <h3>Restart this computer to finish installing VirtualBox</h3>
            <p>The installer asked for a restart. After restarting, open VM Setup Assistant again: it remembers where you are and continues from here.</p>
          </div>
          <div className="actions">
            <button
              onClick={async () => {
                await api.redetectVirtualBox();
                await refresh(true);
              }}
            >
              I restarted, check again
            </button>
          </div>
        </Panel>
      ) : (
        <Panel>
          <p>
            The official VirtualBox installer will open. It is Oracle's own installer; this assistant only starts it and checks the result.
          </p>
          <h3>What you will see</h3>
          {isMac ? (
            <ol>
              <li>A macOS installer window for "Oracle VirtualBox".</li>
              <li>Click Continue through the steps and Install. macOS asks for <strong>your Mac login password</strong> in a system dialog. That dialog belongs to macOS, not to this app.</li>
              <li>When it says the installation was successful, close the installer window. This assistant then checks that VirtualBox works.</li>
            </ol>
          ) : (
            <ol>
              <li>
                A Windows prompt "Do you want to allow this app to make changes to your device?" (User Account Control). Choose <strong>Yes</strong>.
              </li>
              <li>The VirtualBox setup wizard. Keep the default options and click Next until it installs. A warning about network interfaces briefly disconnecting is normal.</li>
              <li>Windows may ask to restart. If it does, restart and reopen this assistant; it continues from here.</li>
            </ol>
          )}
          {vb && report.virtualbox_version_ok === false && (
            <div className="finding warning">
              <h3>An older VirtualBox ({vb.version_raw}) is installed</h3>
              <p>Windows 11 on this computer needs VirtualBox {report.virtualbox_min_version} or newer. The installer will upgrade it in place; existing VMs are kept.</p>
            </div>
          )}
          {busy ? (
            <ProgressView ev={progress} label="Waiting for the installer" />
          ) : (
            <div className="actions">
              <button className="primary" onClick={run} disabled={!state.virtualbox.installer_download?.verified}>
                Open the VirtualBox installer
              </button>
            </div>
          )}
          {result && !ok && (
            <div className={`finding ${result.detected ? "ok" : "warning"}`} role="status">
              <p>{result.message}</p>
            </div>
          )}
        </Panel>
      )}
    </>
  );
}
