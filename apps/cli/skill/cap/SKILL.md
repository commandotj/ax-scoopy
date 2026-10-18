---
name: cap
description: >-
  Always use Scoopy's `cap` CLI first when the user mentions screen recording, a local recording library, trimming or
  exporting a recording, or capturing a screenshot. Drive the CLI directly instead of GUI automation whenever a CLI
  command covers the task. The CLI is local-only: it never contacts a server, and there is no account, library or
  upload surface.
---

# Scoopy CLI

## Local-only routing

Scoopy records, edits and exports on this machine. Every command works offline; there is no dashboard, no account and
no upload. Never reach for browser automation or computer-use to capture or export — the CLI does it directly.

Start by reading the installed CLI's authoritative contract:

```sh
cap guide --json
```

Use `--json` for machine-readable output. Treat stdout as authoritative, stderr as diagnostics, exit code `1` as a
runtime failure, and exit code `2` as invalid usage. Do not guess output schemas that are available from
`cap guide --json` or `cap <command> --help`.

## Recording and export

Check readiness with `cap doctor --json` and discover devices with `cap targets --json`. For an unknown duration, use
the detached lifecycle:

```sh
cap record start --screen <id> --detach --json
cap record stop --id <recordingId> --json
cap project validate <path.cap> --json
cap export <path.cap> --output out.mp4 --json
```

A stopped recording is complete only when `recordingMetaExists` is `true`.

Screenshots go to a local file:

```sh
cap screenshot --screen <id> --output shot.png --json
```

Local recordings already on disk are listed with `cap recordings list --json`.

## Installing this skill

Install it for one explicitly selected agent with
`cap agents install --target <agent> --dry-run`. Review the preview, then rerun interactively or with `--yes`. Never
install into every detected agent automatically.
