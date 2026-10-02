import { MediaDevices } from "../components/MediaDevices";
import { useEffect, useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import type { GuestItem, HostReport, SetupState, VmStatusReport } from "../lib/types";
import { Badge, ErrorBox, NextPanel, Panel } from "../components/ui";

export function FinishVerify({ api, report, state, refresh, onNext }: { api: Api; report: HostReport; state: SetupState; refresh: () => Promise<void>; onNext: () => void }) {
  const [status, setStatus] = useState<VmStatusReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const arm = report.guest_arch === "arm64";
  const g = status?.guest ?? state.guest;

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

  const confirm = async (item: GuestItem, yes: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await api.confirmGuestItem(item, yes);
      await refresh();
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
      <h1>Finish and verify</h1>
      <ErrorBox message={error} />
      {msg && (
        <p className="notice" role="status">
          {msg}
        </p>
      )}
      <div className="two-col">
        <div>
          <Panel title="Status">
            <ul className="status-list">
              <li>
                <Badge v={g.windows_installed} /> Windows is installed <span className="hint">{g.evidence.windows_installed ?? ""}</span>
              </li>
              <li>
                <Badge v={g.windows_running} labels={{ yes: "Running", no: "Off" }} /> Windows is running
              </li>
              <li>
                <Badge v={g.guest_additions} labels={{ yes: `Installed ${g.guest_additions_version ?? ""}`, no: "Not detected" }} /> Guest Additions (better display, mouse, clipboard)
              </li>
              <li>
                <Badge v={g.network_link} labels={{ yes: "Connected", no: "No link" }} /> Network link <span className="hint">{g.evidence.network_link ?? ""}</span>
              </li>
              <li>
                <Badge v={g.internet_access} labels={{ pending: "Not confirmed" }} /> Internet access (you confirm this; an address alone does not prove it)
              </li>
              <li>
                <Badge v={g.windows_updates_complete} labels={{ pending: "Not confirmed" }} /> Windows updates complete
              </li>
              <li>
                <Badge v={g.windows_activated} labels={{ pending: "Not confirmed" }} /> Windows is activated
              </li>
            </ul>
            <div className="actions">
              {!running ? (
                <button
                  className="primary"
                  onClick={async () => {
                    setBusy(true);
                    try {
                      setMsg(await api.startVm());
                    } catch (e) {
                      setError(errorMessage(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                  disabled={busy}
                >
                  Start Windows
                </button>
              ) : null}
              <button onClick={poll} disabled={busy}>
                Refresh
              </button>
            </div>
          </Panel>
          <Panel title="Confirm what you can see inside Windows">
            <p className="hint">These facts live inside Windows and cannot be read from outside without your Windows password, which this app never asks for.</p>
            <div className="actions">
              <button onClick={() => confirm("internet_access", true)} disabled={busy}>
                A web page loaded in Windows
              </button>
              <button onClick={() => confirm("internet_access", false)} disabled={busy}>
                Web pages do not load
              </button>
            </div>
            <div className="actions">
              <button onClick={() => confirm("windows_updates_complete", true)} disabled={busy}>
                Windows Update says up to date
              </button>
              <button onClick={() => confirm("windows_activated", true)} disabled={busy}>
                Windows is activated
              </button>
              <button onClick={() => confirm("windows_activated", false)} disabled={busy}>
                Not activated (no license yet)
              </button>
            </div>
          </Panel>
          <MediaDevices api={api} running={running} isMac={report.host.os === "mac_os"} />
          <div className="actions">
            <button className="primary" onClick={onNext}>
              Go to everyday use
            </button>
          </div>
        </div>
        <NextPanel>
          <h3>1. Install Guest Additions</h3>
          <ol>
            <li>In the VirtualBox window's menu, choose <strong>Devices › Insert Guest Additions CD image</strong> (the disc was already inserted during creation; this just makes sure).</li>
            <li>In Windows, open File Explorer, open the CD drive "VirtualBox Guest Additions" and run <code>{arm ? "VBoxWindowsAdditions-arm64.exe" : "VBoxWindowsAdditions.exe"}</code>. Approve the Windows security prompt.</li>
            <li>Restart Windows when asked. The status list updates itself once Guest Additions run.</li>
          </ol>
          <h3>2. Check the internet</h3>
          <p>Open Microsoft Edge in Windows and load any web page. Then click "A web page loaded in Windows".</p>
          <h3>3. Run Windows Update</h3>
          <p>Settings › Windows Update › Check for updates. Repeat until it says you are up to date. Updates can take an hour. {arm ? "If networking stops working after updates, use Troubleshooting on the next screen: this is a known scenario on ARM VMs." : ""}</p>
          <h3>4. Activation (optional)</h3>
          <p>Settings › System › Activation. Enter a product key you own, or sign in with a Microsoft account that has a digital license. Without one, Windows keeps working with a watermark.</p>
        </NextPanel>
      </div>
    </>
  );
}
