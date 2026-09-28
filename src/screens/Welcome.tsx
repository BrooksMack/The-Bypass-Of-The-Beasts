import { Panel } from "../components/ui";

export function Welcome({ onStart, hasProgress, onResume }: { onStart: () => void; hasProgress: boolean; onResume: () => void }) {
  return (
    <>
      <h1>Set up Windows on this computer</h1>
      <Panel>
        <p>
          This assistant creates a <strong>separate Windows 11 computer that runs inside your current computer</strong> (a
          "virtual machine"). Your files and your current system stay as they are. Windows runs in its own window that you
          can open, use, and close like any other app.
        </p>
        <p>It uses Oracle VirtualBox, a free program that does the actual work. The assistant guides you through installing it.</p>
        <h3>Before you start</h3>
        <ul>
          <li>
            <strong>Windows licensing is separate.</strong> Windows 11 installs and runs without a product key, but stays
            unactivated (a watermark and some personalization limits) until you supply a valid license from Microsoft.
          </li>
          <li>
            <strong>Large downloads and disk space.</strong> Expect about 6 GB of Windows download, a small VirtualBox
            download, and 30 to 60 GB of disk space as Windows installs and updates. The assistant checks your free space
            before each big step.
          </li>
          <li>
            <strong>Some steps happen in Windows itself.</strong> When Windows asks you to accept its license, sign in, or
            create a PIN, you type those directly into Windows, never into this assistant.
          </li>
        </ul>
        <div className="actions">
          {hasProgress ? (
            <>
              <button className="primary" onClick={onResume} autoFocus>
                Continue where I left off
              </button>
              <button onClick={onStart}>Start over (keeps any existing VM)</button>
            </>
          ) : (
            <button className="primary" onClick={onStart} autoFocus>
              Set up Windows
            </button>
          )}
        </div>
      </Panel>
    </>
  );
}
