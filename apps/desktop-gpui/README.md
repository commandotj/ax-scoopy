# Scoopy desktop (gpui)

Scoopy's desktop app, drawn entirely by [gpui](https://www.gpui.rs/). No webview; every pixel is
Rust. Local only: no account, no server, no network.

The main recording window uses a fixed 330×432 layout with device controls and recording and
screenshot browsers.

## Running it

```sh
cd apps/desktop-gpui
cargo run
```

`RUST_LOG=cap_gpui=debug cargo run` for logs — note the filter is `cap_gpui`
(the binary), not `cap_desktop_gpui` (the package).

The first build takes about 11 minutes and 6.4 GB of `target/`: gpui pulls its
own revisions of the wgpu and font stacks. After that, an edit rebuilds only
the app crate, at `opt-level = 0` and incrementally, in about 22s with plain
`cargo build` or the dev loop below; the app crate's incremental cache is about
1.6 GB of `target/debug/incremental`. Dependencies build at `opt-level = 2`.
The app crate and its dependencies carry line-table debug info, so breakpoints
and backtraces keep their lines; for locals in a debugger, build with
`--config 'profile.dev.package.cap-desktop-gpui.debug=true'`.

Loops over pixels or samples live in `kernels/` (`cap-gpui-kernels`,
`opt-level = 3`): unoptimised, the editor's frame conversion alone costs 42.7ms
a frame against 1.0ms, a 23fps preview instead of a 60fps one. That includes
generic dependency calls over large data — `imageops::resize`, image encoders,
`serde_json::from_reader`, `md5` — because a generic compiles into the crate
that instantiates it, at that crate's opt-level. Put the call in `kernels/`
behind a plain-slice function and call that from the app.

### The dev loop

```sh
./dev.sh
```

`bun run dev:desktop` (from the repo root) starts this loop automatically next
to the Tauri app, output prefixed `[gpui]`; set `CAP_GPUI_DEV=0` to opt out
(`scripts/dev-desktop.mjs`).

Native Rust cannot hot-reload, so this is the Solid-dev-server feel rebuilt
from its observable parts. The script watches the app's sources, the `cap-*`
crates the editor depends on and the local gpui checkout, rebuilds on save,
and swaps the running app **only when the build succeeds** — a compile error
leaves the previous build running, the way Vite keeps the page alive under an
error overlay. The relaunch reopens exactly where the old process was: same
windows at the same frames, same settings page, same editor project with its
sidebar tab and scroll (`src/dev_restore.rs`, driven by
`CAP_GPUI_DEV_RESTORE`, applied through the same entry points real clicks
take). Relaunches deliberately do not activate the app, so keyboard focus
stays in your code editor while the windows refresh beside it; and the swap
waits out an in-flight recording rather than truncating it (the state file
doubles as that handshake — `dev.sh` will not kill the app while it reads
`"recording":true`).

The loop runs a plain `cargo build`, so it shares every artifact with one run
by hand: switching between `./dev.sh` and `cargo build` rebuilds nothing.

### Why it is a separate workspace

`Cargo.toml` declares its own `[workspace]`. The root workspace carries
`[patch.crates-io]` entries — a vendored `wgpu-hal`, a forked `tao` — that exist
only for the Tauri app and that gpui's tree does not want. Nesting a second
workspace here leaves the root `Cargo.lock`, the `cargo hakari` workspace-hack
and CI completely untouched.

`rust-toolchain.toml` pins `stable` rather than the root's 1.88.0, because
gpui's dependencies need 1.89+ (`smol_str` 0.3.6, `cosmic-text` 0.19). The
nearest toolchain file wins, so the Tauri app is unaffected.
