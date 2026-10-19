# RFC 0004: Remove remote compute (render-farm)

**Status:** Implemented

**Parent:** [0001](0001-remove-remote-features.md)

## Summary

Delete `apps/render-farm` and `crates/render-farm`.

## Problem

Distributed rendering needs remote workers and is unused by a local-only app.

## Goals

- Remove both members from the Cargo workspace.
- Remove Dockerfiles and references in `export` and `rendering` crates.

## Non-goals

- Local GPU rendering and export.

## Design

Grep `scoopy-render-farm` and `render_farm` for dependents, cut them, delete the directories, refresh `Cargo.lock`.

## Acceptance

`cargo check --workspace` passes with no render-farm references.
