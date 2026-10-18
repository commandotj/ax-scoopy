# RFC 0003: Remove server side

**Status:** Implemented

**Parent:** [0001](0001-remove-remote-features.md)

## Summary

Delete the hosted backend: `apps/web`, `apps/discord-bot`, `apps/media-server`, `packages/database`, `packages/web-*`, `infra/`, docker compose files, SAML SSO doc.

## Problem

These exist only to serve the cloud client removed in 0002.

## Goals

- Remove the apps and packages listed.
- Remove docker, coolify and env-setup tooling that targets them.
- Update root `package.json`, `turbo.json`, `bun.lock`, `AGENTS.md` and CI.

## Non-goals

- `apps/desktop` Tauri harness (decide separately).
- Shared packages the remaining apps still import.

## Design

Order after 0002. Build the import graph first (`packages/*` consumers) so no kept package loses a dependency. Delete, then regenerate lockfiles.

## Acceptance

`bun install` and `bun run typecheck` pass. No workflow, script or doc references a removed path.
