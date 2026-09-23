<!--
This PR is squash-merged through the merge queue. The TITLE becomes the commit
subject on main and this BODY becomes the commit message, verbatim, so write
both for someone reading `git log` a year from now.

Title: a Conventional Commit, e.g. `fix(scoring): the daily full window actually comes due`
Body: what was wrong, why, what changed, and how you know it works.
-->

## Problem

<!-- What is wrong or missing, with evidence (a measurement, a log line, an issue). -->

## Change

<!-- What this PR does, and why this approach over the alternatives. -->

## Verification

<!-- How you know it works: commands run and their results, tests added, live checks. -->

## Checklist

- [ ] `pnpm run validate:all` passes locally (lint, types, tests, build, Rust)
- [ ] New Rust code has tests; non-trivial frontend components have tests
- [ ] No secrets, API keys, or personal data in the diff, title, or body
- [ ] A scoring change bumps `PIPELINE_VERSION`
- [ ] A dependency change regenerates `NOTICE` in the same PR (`node scripts/generate-notice.cjs`)

## Screenshots

<!-- UI changes only: before and after. Delete this section otherwise. -->
