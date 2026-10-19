# RFC 0009: Remove remote packages

**Status:** Implemented

## Summary

Remove remote and cloud packages under `packages/` (`packages/env`, `packages/s3`, `packages/sdk-embed`, `packages/sdk-recorder`), and purge remote modules (`stripe`, `dub`, `plans`, `database.ts`) and unused remote dependencies from `packages/utils`.

## Problem

After removing remote applications and the backend in RFC 0001–0005, several packages and modules in `packages/` remain that exist solely for the removed cloud infrastructure: environment configurations for S3/database/Stripe (`packages/env`), S3 storage docs (`packages/s3`), embedding and upload SDKs (`packages/sdk-embed`, `packages/sdk-recorder`), and remote Stripe/Dub/plans/database modules in `packages/utils`.

## Goals

- Delete obsolete remote packages: `packages/env`, `packages/s3`, `packages/sdk-embed`, `packages/sdk-recorder`.
- Remove remote modules from `packages/utils` (`src/lib/stripe/`, `src/lib/dub.ts`, `src/constants/plans.ts`, `src/types/database.ts`).
- Remove remote dependencies from `packages/utils/package.json` (`@aws-sdk/client-s3`, `@scoopy/env`, `dub`, `stripe`).
- Update `packages/utils/src/index.ts` to export only local utilities (`helpers.ts`).

## Non-goals

- Removing local packages required for desktop recording, UI, and TS configuration: `packages/recorder-core`, `packages/ui`, `packages/ui-solid`, `packages/tsconfig`, `packages/config`, `packages/web-api-contract`.
- Modifying Rust crates or desktop-gpui core recording logic.

## Design

1. Remove directories `packages/env`, `packages/s3`, `packages/sdk-embed`, and `packages/sdk-recorder`.
2. In `packages/utils`, delete `src/lib/stripe`, `src/lib/dub.ts`, `src/constants/plans.ts`, `src/types/database.ts`. In `src/index.ts`, re-export only `./helpers.ts`. In `package.json`, remove dependencies on `@aws-sdk/client-s3`, `@scoopy/env`, `dub`, and `stripe`.
3. In `packages/ui`, declare ambient process environment in `Logo.tsx` for standalone version resolution.
4. Prune lockfile via `bun install` and verify `bun run typecheck` passes.

## Acceptance

- No remote packages (`env`, `s3`, `sdk-embed`, `sdk-recorder`) exist under `packages/`.
- `packages/utils` contains no Stripe, Dub, plans, or database references, and carries no remote cloud dependencies.
- `bun run typecheck` passes without errors.
- `node scripts/specify.mjs sync-check` passes with no errors.
