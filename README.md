<p align="center">
	<img width="150" height="150" src="apps/desktop-gpui/assets/dock-icon.png" alt="Scoopy logo">
</p>

<h1 align="center">Scoopy</h1>

<p align="center">
	Local-first, local-only screen recorder and editor.<br>No account, no server, no upload, no network.
</p>

<img src="apps/desktop-gpui/assets/images/light.jpg" alt="Scoopy app preview">

Scoopy records your screen, camera and microphone, and edits the result into a finished video
file on your own machine. It is local-first and local-only: no server, no remote service, and
everything stays on your disk.

Use it for product demos, bug reports, onboarding, tutorials, design reviews, engineering
walkthroughs, async standups, or any moment where showing the work beats scheduling another call.

## Why Scoopy

- **Local only, by construction.** There is no Scoopy server, no remote service, no account,
  no telemetry and no subscription. The app makes no network requests at runtime, so it works
  on a plane.
- **Your files, your disk.** Recordings, projects and exports stay on your machine. No sync,
  no share links, no cloud.
- **Record, edit, export.** Capture screen, camera and microphone, then trim, add zoom,
  backgrounds and captions in the editor, and export a finished file.
- **Studio and Instant modes.** Studio records in full quality and opens the editor; Instant
  records in a single pass and finishes the moment you stop.
- **Screenshots with annotation.** Capture a screen or window, draw on it, add a background,
  and save it.
- **A CLI for agents.** `apps/cli` exposes the same record → export flow with `--json` output
  for scripting and automation.

## Recording Modes

| Mode | Best for | How it works |
| --- | --- | --- |
| Instant Mode | Fast feedback, bug reports, async updates | Records in one pass and finalizes as soon as you stop. |
| Studio Mode | Product demos, tutorials, launches, client work | Records locally in full quality, opens the editor, exports a polished video. |

## Data Ownership

Recordings live on your machine, in Scoopy's local library. Nothing is uploaded, because there
is no server to upload to and no remote feature in the code. Export writes a file wherever you
point it.

## Get Started

1. Build or download the app for macOS.
2. Grant Screen Recording (and Microphone, if you use audio) permission when macOS asks.
3. Pick a mode, record, and open the editor to export.

## Local Development

Scoopy is a Turborepo monorepo with Rust media crates, a GPUI desktop app, a Rust CLI, and a
Tauri app kept as a development harness.

Requirements:

- Node.js 20 or newer
- Bun 1.4.0
- Rust 1.88 or newer

Install and set up the repo:

```bash
bun install
bun run scoopy-setup
```

The GPUI app patches `gpui` to a local checkout of a pinned upstream revision. Restore it once
per clone with:

```bash
./scripts/prepare-gpui-dependency.sh
```

Common commands:

| Command | Purpose |
| --- | --- |
| `bun run dev` | Start the workspace dev tasks |
| `bun run dev:desktop` | Start the desktop app |
| `bun run build` | Build the workspace |
| `bun run gpui:build <target>` | Bundle the GPUI desktop release (the shipping app) |
| `bun run tauri:build` | Build the Tauri harness release |
| `bun run lint` | Run Biome linting |
| `bun run format` | Format with Biome |
| `bun run typecheck` | Run TypeScript project references |
| `cargo test -p <crate>` | Run Rust tests for a crate |

Rust builds need FFmpeg headers. On macOS with Homebrew:

```bash
PKG_CONFIG_PATH="$(brew --prefix ffmpeg@6)/lib/pkgconfig" cargo check -p scoopy
```

`apps/desktop-gpui` is its own Cargo workspace; build it with
`cargo build --manifest-path apps/desktop-gpui/Cargo.toml --bin scoopy-gpui`.

## Repository Map

| Path | What lives there |
| --- | --- |
| `apps/desktop-gpui` | GPUI desktop app — the app Scoopy ships |
| `apps/desktop` | Tauri v2 desktop app kept as a development harness |
| `apps/cli` | Rust CLI for recording, export and agent automation |
| `packages/ui`, `packages/ui-solid`, `packages/utils` | Shared UI and utility packages |
| `crates/*` | Capture, camera, audio, encoding, rendering, muxing, export and test crates |
| `scripts/*` | Build, packaging, verification and maintenance tooling |

Desktop capture and export are backed by Rust crates for fast recording, rendering and
platform-specific media access.

## Deep Links

Scoopy handles `scoopy://action?value=<json>` URLs for automation and scripting — see
`apps/desktop-gpui/src/deeplink.rs` for the action grammar.

## License

Released under the MIT License. See [LICENSE](LICENSE). Third-party components keep the
license provided by their owner.
