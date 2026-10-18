# Product

<!-- impeccable:product-schema 1 -->

## Platform

Native desktop (macOS and Windows), built with GPUI in `apps/desktop-gpui`. No iOS, Android or web design preset applies.

## Users
Assumed, not confirmed: people who record their screen, camera and microphone and want a finished video file. Creators, tutorial makers, demo and bug-repro authors. Confirm before relying on this.

## Product Purpose
A local-only screen recorder with a built-in editor. Record, trim and polish (zoom, background, captions), then export a video. Success: a recording becomes a finished file with no account, no upload and no network.

## Positioning
Scoopy is inspired by Cap and designed for local only: no server, no account, no network. Every remote feature is removed (see `.spec/rfc/0001-remove-remote-features.md`).

## Capabilities and Constraints
- Recording: screen, camera, microphone. Studio Mode only; Instant Mode needs upload and is removed.
- Editing: timeline, crop, zoom, backgrounds, captions, audio.
- Export to local files.
- Removed or being removed: sign-in, upload, share links, auto-update, web dashboard, render farm, mobile and browser clients.
- Open decisions: exact primary user, whether auto-update stays (product name resolved to Scoopy in RFC 0008).

## Evidence on Hand
No testimonials, metrics or user research in the repo. Do not fabricate any.

## Product Principles
- Local first: nothing leaves the machine.
- The recording flow stays fast and out of the way.
- Editing controls are dense and predictable; this is a tool, not a showcase.
