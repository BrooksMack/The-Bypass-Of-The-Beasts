import { useEffect, useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import type { Diagnosis, HostReport, RepairAction, SetupState, VmStatusReport } from "../lib/types";
import { Badge, ErrorBox, Panel } from "../components/ui";

export function Dashboard({ api, report, state, refresh, onGoTo }: { api: Api; report: HostReport; state: SetupState; refresh: () => Promise<void>; onGoTo: (s: "choose_setup") => void }) {
  const [status, setStatus] = useState<VmStatusReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<"controls" | "troubleshoot" | "settings" | "report">("controls");

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
    setMsg(null);
    try {
      setMsg(await fn());
      await poll();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const info = status?.info ?? null;
  const running = status?.running ?? false;
  const saved = info?.state === "saved";
  const g = status?.guest ?? state.guest;

  return (
    <>
      <h1>{state.vm?.name ?? "Windows"}</h1>
      <p aria-live="polite">
        <strong>{status?.state_label ?? "Checking…"}</strong>
        {status && !status.virtualbox_available && " — VirtualBox is not available right now."}
      </p>
      <ErrorBox message={error} />
      {msg && (
        <p className="notice" role="status">
          {msg}
        </p>
      )}
      <div className="actions" role="tablist" aria-label="Sections">
        {(["controls", "troubleshoot", "settings", "report"] as const).map((t) => (
          <button key={t} role="tab" aria-selected={tab === t} className={tab === t ? "primary" : ""} onClick={() => setTab(t)}>
            {t === "controls" ? "Start and stop" : t === "troubleshoot" ? "Troubleshooting" : t === "settings" ? "Settings" : "Support report"}
          </button>
        ))}
      </div>

      {tab === "controls" && (
        <>
          <Panel title="Start and stop">
            <div className="actions">
              {!running ? (
                <button className="primary" onClick={() => act(api.startVm)} disabled={busy || !status?.virtualbox_available}>
                  {saved ? "Resume Windows" : "Start Windows"}
                </button>
              ) : (
                <>
                  <button className="primary" onClick={() => act(api.startVm)} disabled={busy}>
                    Open the Windows window
                  </button>
                  <button onClick={() => act(api.shutdownVm)} disabled={busy}>
                    Shut down Windows normally
                  </button>
                  <button onClick={() => act(api.saveStateVm)} disabled={busy}>
                    Save state and close
                  </button>
                </>
              )}
              <button onClick={() => api.openVmFolder()}>Open the VM's folder</button>
            </div>
            <ul>
              <li>
                <strong>Shut down normally</strong> asks Windows to shut down, like pressing a power button once. Windows may show "Do you want to shut down?" inside its window; answer there.
              </li>
              <li>
                <strong>Save state</strong> freezes Windows exactly as it is and closes its window in seconds. Resuming brings everything back. Use it for a quick break; use a normal shutdown before updates or when things misbehave.
              </li>
              <li>
                <strong>Closing this assistant</strong> never stops Windows. Windows keeps running in its VirtualBox window until you shut it down or save its state there (File › Close).
              </li>
            </ul>
            {running && (
              <details>
                <summary>Windows is frozen and will not shut down</summary>
                <p>Force power off is like pulling the plug: unsaved work in Windows is lost and Windows may check its disk on the next start. Use it only when nothing else works.</p>
                <div className="actions">
                  <button className="danger" onClick={() => window.confirm("Force power off? Unsaved work inside Windows will be lost.") && act(api.powerOffVm)} disabled={busy}>
                    Force power off
                  </button>
                </div>
              </details>
            )}
          </Panel>
          <Panel title="Health">
            <ul className="status-list">
              <li>
                <Badge v={g.windows_installed} /> Windows installed
              </li>
              <li>
                <Badge v={g.guest_additions} labels={{ yes: `Installed ${g.guest_additions_version ?? ""}`, no: "Not detected" }} /> Guest Additions
              </li>
              <li>
                <Badge v={g.network_link} labels={{ yes: "Connected", no: "No link" }} /> Network link
              </li>
              <li>
                <Badge v={g.internet_access} labels={{ pending: "Not confirmed" }} /> Internet access (confirmed by you)
              </li>
              <li>
                <Badge v={g.windows_updates_complete} labels={{ pending: "Not confirmed" }} /> Windows updates
              </li>
              <li>
                <Badge v={g.windows_activated} labels={{ pending: "Not confirmed" }} /> Activation
              </li>
            </ul>
          </Panel>
        </>
      )}

      {tab === "troubleshoot" && <Troubleshooting api={api} arm={report.guest_arch === "arm64"} running={running} onMessage={setMsg} onError={setError} onChanged={poll} />}

      {tab === "settings" && (
        <Panel title="Settings">
          <dl className="facts">
            <dt>Windows</dt>
            <dd>{info?.ostype ?? state.choices?.guest_arch}</dd>
            <dt>Memory</dt>
            <dd>{info?.memory_mb ?? state.choices?.sizing.ram_mb} MB</dd>
            <dt>Processors</dt>
            <dd>{info?.cpus ?? state.choices?.sizing.cpus}</dd>
            <dt>Network adapter</dt>
            <dd>
              {info?.nic1 ?? "?"} / {info?.nic1_type ?? "?"}
              {state.vm?.original_config?.nictype1 ? ` (originally ${state.vm.original_config.nictype1})` : ""}
            </dd>
            <dt>Installation disc</dt>
            <dd>{state.vm?.install_iso_attached ? "Still inserted" : "Ejected"}</dd>
            <dt>Settings file</dt>
            <dd>
              <code>{info?.config_file ?? state.vm?.config_file ?? "?"}</code>
            </dd>
            <dt>VirtualBox</dt>
            <dd>{report.virtualbox?.version_raw ?? "not found"}</dd>
            <dt>Created by this app</dt>
            <dd>{state.vm?.created_by_app ? "Yes" : "No (adopted)"}</dd>
          </dl>
          <p className="hint">Memory, processors and disk size are managed in VirtualBox itself (VM › Settings) while Windows is shut down. This assistant does not change them after creation.</p>
          {state.vm?.install_iso_attached && (
            <div className="actions">
              <button onClick={() => act(api.ejectInstallIso)} disabled={busy}>
                Eject the Windows installation disc
              </button>
            </div>
          )}
          <DangerZone api={api} state={state} refresh={refresh} onGoTo={onGoTo} />
        </Panel>
      )}

      {tab === "report" && <SupportReport api={api} />}
    </>
  );
}

function Troubleshooting({ api, arm, running, onMessage, onError, onChanged }: { api: Api; arm: boolean; running: boolean; onMessage: (m: string) => void; onError: (e: string | null) => void; onChanged: () => Promise<void> }) {
  const [diag, setDiag] = useState<Diagnosis | null>(null);
  const [userError, setUserError] = useState("");
  const [busy, setBusy] = useState(false);

  const run = async () => {
    setBusy(true);
    onError(null);
    try {
      setDiag(await api.networkDiagnose(userError.trim() || null));
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const apply = async (a: RepairAction) => {
    setBusy(true);
    onError(null);
    try {
      let m = "";
      switch (a.kind) {
        case "start_vm":
          m = await api.startVm();
          break;
        case "attach_nat":
          m = await api.networkApplyRepair({ kind: "attach_nat" });
          break;
        case "connect_cable":
          m = await api.networkApplyRepair({ kind: "connect_cable" });
          break;
        case "reconnect_cable":
          m = await api.networkApplyRepair({ kind: "reconnect_cable" });
          break;
        case "switch_nic_type":
          if (!window.confirm(`Switch the virtual network adapter to '${a.to}'? Do this only after installing the matching driver inside Windows and shutting Windows down. The current setting is saved so it can be restored.`)) return;
          m = await api.networkApplyRepair({ kind: "switch_nic_type", to: a.to });
          break;
        default:
          return;
      }
      onMessage(m);
      await onChanged();
      await run();
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Panel title="Network check">
        <p>This looks at the virtual adapter, the cable, what Windows reports through Guest Additions, and what you confirmed. Fixes are offered only for the problem found.</p>
        <label htmlFor="deverr">If Device Manager in Windows shows an error on the network adapter, paste or type it here (optional)</label>
        <input id="deverr" type="text" placeholder='e.g. "This device cannot start. (Code 10)"' value={userError} onChange={(e) => setUserError(e.target.value)} />
        <div className="actions">
          <button className="primary" onClick={run} disabled={busy}>
            Check the network
          </button>
        </div>
        {diag && (
          <div className={`finding ${diag.problem === "healthy" ? "ok" : diag.problem === "internet_unverified" ? "info" : "warning"}`} role="status">
            <h3>{diag.summary}</h3>
            {diag.actions.map((a, i) => (
              <div key={i} style={{ marginTop: "0.5rem" }}>
                {a.kind === "guest_manual" ? (
                  <>
                    <strong>{a.title}</strong>
                    <ol>
                      {a.steps.map((s, j) => (
                        <li key={j}>{s}</li>
                      ))}
                    </ol>
                  </>
                ) : a.kind === "confirm_in_guest" ? (
                  <p>Open a web page inside Windows, then confirm the result on the Finish and verify step or below.</p>
                ) : a.kind === "install_guest_additions" ? (
                  <p>Install Guest Additions inside Windows (Devices › Insert Guest Additions CD image, then run the installer from the CD drive).</p>
                ) : a.kind === "none" ? null : (
                  <button onClick={() => apply(a)} disabled={busy || (a.kind === "switch_nic_type" && running)}>
                    {a.kind === "start_vm" ? "Start Windows" : a.kind === "attach_nat" ? "Attach a NAT adapter" : a.kind === "connect_cable" ? "Connect the cable" : a.kind === "reconnect_cable" ? "Unplug and replug the cable" : `Switch adapter to ${a.to}${running ? " (shut Windows down first)" : ""}`}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
        <div className="actions">
          <button
            onClick={async () => {
              setBusy(true);
              try {
                onMessage(await api.networkApplyRepair({ kind: "restore_nic_type" }));
              } catch (e) {
                onError(errorMessage(e));
              } finally {
                setBusy(false);
              }
            }}
            disabled={busy || running}
          >
            Restore the original adapter type
          </button>
        </div>
      </Panel>
      <Panel title="Display problems">
        <ul>
          <li>Black screen at start: close the VirtualBox window with Power off and start again. Keep video memory at its default (128 MB); other values are known to cause black screens.</li>
          <li>"Display failure" notification or a frozen picture after Windows resumes: choose Save state from the VirtualBox window (File › Close › Save the machine state), then start again. If it repeats, shut Windows down normally and start it fresh instead of resuming.</li>
          <li>Tiny or huge text: install Guest Additions, then use Windows Settings › Display › Scale. Avoid VirtualBox's scaled mode until Guest Additions run.</li>
          <li>This is a general-purpose VM, not a gaming setup: 3D acceleration is left off on purpose for stability.</li>
        </ul>
        {arm && <p className="hint">On Apple Silicon, display behaviour after sleep/resume has not been fully validated by this project. Prefer a normal Windows shutdown before closing the lid for long periods.</p>}
      </Panel>
      <Panel title="Other common problems">
        <ul>
          <li>Mouse or keyboard stuck in the Windows window: press the Host key (Right Ctrl on Windows, Left Cmd on Mac).</li>
          <li>Windows says it is not activated: expected until you provide a license (Settings › System › Activation).</li>
          <li>Windows is slow: shut it down and check the Settings tab for memory and processors; on Windows PCs, see the troubleshooting guide about Hyper-V/Core isolation trade-offs.</li>
          <li>More help: the project's troubleshooting guide (opens in your browser).</li>
        </ul>
        <div className="actions">
          <button onClick={() => api.openOfficialPage("project_troubleshooting")}>Open the troubleshooting guide</button>
          <button onClick={() => api.openOfficialPage("project_issues")}>Report a problem</button>
        </div>
      </Panel>
    </>
  );
}

function SupportReport({ api }: { api: Api }) {
  const [text, setText] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  return (
    <Panel title="Support report">
      <p>Creates a text report about this computer, the setup state and VirtualBox's logs, with passwords, user names, e-mail addresses, MAC addresses and public IP addresses removed. Nothing is sent anywhere; you decide whether to share it.</p>
      <div className="actions">
        <button
          className="primary"
          onClick={async () => {
            setBusy(true);
            setMsg(null);
            try {
              setText(await api.buildSupportReport());
            } catch (e) {
              setMsg(errorMessage(e));
            } finally {
              setBusy(false);
            }
          }}
          disabled={busy}
        >
          {busy ? "Collecting…" : "Create report"}
        </button>
        {text && (
          <button
            onClick={async () => {
              const p = await api.pickSavePath(`vm-setup-assistant-report-${new Date().toISOString().slice(0, 10)}.txt`);
              if (!p) return;
              try {
                await api.saveSupportReport(p, text);
                setMsg(`Saved to ${p}`);
              } catch (e) {
                setMsg(errorMessage(e));
              }
            }}
          >
            Save as…
          </button>
        )}
      </div>
      {msg && <p className="notice">{msg}</p>}
      {text && (
        <>
          <p className="hint">Preview (review before sharing):</p>
          <pre className="report" tabIndex={0}>
            {text}
          </pre>
        </>
      )}
    </Panel>
  );
}

function DangerZone({ api, state, refresh, onGoTo }: { api: Api; state: SetupState; refresh: () => Promise<void>; onGoTo: (s: "choose_setup") => void }) {
  const [confirmName, setConfirmName] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  return (
    <details>
      <summary>Remove this virtual machine</summary>
      <p>
        Deleting removes the virtual machine <strong>and its virtual disk with everything inside Windows</strong>. This cannot be undone. Type the VM's name to confirm.
      </p>
      {!state.vm?.created_by_app && <p className="hint">This VM was not created by this app, so the app will not delete it. Use VirtualBox.</p>}
      <label htmlFor="delname">Type "{state.vm?.name}" to confirm</label>
      <input id="delname" type="text" value={confirmName} onChange={(e) => setConfirmName(e.target.value)} />
      <div className="actions">
        <button
          className="danger"
          disabled={!state.vm?.created_by_app || confirmName !== state.vm?.name}
          onClick={async () => {
            if (!window.confirm(`Delete "${state.vm?.name}" and its disk permanently?`)) return;
            try {
              setMsg(await api.deleteVm(confirmName));
              await refresh();
              onGoTo("choose_setup");
            } catch (e) {
              setMsg(errorMessage(e));
            }
          }}
        >
          Delete the VM and its disk
        </button>
        <button
          onClick={async () => {
            if (!window.confirm("Forget this setup's progress? The VM stays in VirtualBox untouched; the assistant starts from the beginning.")) return;
            await api.forgetSetup();
            await refresh();
            onGoTo("choose_setup");
          }}
        >
          Forget setup progress (keeps the VM)
        </button>
      </div>
      {msg && <p className="notice">{msg}</p>}
    </details>
  );
}
