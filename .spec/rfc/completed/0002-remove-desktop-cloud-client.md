# RFC 0002: Remove desktop cloud client

**Status:** Implemented

**Parent:** [0001](0001-remove-remote-features.md)

## Summary

Delete cloud code from `apps/desktop-gpui`: sign-in, upload, share links, auto-update, remote library state.

## Problem

The desktop app links auth and upload paths that only make sense with the Cap backend.

## Goals

- Remove `auth`, `upload` (queue, multipart tests, verify tests) and `updates` modules.
- Strip auth deeplinks from `deeplink.rs` and upload or share UI from `settings_window.rs`, `main_window.rs`, `library.rs` and `editor_export.rs`.
- Drop dependencies left unused.

## Non-goals

- Local deeplinks, local library, local export.
- Server code (0003).

## Design

Audit each module and caller (`grep` for `crate::auth`, `crate::upload`, `crate::updates`), delete leaf-first, then fix callers. The uncommitted diff in settings, main and editor windows already starts this; fold it in.

## Acceptance

`cargo check -p` and `cargo clippy` pass for desktop-gpui. `grep -ri 'upload\|auth' apps/desktop-gpui/src` shows only local meanings. Record, edit and export work offline.
