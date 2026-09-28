import { useEffect, useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import type { HostReport, SetupState, VmStatusReport } from "../lib/types";
import { ErrorBox, NextPanel, Panel } from "../components/ui";

export function InstallWindows({ api, report, refresh, onNext }: { api: Api; report: HostReport; state: SetupState; refresh: () => Promise<void>; onNext: () => void }) {
  const [status, setStatus] = useState<VmStatusReport | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const arm = report.guest_arch === "arm64";

  const poll = async () => {
    try {
      setStatus(await api.vmStatus());
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  useEffect(() => {
    void poll();
    const t = setInterval(poll, 5000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const act = async (fn: () => Promise<string>) => {
    setBusy(true);
    setError(null);
    try {
      setMsg(await fn());
      await poll();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const running = status?.running ?? false;

  return (
    <>
      <h1>Install Windows</h1>
      <ErrorBox message={error} />
      {msg && (
        <p className="notice" role="status">
          {msg}
        </p>
      )}
      <div className="two-col">
        <div>
          <Panel title={status?.state_label ?? "Checking…"}>
            <p>Windows Setup runs inside the VirtualBox window, which opens separately from this assistant. Keep this window open for guidance.</p>
            <div className="actions">
              {!running ? (
                <button className="primary" onClick={() => act(api.startVm)} disabled={busy || !status?.virtualbox_available}>
                  Start Windows Setup
                </button>
              ) : (
                <>
                  <button onClick={() => act(api.saveStateVm)} disabled={busy}>
                    Pause (save state)
                  </button>
                  <button onClick={() => act(api.shutdownVm)} disabled={busy}>
                    Ask Windows to shut down
                  </button>
                </>
              )}
            </div>
            {running && status?.guest.guest_additions === "yes" && <p className="hint">Guest Additions detected: Windows is installed and running.</p>}
          </Panel>
          <Panel title="When Windows is installed">
            <p>Once you reach the Windows desktop for the first time, tell the assistant so it can eject the installation disc and move on to the finishing steps.</p>
            <div className="actions">
              <button
                className="primary"
                onClick={async () => {
                  setBusy(true);
                  try {
                    await api.confirmGuestItem("windows_installed", true);
                    try {
                      await api.ejectInstallIso();
                    } catch (e) {
                      setError(errorMessage(e));
                    }
                    await refresh();
                    onNext();
                  } finally {
                    setBusy(false);
                  }
                }}
                disabled={busy}
              >
                I reached the Windows desktop
              </button>
            </div>
          </Panel>
        </div>
        <NextPanel>
          <ol>
            <li>
              Click <strong>Start Windows Setup</strong>. A VirtualBox window opens.
            </li>
            <li>
              If you see <em>"Press any key to boot from CD or DVD"</em>, click inside the window and press a key right away. (If you miss it and see a boot error or shell, close the window choosing <em>Power off</em>, then start again.)
            </li>
            <li>Choose your language and keyboard, then click Install now.</li>
            <li>
              When asked for a product key, choose <strong>I don't have a product key</strong>. Pick the edition you own (Windows 11 Home for most people). You can activate later inside Windows.
            </li>
            <li>Accept Microsoft's license terms. Choose "Custom: Install Windows only" and select the only disk shown (it is the virtual disk, not your real one).</li>
            <li>Windows copies files and restarts a few times. Do nothing during restarts; the window stays open.</li>
            <li>
              In the Windows out-of-box setup (region, keyboard, network, account), type everything <strong>into the Windows window</strong>. Windows 11 Home asks you to sign in with a Microsoft account and may send a code to your phone or e-mail; enter it there. Then create a PIN when asked.
            </li>
            <li>
              When the Windows desktop appears, click <strong>I reached the Windows desktop</strong> here.
            </li>
          </ol>
          <p className="hint">
            {arm
              ? "On ARM Macs the network adapter appears as an Ethernet connection inside Windows. If Windows says it cannot connect during setup, continue anyway if offered; the assistant checks networking afterwards."
              : "If the mouse pointer gets captured by the VirtualBox window, press the Host key (Right Ctrl on Windows, Left Cmd on Mac) to release it."}
          </p>
          <p className="hint">Closing this assistant does not stop Windows. The VirtualBox window keeps running; you can reopen the assistant at any time.</p>
        </NextPanel>
      </div>
      <details>
        <summary>Something went wrong?</summary>
        <ul>
          <li>Black window that never shows Windows Setup: close the VirtualBox window choosing Power off, then Start again. Leave video memory at its default; changing it causes black screens on some versions.</li>
          <li>"This PC can't run Windows 11": the VM was created with Secure Boot{arm ? "" : " and TPM 2.0"}. Power off and retry creation from the previous step; if it persists, export a support report from the dashboard.</li>
          <li>Setup restarts back into the installer instead of continuing: press nothing at "Press any key"; it then boots from the virtual disk.</li>
        </ul>
      </details>
    </>
  );
}
