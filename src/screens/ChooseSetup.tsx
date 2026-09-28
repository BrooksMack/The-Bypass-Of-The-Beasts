import { useEffect, useState } from "react";
import { errorMessage, fmtBytes, type Api } from "../lib/api";
import type { HostReport, IsoInfo, IsoSource, SetupState } from "../lib/types";
import { ErrorBox, Panel } from "../components/ui";

export function ChooseSetup({ api, report, state, onSaved }: { api: Api; report: HostReport; state: SetupState; onSaved: () => void }) {
  const limits = report.limits!;
  const c = state.choices;
  const [name, setName] = useState(c?.vm_name ?? "Windows 11");
  const [folder, setFolder] = useState(c?.base_folder ?? report.default_base_folder);
  const [ram, setRam] = useState(c?.sizing.ram_mb ?? limits.recommended_ram_mb);
  const [cpus, setCpus] = useState(c?.sizing.cpus ?? limits.recommended_cpus);
  const [disk, setDisk] = useState(c?.sizing.disk_gb ?? limits.recommended_disk_gb);
  const [iso, setIso] = useState<string | null>(c?.iso_path ?? null);
  const [isoSource, setIsoSource] = useState<IsoSource | null>(c?.iso_source ?? null);
  const [isoInfo, setIsoInfo] = useState<IsoInfo | null>(null);
  const [isoProblem, setIsoProblem] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const guestArch = report.guest_arch ?? "x64";

  const existingNames = new Set(report.existing_vms.filter((v) => !v.owned_by_this_setup).map((v) => v.name));
  const nameConflict = existingNames.has(name.trim());

  useEffect(() => {
    if (!iso) {
      setIsoInfo(null);
      return;
    }
    let live = true;
    api
      .inspectIso(iso)
      .then((info) => {
        if (!live) return;
        setIsoInfo(info);
        if (!info.looks_like_windows) setIsoProblem("This file does not look like Windows installation media.");
        else if (info.arch !== guestArch) setIsoProblem(`This ISO is for ${info.arch === "arm64" ? "ARM64" : "x64"} but this computer needs the ${guestArch === "arm64" ? "ARM64" : "x64"} version of Windows 11.`);
        else setIsoProblem(null);
      })
      .catch((e) => live && setIsoProblem(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [iso, api, guestArch]);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.setChoices({ vm_name: name.trim(), base_folder: folder, ram_mb: ram, cpus, disk_gb: disk, iso_path: iso, iso_source: iso ? (isoSource ?? "existing") : null });
      onSaved();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <h1>Choose setup</h1>
      <p>The recommended settings fit this computer. You can change them later only by creating a new VM, so take a moment here.</p>
      <ErrorBox message={error} />
      <Panel title="Basics">
        <label htmlFor="vmname">Name for the Windows computer</label>
        <input id="vmname" type="text" value={name} onChange={(e) => setName(e.target.value)} maxLength={64} />
        {nameConflict && <p className="hint" role="alert">A virtual machine with this name already exists in VirtualBox. Pick another name so nothing gets mixed up.</p>}

        <label htmlFor="iso">Windows 11 installation file (ISO)</label>
        <p className="hint" id="iso-hint">
          Windows to install: <strong>{guestArch === "arm64" ? "Windows 11 on ARM (ARM64)" : "Windows 11 (x64)"}</strong>. If you do not have the ISO yet, the next step opens Microsoft's official download page.
        </p>
        <div className="actions" style={{ marginTop: 0 }}>
          <button
            onClick={async () => {
              const p = await api.pickIso();
              if (p) {
                setIso(p);
                setIsoSource("existing");
              }
            }}
            aria-describedby="iso-hint"
          >
            {iso ? "Choose a different ISO" : "I already have the ISO"}
          </button>
          {iso && (
            <button className="link" onClick={() => setIso(null)}>
              Clear
            </button>
          )}
        </div>
        {iso && (
          <p className="hint" style={{ marginTop: "0.5rem" }}>
            Selected: <code>{iso}</code>
            {isoInfo && !isoProblem && ` — ${isoInfo.volume_id}${isoInfo.language_hint ? `, language ${isoInfo.language_hint}` : ""}, ${fmtBytes(isoInfo.size_bytes)}. Looks right for this computer.`}
          </p>
        )}
        {isoProblem && (
          <p className="error-box" role="alert">
            {isoProblem}
          </p>
        )}
      </Panel>

      <details>
        <summary>Advanced settings (memory, processors, disk, storage location)</summary>
        <Panel>
          <label htmlFor="ram">Memory for Windows (MB)</label>
          <input id="ram" type="number" min={limits.min_ram_mb} max={limits.max_ram_mb} step={512} value={ram} onChange={(e) => setRam(Number(e.target.value))} />
          <p className="hint">
            Recommended {limits.recommended_ram_mb} MB. Allowed {limits.min_ram_mb} to {limits.max_ram_mb} MB so this computer keeps enough for itself.
          </p>
          <label htmlFor="cpus">Processors for Windows</label>
          <input id="cpus" type="number" min={limits.min_cpus} max={limits.max_cpus} value={cpus} onChange={(e) => setCpus(Number(e.target.value))} />
          <p className="hint">
            Recommended {limits.recommended_cpus}. Allowed {limits.min_cpus} to {limits.max_cpus}.
          </p>
          <label htmlFor="disk">Virtual disk maximum size (GB)</label>
          <input id="disk" type="number" min={limits.min_disk_gb} max={limits.max_disk_gb} step={10} value={disk} onChange={(e) => setDisk(Number(e.target.value))} />
          <p className="hint">
            This is a maximum, not what is used right away. The disk file starts small and grows as Windows writes data; a {disk} GB disk does not
            occupy {disk} GB immediately. Deleting files inside Windows does not automatically shrink the file on this computer.
          </p>
          <label htmlFor="folder">Where to store the virtual machine</label>
          <input id="folder" type="text" value={folder} onChange={(e) => setFolder(e.target.value)} />
          <div className="actions" style={{ marginTop: "0.4rem" }}>
            <button
              onClick={async () => {
                const p = await api.pickFolder();
                if (p) setFolder(p);
              }}
            >
              Choose folder…
            </button>
          </div>
          <p className="hint">Avoid cloud-synced folders (OneDrive, iCloud Drive, Dropbox): the disk file changes constantly and would sync forever.</p>
        </Panel>
      </details>

      <div className="actions">
        <button className="primary" onClick={save} disabled={busy || !name.trim() || nameConflict || !!isoProblem}>
          {busy ? "Saving…" : "Save and continue"}
        </button>
      </div>
    </>
  );
}
