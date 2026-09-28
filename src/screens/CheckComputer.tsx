import { useEffect, useState } from "react";
import { fmtBytes, errorMessage, type Api } from "../lib/api";
import type { HostReport } from "../lib/types";
import { ErrorBox, FindingCard, OkCard, Panel } from "../components/ui";

const ARCH: Record<string, string> = { x86_64: "Intel/AMD 64-bit (x64)", aarch64: "ARM 64-bit (Apple Silicon / Arm64)", other: "Unknown" };
const OS: Record<string, string> = { windows: "Windows", mac_os: "macOS", linux: "Linux", other: "Unknown" };
const TRI: Record<string, string> = { yes: "Available", no: "Not available", unknown: "Could not be determined" };

export function CheckComputer({ api, report, setReport, onNext }: { api: Api; report: HostReport | null; setReport: (r: HostReport) => void; onNext: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      setReport(await api.inspectHost());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  useEffect(() => {
    if (!report) void run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const blockers = report?.assessment.findings.filter((f) => f.severity === "blocker") ?? [];
  const others = report?.assessment.findings.filter((f) => f.severity !== "blocker") ?? [];
  const vb = report?.virtualbox ?? null;

  return (
    <>
      <h1>Check this computer</h1>
      <ErrorBox message={error} />
      {busy && !report && <p aria-live="polite">Looking at this computer…</p>}
      {report && (
        <>
          <Panel title="What was found">
            <dl className="facts">
              <dt>Operating system</dt>
              <dd>
                {OS[report.host.os]} {report.host.os_version}
              </dd>
              <dt>Processor</dt>
              <dd>
                {ARCH[report.host.arch]}
                {report.host.cpu_brand ? ` (${report.host.cpu_brand})` : ""}
                {report.host.translated ? " — this app is running in translation mode" : ""}
              </dd>
              <dt>Memory</dt>
              <dd>
                {fmtBytes(report.host.total_ram_bytes)} total, {fmtBytes(report.host.available_ram_bytes)} currently free
              </dd>
              <dt>Processors (logical)</dt>
              <dd>{report.host.logical_cpus}</dd>
              <dt>Free disk space</dt>
              <dd>
                {report.disk_at_base ? `${fmtBytes(report.disk_at_base.available_bytes)} free at ${report.default_base_folder}` : "Unknown"}
                {report.storage_estimate && ` — about ${fmtBytes(report.storage_estimate.total_bytes)} needed for setup`}
              </dd>
              <dt>Hardware virtualization</dt>
              <dd>{TRI[report.host.virtualization]}</dd>
              <dt>Other virtualization features</dt>
              <dd>{report.host.hypervisor_conflict === "yes" ? report.host.hypervisor_conflict_detail ?? "Active" : report.host.hypervisor_conflict === "no" ? "None detected" : "Unknown"}</dd>
              <dt>VirtualBox</dt>
              <dd>
                {vb
                  ? `Installed: version ${vb.version_raw}${report.virtualbox_version_ok === false ? ` (too old; ${report.virtualbox_min_version} or newer is needed)` : ""}`
                  : report.virtualbox_error
                    ? `Found but not working: ${report.virtualbox_error}`
                    : "Not installed (the assistant will help you install it)"}
              </dd>
              <dt>Existing virtual machines</dt>
              <dd>
                {report.existing_vms.length === 0
                  ? "None"
                  : report.existing_vms.map((v) => `${v.name}${v.created_by_app ? " (created by this app)" : ""}`).join(", ")}
              </dd>
              <dt>Windows to install</dt>
              <dd>{report.guest_arch === "arm64" ? "Windows 11 on ARM (ARM64)" : report.guest_arch === "x64" ? "Windows 11 (x64)" : "Not supported on this computer"}</dd>
            </dl>
          </Panel>
          <Panel title="Result">
            {blockers.length === 0 && <OkCard title="This computer can run a Windows 11 virtual machine">{others.length > 0 && <p>There are some recommendations below, but nothing stops you from continuing.</p>}</OkCard>}
            {blockers.map((f) => (
              <FindingCard key={f.code} f={f} />
            ))}
            {others.map((f) => (
              <FindingCard key={f.code} f={f} />
            ))}
            {report.assessment.support_level === "untested" && blockers.length === 0 && (
              <p className="hint">
                Support status for this computer type: implemented from Oracle's documentation but not yet verified end-to-end by
                this project. See the compatibility matrix in the project documentation.
              </p>
            )}
          </Panel>
          <div className="actions">
            <button onClick={run} disabled={busy}>
              {busy ? "Checking…" : "Check again"}
            </button>
            <button className="primary" onClick={onNext} disabled={busy || blockers.length > 0}>
              Continue
            </button>
          </div>
        </>
      )}
    </>
  );
}
