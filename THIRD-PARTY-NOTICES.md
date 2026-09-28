# Third-party notices

VM Setup Assistant is MIT-licensed. It coordinates the installation of separately licensed products
that it does **not** redistribute:

- **Oracle VirtualBox** (GPL v3 for the base package) is downloaded by the user from
  https://www.virtualbox.org/ and installed with Oracle's own installer. The optional VirtualBox Extension
  Pack is under Oracle's separate Personal Use and Evaluation License (PUEL) and is not used or installed
  by this app.
- **Microsoft Windows 11** installation media is downloaded by the user from Microsoft's official page.
  Windows requires a license from Microsoft; this app includes no media, images or product keys.
- **virtio-win drivers** (Red Hat / Fedora project) may be obtained by the user from the official
  virtio-win project for the documented ARM network-recovery scenario. They are not bundled.

## Bundled open-source components

The application binaries include the following open-source libraries (non-exhaustive; the SBOM attached to
each release lists every crate and npm package with its license):

- Tauri and its plugins (dialog, opener, single-instance, log): MIT / Apache-2.0
- Rust crates: serde, tokio, reqwest, rustls, sha2, regex, sysinfo, directories, uuid, chrono, thiserror,
  which, tempfile, hex, futures, tracing (MIT / Apache-2.0); ring (ISC / MIT-like)
- React and react-dom: MIT
- Windows builds bundle the Microsoft Edge WebView2 bootstrapper (Microsoft's WebView2 license).

Copies of the licenses are included in the release SBOM references and in the respective packages.
