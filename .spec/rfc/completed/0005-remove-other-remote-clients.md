# RFC 0005: Remove mobile, chrome-extension, status apps

**Status:** Implemented

**Parent:** [0001](0001-remove-remote-features.md)

## Summary

Delete `apps/mobile`, `apps/chrome-extension`, `apps/status`.

## Problem

Satellite clients of the removed backend.

## Goals

- Remove the three apps and their workspace and CI entries.

## Non-goals

- Desktop apps.

## Design

Confirm no kept package imports them, delete, refresh lockfile.

## Acceptance

Workspace installs and builds. No dangling references.
