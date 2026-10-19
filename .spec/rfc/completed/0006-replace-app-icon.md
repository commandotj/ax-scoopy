# RFC 0006: Replace app icon

**Status:** Implemented

## Summary

Replace the inherited Scoopy icon and logo with a new icon for the local-only recorder. Concepts live in `logos/`; the chosen SVG is exported and swapped into the app asset files.

## Problem

The app ships Scoopy branding (`apps/desktop-gpui/assets/dock-icon.png`, `assets/icons/logo-full*.svg`, `assets/tray/*`). After RFC 0001 it is a different, offline product.

## Goals

- One new icon that reads at 16 px and 32 px.
- Replace dock, tray and logo assets that exist today; add no new asset types.
- Packaging icons under `apps/desktop-gpui/packaging` and `packaging/` updated if they hold the old mark.

## Non-goals

- Product rename (open decision in PRODUCT.md).
- UI redesign.

## Design

1. Interview, then 3 to 5 SVG concepts in `logos/concepts/`; pick one; refine in `logos/iterations/`.
2. Export PNGs with the logo-designer export script to `logos/export/`.
3. Overwrite only existing files listed in Problem, keeping names and sizes.
4. Check tray icons separately: they are state variants (default, stop), so the mark must work as a monochrome template glyph.

## Acceptance

- Old Scoopy mark appears nowhere in `apps/desktop-gpui/assets`.
- App launches with the new dock and tray icons on macOS.
- 16 px export is legible.
