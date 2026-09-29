import { useEffect, useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import {
  ADAPTER_MODELS,
  PARAVIRT_PROVIDERS,
  defaultIdentityConfig,
  type IdentityConfig,
  type IdentityPreview,
  type NetworkModeCfg,
  type SetupState,
} from "../lib/types";
import { ErrorBox, Panel } from "../components/ui";

/** Trim to null so empty inputs clear the identifier instead of sending "". */
function orNull(v: string): string | null {
  const t = v.trim();
  return t === "" ? null : t;
}

function TextRow({
  label,
  value,
  placeholder,
  hint,
  onChange,
}: {
  label: string;
  value: string | null;
  placeholder?: string;
  hint?: string;
  onChange: (v: string | null) => void;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <input type="text" value={value ?? ""} placeholder={placeholder} onChange={(e) => onChange(orNull(e.target.value))} />
      {hint && <small className="hint">{hint}</small>}
    </label>
  );
}

/**
 * Guest-visible VM identity for compatibility testing. Off by default. Every change is reversible
 * and the panel always shows what these settings cannot hide.
 */
export function Identity({ api, state, onChanged }: { api: Api; state: SetupState; onChanged: () => Promise<void> }) {
  const [cfg, setCfg] = useState<IdentityConfig>(state.choices?.identity ?? defaultIdentityConfig());
  const [preview, setPreview] = useState<IdentityPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const hasVm = !!state.vm;
  const applied = !!state.vm?.identity_applied;

  useEffect(() => {
    let live = true;
    api
      .getIdentityConfig()
      .then((c) => live && setCfg(c))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [api]);

  useEffect(() => {
    let live = true;
    api
      .previewIdentityConfig(cfg)
      .then((p) => live && setPreview(p))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [api, cfg]);

  const patch = (f: (c: IdentityConfig) => IdentityConfig) => setCfg((c) => f(structuredClone(c)));

  const run = async (fn: () => Promise<string>) => {
    setBusy(true);
    setError(null);
    setMsg(null);
    try {
      setMsg(await fn());
      await onChanged();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const save = () =>
    run(async () => {
      await api.setIdentityConfig(cfg);
      return "Identity configuration saved. It applies when the VM is created, or use Apply now below.";
    });

  const netMode = cfg.network_mode.mode;
  const setNetMode = (mode: NetworkModeCfg["mode"]) =>
    patch((c) => {
      c.network_mode =
        mode === "nat"
          ? { mode: "nat" }
          : mode === "nat_network"
          ? { mode: "nat_network", name: "" }
          : mode === "bridged"
          ? { mode: "bridged", host_adapter: "" }
          : { mode: "host_only", host_adapter: "" };
      return c;
    });

  return (
    <>
      <Panel title="VM identity (compatibility testing)">
        <p>
          Some Windows software refuses to run, or behaves differently, inside a virtual machine. These optional settings change the
          hardware identifiers the guest reads, so a VM can be tested as a specific machine. They use only Oracle's documented
          VBoxManage settings, are off unless you turn them on, and can be reverted.
        </p>
        <p className="hint">This cannot make a VM undetectable. See "What this cannot hide" below and the identity checklist in the docs.</p>

        <label className="field toggle">
          <input type="checkbox" checked={cfg.enabled} onChange={(e) => patch((c) => ((c.enabled = e.target.checked), c))} />
          <span>Enable VM identity configuration</span>
        </label>

        {cfg.enabled && (
          <>
            <fieldset>
              <legend>Firmware (BIOS)</legend>
              <TextRow label="BIOS vendor" value={cfg.firmware.bios_vendor} placeholder="American Megatrends Inc." onChange={(v) => patch((c) => ((c.firmware.bios_vendor = v), c))} />
              <TextRow label="BIOS version" value={cfg.firmware.bios_version} placeholder="1503" onChange={(v) => patch((c) => ((c.firmware.bios_version = v), c))} />
              <TextRow label="BIOS release date" value={cfg.firmware.bios_release_date} placeholder="04/12/2023" onChange={(v) => patch((c) => ((c.firmware.bios_release_date = v), c))} />
            </fieldset>

            <fieldset>
              <legend>System, mainboard and chassis</legend>
              <TextRow label="Manufacturer" value={cfg.system.manufacturer} placeholder="Dell Inc." onChange={(v) => patch((c) => ((c.system.manufacturer = v), c))} />
              <TextRow label="Product name" value={cfg.system.product_name} placeholder="OptiPlex 7090" onChange={(v) => patch((c) => ((c.system.product_name = v), c))} />
              <TextRow label="Version" value={cfg.system.version} onChange={(v) => patch((c) => ((c.system.version = v), c))} />
              <TextRow label="Serial number" value={cfg.system.serial_number} onChange={(v) => patch((c) => ((c.system.serial_number = v), c))} />
              <TextRow label="SKU" value={cfg.system.sku} onChange={(v) => patch((c) => ((c.system.sku = v), c))} />
              <TextRow label="Family" value={cfg.system.family} onChange={(v) => patch((c) => ((c.system.family = v), c))} />
              <TextRow label="System UUID" value={cfg.system.uuid} placeholder="12345678-1234-1234-1234-1234567890ab" hint="Also sets the VM's hardware UUID to match." onChange={(v) => patch((c) => ((c.system.uuid = v), c))} />
              <TextRow label="Board manufacturer" value={cfg.system.board_manufacturer} onChange={(v) => patch((c) => ((c.system.board_manufacturer = v), c))} />
              <TextRow label="Board product" value={cfg.system.board_product} onChange={(v) => patch((c) => ((c.system.board_product = v), c))} />
              <TextRow label="Board serial" value={cfg.system.board_serial} onChange={(v) => patch((c) => ((c.system.board_serial = v), c))} />
              <TextRow label="Chassis manufacturer" value={cfg.system.chassis_manufacturer} onChange={(v) => patch((c) => ((c.system.chassis_manufacturer = v), c))} />
              <TextRow label="Chassis asset tag" value={cfg.system.chassis_asset_tag} onChange={(v) => patch((c) => ((c.system.chassis_asset_tag = v), c))} />
            </fieldset>

            <fieldset>
              <legend>Storage (virtual disk)</legend>
              <TextRow label="Disk serial" value={cfg.storage.disk_serial} hint="Up to 20 characters." onChange={(v) => patch((c) => ((c.storage.disk_serial = v), c))} />
              <TextRow label="Disk model" value={cfg.storage.disk_model} placeholder="Samsung SSD 970 EVO Plus" hint="Up to 40 characters." onChange={(v) => patch((c) => ((c.storage.disk_model = v), c))} />
              <TextRow label="Firmware revision" value={cfg.storage.disk_firmware_revision} hint="Up to 8 characters." onChange={(v) => patch((c) => ((c.storage.disk_firmware_revision = v), c))} />
            </fieldset>

            <fieldset>
              <legend>Network adapter</legend>
              <TextRow label="MAC address" value={cfg.mac_address} placeholder="080027AABBCC" hint="12 hex digits. Changes the vendor prefix VirtualBox otherwise reveals." onChange={(v) => patch((c) => ((c.mac_address = v), c))} />
              <label className="field">
                <span>Adapter model</span>
                <select value={cfg.adapter_model ?? ""} onChange={(e) => patch((c) => ((c.adapter_model = e.target.value === "" ? null : e.target.value), c))}>
                  <option value="">Keep the profile default</option>
                  {ADAPTER_MODELS.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Network mode</span>
                <select value={netMode} onChange={(e) => setNetMode(e.target.value as NetworkModeCfg["mode"])}>
                  <option value="nat">NAT (default)</option>
                  <option value="nat_network">NAT network</option>
                  <option value="bridged">Bridged</option>
                  <option value="host_only">Host-only</option>
                </select>
              </label>
              {cfg.network_mode.mode === "nat_network" && (
                <TextRow label="NAT network name" value={cfg.network_mode.name} onChange={(v) => patch((c) => ((c.network_mode = { mode: "nat_network", name: v ?? "" }), c))} />
              )}
              {cfg.network_mode.mode === "bridged" && (
                <TextRow label="Host adapter to bridge onto" value={cfg.network_mode.host_adapter} placeholder="en0 / Ethernet" onChange={(v) => patch((c) => ((c.network_mode = { mode: "bridged", host_adapter: v ?? "" }), c))} />
              )}
              {cfg.network_mode.mode === "host_only" && (
                <TextRow label="Host-only adapter" value={cfg.network_mode.host_adapter} placeholder="vboxnet0" onChange={(v) => patch((c) => ((c.network_mode = { mode: "host_only", host_adapter: v ?? "" }), c))} />
              )}
              {preview && <p className="hint">{preview.connectivity_note}</p>}
            </fieldset>

            <fieldset>
              <legend>Branding reduction</legend>
              <label className="field">
                <span>Paravirtualization interface</span>
                <select value={cfg.branding.paravirt_provider ?? ""} onChange={(e) => patch((c) => ((c.branding.paravirt_provider = e.target.value === "" ? null : e.target.value), c))}>
                  <option value="">Keep the profile default</option>
                  {PARAVIRT_PROVIDERS.map((p) => (
                    <option key={p} value={p}>
                      {p}
                    </option>
                  ))}
                </select>
                <small className="hint">"none" removes the hypervisor CPUID hint but disables paravirtual time sync and can slow the VM.</small>
              </label>
              <label className="field toggle">
                <input type="checkbox" checked={cfg.branding.clear_vbox_oem_strings} onChange={(e) => patch((c) => ((c.branding.clear_vbox_oem_strings = e.target.checked), c))} />
                <span>Neutralise the VirtualBox SMBIOS OEM strings (vboxVer_/vboxRev_)</span>
              </label>
            </fieldset>
          </>
        )}

        <ErrorBox message={error} />
        {msg && (
          <p className="notice" role="status">
            {msg}
          </p>
        )}
        <div className="actions">
          <button className="primary" onClick={save} disabled={busy}>
            Save identity settings
          </button>
          {hasVm && (
            <>
              <button onClick={() => run(api.applyIdentityConfig)} disabled={busy} title="The VM must be powered off.">
                Apply to this VM now
              </button>
              <button className="danger" onClick={() => run(api.revertIdentityConfig)} disabled={busy || !applied} title="The VM must be powered off.">
                Revert identity changes
              </button>
            </>
          )}
        </div>
        {hasVm && <p className="hint">Apply and revert require the VM to be shut down. Applied: {applied ? "yes" : "no"}.</p>}
      </Panel>

      {preview && (
        <>
          <Panel title="What these settings do" subtle>
            {preview.effects.length === 0 ? (
              <p>No changes are configured yet. Turn on the feature and set at least one value.</p>
            ) : (
              <ul>
                {preview.effects.map((e, i) => (
                  <li key={i}>
                    <strong>{e.area}:</strong> {e.change} <span className="hint">Visible via: {e.visible_as}</span>
                  </li>
                ))}
              </ul>
            )}
            <p className="hint">{preview.command_count} VBoxManage change(s) will be issued.</p>
          </Panel>
          <Panel title="What this cannot hide" subtle>
            <ul>
              {preview.remaining_indicators.map((r, i) => (
                <li key={i}>{r}</li>
              ))}
            </ul>
          </Panel>
        </>
      )}
    </>
  );
}
