# RFC 0007: Remove CLI remote auth

**Status:** Implemented

**Parent:** [0001](0001-remove-remote-features.md)

## Summary

Remove server authentication and any other remote call from `apps/cli`, so the CLI is local-only.

## Problem

`apps/cli/src/agent_auth.rs` builds an OAuth-style request against a configured server (client_id `scoopy-cli`). A local-only product has no server.

## Goals

- Delete agent auth and server credential storage.
- Audit remaining CLI commands for network calls and remove them.

## Non-goals

- Local recording or export commands.

## Design

Grep `apps/cli/src` for `credentials`, `agent_server_url`, `reqwest` and `http`. Delete leaf-first, fix commands and tests, drop unused dependencies.

## Acceptance

`cargo check -p scoopy` and `cargo test -p scoopy` pass. No network crate use remains in `apps/cli` unless justified in the RFC.
