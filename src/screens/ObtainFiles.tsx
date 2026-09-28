import { useEffect, useState } from "react";
import { errorMessage, fmtBytes, type Api } from "../lib/api";
import type { HostReport, IsoInfo, ProgressEvent, SetupState, VirtualBoxRelease } from "../lib/types";
import { ErrorBox, OkCard, Panel, ProgressView } from "../components/ui";

export function ObtainFiles({ api, report, state, refresh, onNext }: { api: Api; report: HostReport; state: SetupState; refresh: () => Promise<void>; onNext: () => void }) {
  const guestArch = report.guest_arch ?? "x64";
  const vbInstalled = !!report.virtualbox && report.virtualbox_version_ok !== false;
  const [release, setRelease] = useState<VirtualBoxRelease | null>(null);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isoInfo, setIsoInfo] = useState<IsoInfo | null>(null);
  const [isoError, setIsoError] = useState<string | null>(null);
  const download = state.virtualbox.installer_download;
  const iso = state.choices?.iso_path ?? null;

  useEffect(() => {
    let off: (() => void) | undefined;
    api.onProgress((e) => e.operation === "download" && setProgress(e)).then((f) => (off = f));
    if (!vbInstalled) api.resolveVirtualBoxRelease().then(setRelease).catch((e) => setError(errorMessage(e)));
    return () => off?.();
  }, [api, vbInstalled]);

  useEffect(() => {
    if (!iso) return;
    api.inspectIso(iso).then(setIsoInfo).catch((e) => setIsoError(errorMessage(e)));
  }, [iso, api]);

  const startDownload = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.downloadVirtualBox();
      await refresh();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  const chooseIso = async (source: "existing" | "microsoft_download_page") => {
    const p = await api.pickIso();
    if (!p || !state.choices) return;
    setError(null);
    try {
      const c = state.choices;
      await api.setChoices({ vm_name: c.vm_name, base_folder: c.base_folder, ram_mb: c.sizing.ram_mb, cpus: c.sizing.cpus, disk_gb: c.sizing.disk_gb, iso_path: p, iso_source: source });
      await refresh();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const isoReady = !!iso && !!isoInfo && isoInfo.looks_like_windows && isoInfo.arch === guestArch;
  const vbReady = vbInstalled || (download?.completed && download.verified);

  return (
    <>
      <h1>Get required files</h1>
      <ErrorBox message={error} />

      <Panel title="1. VirtualBox (free, from Oracle)">
        {vbInstalled ? (
          <OkCard title={`VirtualBox ${report.virtualbox!.version_raw} is already installed`}>
            <p>No download needed.</p>
          </OkCard>
        ) : download?.completed && download.verified ? (
          <OkCard title="VirtualBox installer downloaded and verified">
            <p className="hint">
              <code>{download.path}</code>
              <br />
              SHA-256 matched {download.checksum_source === "OracleSha256Sums" ? "Oracle's published checksum list" : "the checksum built into this app (Oracle's list was not reachable)"}.
            </p>
          </OkCard>
        ) : (
          <>
            <dl className="facts">
              <dt>What</dt>
              <dd>{release ? `VirtualBox ${release.version} for this computer (${release.file_name})` : "Finding the current version…"}</dd>
              <dt>Official source</dt>
              <dd>download.virtualbox.org (Oracle)</dd>
              <dt>Size</dt>
              <dd>Roughly {guestArch === "arm64" ? "150 to 250 MB" : "100 to 400 MB"}; the exact size is shown once the download starts</dd>
              <dt>Verification</dt>
              <dd>
                {release
                  ? release.checksum_source === "oracle_sha256_sums"
                    ? "SHA-256 from Oracle's published SHA256SUMS for this version"
                    : "SHA-256 built into this app for version " + release.version + " (Oracle's checksum site was not reachable right now)"
                  : "SHA-256 checksum"}
              </dd>
            </dl>
            {busy ? (
              <>
                <ProgressView ev={progress} label="Downloading VirtualBox" />
                <div className="actions">
                  <button onClick={() => api.cancelOperation()}>Pause (keeps what was downloaded)</button>
                </div>
              </>
            ) : (
              <div className="actions">
                <button className="primary" onClick={startDownload} disabled={!release}>
                  {download && !download.completed ? "Resume download" : "Download VirtualBox"}
                </button>
                <button
                  onClick={async () => {
                    const p = await api.pickInstaller();
                    if (!p) return;
                    setBusy(true);
                    setError(null);
                    try {
                      await api.useExistingVirtualBoxInstaller(p);
                      await refresh();
                    } catch (e) {
                      setError(errorMessage(e));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  I already downloaded it
                </button>
              </div>
            )}
          </>
        )}
      </Panel>

      <Panel title="2. Windows 11 installation file (ISO, from Microsoft)">
        {isoReady ? (
          <OkCard title="Windows 11 ISO selected">
            <p className="hint">
              <code>{iso}</code> — {isoInfo!.volume_id}, {fmtBytes(isoInfo!.size_bytes)}
              {isoInfo!.language_hint ? `, language ${isoInfo!.language_hint}` : ""}. Checked: {guestArch === "arm64" ? "ARM64" : "x64"} EFI boot files and Windows install image present.
            </p>
            <div className="actions">
              <button onClick={() => chooseIso("existing")}>Choose a different ISO</button>
            </div>
          </OkCard>
        ) : (
          <>
            {isoError && <ErrorBox message={isoError} />}
            <p>
              Microsoft does not offer a download this app can fetch for you automatically, so this is a short hand-off:
            </p>
            <ol>
              <li>
                Click <strong>Open Microsoft's download page</strong>. In your browser, find the section <strong>Download Windows 11 Disk Image (ISO)</strong>
                {guestArch === "arm64" ? " for Arm64-based PCs" : " for x64 devices"}, choose <strong>Windows 11 (multi-edition ISO)</strong>, pick your language, and confirm.
              </li>
              <li>The download is about 5 to 7 GB and lands in your Downloads folder. The link Microsoft gives expires after 24 hours; that is fine.</li>
              <li>
                Come back here and click <strong>Select the downloaded ISO</strong>.
              </li>
            </ol>
            <p className="hint">
              Windows edition (Home, Pro) is chosen later inside Windows Setup. A product key is not required to install; activation is separate.
            </p>
            <div className="actions">
              <button className="primary" onClick={() => api.openOfficialPage(guestArch === "arm64" ? "windows11_arm64" : "windows11_x64")}>
                Open Microsoft's download page
              </button>
              <button onClick={() => chooseIso("microsoft_download_page")}>Select the downloaded ISO</button>
            </div>
          </>
        )}
      </Panel>

      <div className="actions">
        <button className="primary" onClick={onNext} disabled={!vbReady || !isoReady || busy}>
          Continue
        </button>
      </div>
    </>
  );
}
