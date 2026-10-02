import { useState } from "react";
import { errorMessage, type Api } from "../lib/api";
import { ErrorBox, Panel } from "./ui";

export function MediaDevices({ api, running, isMac }: { api: Api; running: boolean; isMac: boolean }) {
  const [cameras, setCameras] = useState<{ alias: string; name: string }[] | null>(null);
  const [alias, setAlias] = useState(".0");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  async function act(fn: () => Promise<string>) {
    setBusy(true);
    setError(null);
    setMessage(null);
    try { setMessage(await fn()); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  }
  return <Panel title="Camera and microphone">
    <ErrorBox message={error} />
    <p>Connect devices when you need them. A successful setting change is not a recording test.</p>
    <div className="actions">
      <button disabled={busy} onClick={() => act(() => api.mediaAction({ kind: "microphone", enabled: true }))}>Enable microphone</button>
      <button disabled={busy} onClick={() => act(() => api.mediaAction({ kind: "microphone", enabled: false }))}>Disable microphone</button>
      <button disabled={busy} onClick={() => act(async () => {
        const devices = await api.listCameras();
        setCameras(devices);
        setAlias(devices[0]?.alias ?? ".0");
        return devices.length ? "Choose a camera, then connect it while Windows is running." : "No cameras were reported. Check host camera permissions, close other camera apps, then try again.";
      })}>Find cameras</button>
    </div>
    <label htmlFor="camera-device">Camera</label>
    <select id="camera-device" value={alias} disabled={busy} onChange={(e) => setAlias(e.target.value)}>
      <option value=".0">Default camera</option>
      {cameras?.filter((c) => c.alias !== ".0").map((c) => <option key={c.alias} value={c.alias}>{c.name}</option>)}
    </select>
    <div className="actions">
      <button disabled={busy || !running} onClick={() => act(() => api.mediaAction({ kind: "attach_camera", alias }))}>Connect camera</button>
      <button disabled={busy || !running} onClick={() => act(() => api.mediaAction({ kind: "detach_camera", alias }))}>Disconnect camera</button>
    </div>
    {!running && <p className="hint">Start Windows before connecting a camera. Resume or shut down a saved VM before changing microphone settings.</p>}
    {message && <p role="status">{message}</p>}
    {isMac && <p>On your Mac, open System Settings › Privacy &amp; Security › Camera and Microphone, and allow VirtualBox when prompted. Choose the microphone in System Settings › Sound › Input. Close other apps using the camera. Restart the VM after changing Mac permissions if necessary.</p>}
    <p>In Windows, allow camera and microphone access in Settings › Privacy &amp; security. Test the Camera app and record/play back a short clip in Sound Recorder before using a calling app. Reconnect the camera after restarting Windows if needed.</p>
    <details>
      <summary>Display and privacy</summary>
      <p>Keep Guest Additions installed for smooth mouse movement and automatic screen resizing. New VMs disable shared clipboard and drag-and-drop, and do not share host folders. These settings reduce host data exposure; they do not conceal virtual hardware from software inside Windows.</p>
      <p>For a clean view, use View › Full-screen Mode in the Windows VM window. When sharing your screen, share only the Windows application you intend to show. Eject installation discs after setup from the settings screen.</p>
    </details>
  </Panel>;
}
