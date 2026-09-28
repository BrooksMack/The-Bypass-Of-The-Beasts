# Contributing

## Development setup

Prerequisites: Node.js 22, Rust stable (1.88+), and the Tauri platform prerequisites
(Xcode command-line tools on macOS; Visual Studio Build Tools with the C++ workload and WebView2 on Windows;
`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev` on Ubuntu).

```bash
npm ci                     # frontend dependencies
npm run typecheck          # TypeScript
npm test                   # UI workflow tests against the labelled mock backend
cargo test --workspace     # Rust core + app crate tests (no VirtualBox needed)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
npm run tauri dev          # run the desktop app (needs VirtualBox for the real flow)
npm run dev                # frontend only in a browser with the MOCK backend
npm run tauri build        # build the installer for this machine
```

End users never need any of this; they download installers from Releases.

## Layout

- `crates/vmsa-core/`: all logic, no GUI. Add tests here for anything that could break an install.
- `src-tauri/`: Tauri commands (`commands.rs`), orchestration (`setup.rs`), app wiring (`lib.rs`).
- `src/`: React screens (`screens/`), shared components, `lib/api.ts` (backend bridge) and `lib/mock.ts`.
- `tests/fixtures/vboxmanage/`: sanitized VBoxManage output used by parser tests.
- `docs/`: user and maintainer documentation; keep `docs/HANDOFF.md` current.

## Rules of the road

- Never interpolate user input into a shell string; pass argument arrays.
- Never delete or reconfigure a VM the app did not create; record the original configuration before changing one it did.
- Never collect the user's Windows password or PIN.
- Do not describe anything as verified unless the code observed it or the user confirmed it.
- Keep secrets, personal paths, ISOs, installers and VM files out of the repository (see `.gitignore`).
- Update `docs/COMPATIBILITY.md` only with results you actually observed, with date and versions.

## Pull requests

Run the checks above. Describe what you tested and on which hardware. CI builds on Windows, macOS
(Apple Silicon and Intel) and Linux but cannot run a VM; say so in the PR if your change needs a manual test.
