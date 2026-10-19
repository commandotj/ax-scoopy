# RFC 0001: Remove all remote features (Umbrella)

**Status:** Implemented

**Type:** Umbrella

## Summary

Ax-scoopy becomes a local-only recorder and editor. Everything that talks to a remote service, runs remote compute, or exists to serve remote users is removed. Each concern is a child RFC.

## Problem

The fork inherits Scoopy's cloud stack: sign-in, upload and share links, a web app and database, a render farm, and satellite clients. None of it is needed for a local app, and it enlarges build, review and security surface.

## Goals

- All children Implemented.
- `apps/desktop-gpui` builds and runs with no network dependency for recording, editing and export.
- No workspace member exists only to serve a removed remote feature.

## Non-goals

- Removing local media, recording, rendering or export crates.
- Re-adding any remote feature under this umbrella. New remote work needs a new RFC.

## Children

| RFC | Concern |
|-----|---------|
| [0007](0007-remove-cli-remote.md) | Remove CLI remote auth |
| [0002](0002-remove-desktop-cloud-client.md) | Remove desktop cloud client |
| [0003](0003-remove-server-side.md) | Remove server side |
| [0004](0004-remove-remote-compute.md) | Remove remote compute (render-farm) |
| [0005](0005-remove-other-remote-clients.md) | Remove mobile, chrome-extension, status apps |

## Acceptance

Close when 0002-0005 are Implemented. Never reopen for new work.
