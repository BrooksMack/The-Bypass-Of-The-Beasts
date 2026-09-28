import type { ReactNode } from "react";
import type { Finding, ProgressEvent, Verified } from "../lib/types";
import { fmtBytes } from "../lib/api";

export function Panel({ title, children, subtle }: { title?: string; children: ReactNode; subtle?: boolean }) {
  return (
    <section className={subtle ? "panel subtle" : "panel"}>
      {title && <h2>{title}</h2>}
      {children}
    </section>
  );
}

export function FindingCard({ f }: { f: Finding }) {
  const label = f.severity === "blocker" ? "Needs attention before continuing" : f.severity === "warning" ? "Recommendation" : "Note";
  return (
    <div className={`finding ${f.severity}`} role={f.severity === "blocker" ? "alert" : undefined}>
      <span className="hint">{label}</span>
      <h3>{f.title}</h3>
      <p>{f.detail}</p>
      {f.action && <p className="action">What to do: {f.action}</p>}
    </div>
  );
}

export function OkCard({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="finding ok">
      <h3>{title}</h3>
      {children}
    </div>
  );
}

export function Badge({ v, labels }: { v: Verified; labels?: Partial<Record<Verified, string>> }) {
  const text = labels?.[v] ?? (v === "yes" ? "Verified" : v === "no" ? "Not yet" : "Pending");
  return <span className={`badge ${v}`}>{text}</span>;
}

export function ErrorBox({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <div className="error-box" role="alert">
      {message}
    </div>
  );
}

export function ProgressView({ ev, label }: { ev: ProgressEvent | null; label?: string }) {
  if (!ev) return null;
  const hasBytes = ev.bytes_total != null && ev.bytes_total > 0 && ev.bytes_done != null;
  const pct = hasBytes ? Math.min(100, (100 * (ev.bytes_done as number)) / (ev.bytes_total as number)) : null;
  const stepPct = ev.step_count && ev.step_index != null ? Math.round((100 * ev.step_index) / ev.step_count) : null;
  return (
    <div aria-live="polite">
      <p>
        <strong>{label ?? ev.step}</strong>
        {label && <span> — {ev.step}</span>}
      </p>
      {ev.detail && <p className="hint">{ev.detail}</p>}
      {pct != null ? (
        <>
          <div className="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
            <div style={{ width: `${pct}%` }} />
          </div>
          <p className="hint">
            {fmtBytes(ev.bytes_done)} of {fmtBytes(ev.bytes_total)} ({Math.round(pct)}%)
            {ev.bytes_per_sec != null && ev.bytes_per_sec > 0 ? ` at ${fmtBytes(ev.bytes_per_sec)}/s` : ""}
          </p>
        </>
      ) : stepPct != null ? (
        <>
          <div className="progress" role="progressbar" aria-valuenow={stepPct} aria-valuemin={0} aria-valuemax={100}>
            <div style={{ width: `${stepPct}%` }} />
          </div>
          <p className="hint">
            Step {ev.step_index} of {ev.step_count}
          </p>
        </>
      ) : (
        <>
          <div className="progress indeterminate" role="progressbar" aria-busy="true">
            <div />
          </div>
          <p className="hint">Progress for this step cannot be measured; it is still working.</p>
        </>
      )}
    </div>
  );
}

export function NextPanel({ title = "What to do next", children }: { title?: string; children: ReactNode }) {
  return (
    <aside className="panel next-panel" aria-label={title}>
      <h2>{title}</h2>
      {children}
    </aside>
  );
}
