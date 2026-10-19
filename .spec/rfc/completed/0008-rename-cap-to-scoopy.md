# RFC 0008: Rename Scoopy to Scoopy

**Status:** Implemented

## Summary

Rename the application and product identity from Scoopy to Scoopy across user-facing surfaces: application menu, window titles, settings headers, permission prompts, sample caption texts, app bundle metadata (`tauri.conf.json`), instance/process matching, and product documentation (`PRODUCT.md`, `package.json`).

## Problem

Following RFC 0001 (removal of remote/cloud features) and RFC 0006 (replacement of app icons with the Scoopy mark), the application is a distinct, offline screen recorder and editor. The user interface, system dialogs, app metadata, and menus still refer to "Scoopy" (`APP_NAME = "Scoopy"`, `productName: "Scoopy"`, `title: "Scoopy"`, "Allow Scoopy in System Settings...").

## Goals

- Update user-facing application name to "Scoopy" in `apps/desktop-gpui/src/menus.rs` (`APP_NAME`).
- Update window titles and app IDs in `apps/desktop-gpui/src/main.rs` and `apps/desktop-gpui/src/settings_window.rs` ("Scoopy", "Scoopy Settings", "Scoopy Camera").
- Update permission dialogs and alert text in `apps/desktop-gpui/src/permissions.rs` and `apps/desktop-gpui/src/menus.rs`.
- Update bundle configuration in `apps/desktop-gpui/tauri.conf.json` (`productName: "Scoopy"`, `mainBinaryName: "Scoopy"`, description).
- Support "Scoopy" and "scoopy" in single-instance process matching in `apps/desktop-gpui/src/single_instance.rs`.
- Update product documentation (`PRODUCT.md`, root `package.json`) resolving the open naming decision to Scoopy.

## Non-goals

- Renaming internal Rust crate names (e.g. `crates/recording`, `crates/camera`, `crates/audio`) or core Rust module paths (`cap_*`).
- Modifying `.scoopy` project file extension or project file association format.
- Overhauling UI components or adding new features.

## Design

1. **Application Constants & Menus:** In `apps/desktop-gpui/src/menus.rs`, set `pub const APP_NAME: &str = "Scoopy";`. This automatically renames the macOS application menu, "About Scoopy", "Hide Scoopy", "Quit Scoopy", and alert dialogs.
2. **Window Titles & IDs:** In `apps/desktop-gpui/src/main.rs`, update window titles and `app_id` to `"Scoopy"`. In `settings_window.rs`, update titles to `"Scoopy Camera"` and `"Scoopy Settings"`, and owner name to `"Scoopy"`.
3. **Permissions & System Prompts:** In `apps/desktop-gpui/src/permissions.rs`, replace user-visible prompts referencing "Scoopy" with "Scoopy".
4. **Single-Instance Matching:** In `apps/desktop-gpui/src/single_instance.rs`, include `"Scoopy"` / `"Scoopy.exe"` in candidate image names so process detection works seamlessly for both development and release binaries.
5. **App Bundle & Project Metadata:** In `apps/desktop-gpui/tauri.conf.json`, update `productName` and `mainBinaryName` to `"Scoopy"`, and update description. Update root `package.json` `"name"` to `"scoopy"`, and update `PRODUCT.md` noting the naming decision is finalized.

## Acceptance

- `APP_NAME` in `menus.rs` is `"Scoopy"`.
- Menu bar displays "Scoopy" (About Scoopy, Hide Scoopy, Quit Scoopy).
- Window titles show "Scoopy" / "Scoopy Settings".
- `tauri.conf.json` defines `productName` as `"Scoopy"`.
- `node scripts/specify.mjs validate .spec/rfc/0008-rename-scoopy-to-scoopy.md` passes with no errors.
