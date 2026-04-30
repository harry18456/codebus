## ADDED Requirements

### Requirement: Frontend typecheck baseline stays at zero errors

The `web/` package SHALL maintain a `vue-tsc` typecheck baseline of exactly zero errors at every committed state of `main`. Any Spectra change that lands under `openspec/changes/archive/` MUST NOT introduce a new TS diagnostic that would cause `cd web && npm run typecheck` (which delegates to `nuxt typecheck` → vue-tsc in project-references build mode) to exit non-zero.

A defensive test `web/tests/types/typecheck-baseline.spec.ts` SHALL spawn `vue-tsc --build --noEmit` from inside the test process and assert exit code zero. The `--build` flag is required because `web/tsconfig.json` is a project-references-only file (`files: []` + four references); `vue-tsc -p .` without `--build` does not traverse the references and silently checks nothing. The test MUST run as part of the normal `npm run test` invocation (i.e., `vitest run`) so the next regression is caught at the CI / pre-commit gate, not at the next manual `npm run typecheck` audit.

If a future change legitimately needs to relax this baseline (e.g., a planned migration with intermediate type errors), the change proposal MUST justify the relaxation in its Non-Goals section and the defensive test MUST be updated in the same change to permit the documented diagnostics — silent baseline drift MUST NOT occur.

#### Scenario: vue-tsc reports zero errors against the current main

- **WHEN** the repository is at a clean checkout of `main` and `cd web && npx vue-tsc --build --noEmit` runs to completion
- **THEN** the process exit code MUST be `0`
- **AND** the stdout/stderr MUST NOT contain any line matching the pattern `error TS\d+:`

#### Scenario: Defensive vitest test asserts zero typecheck errors

- **WHEN** `npm run test` runs to completion against a clean `main`
- **THEN** the `tests/types/typecheck-baseline.spec.ts` test MUST pass
- **AND** the assertion MUST be that `vue-tsc --build --noEmit` exited with code `0`

#### Scenario: Defensive vitest test surfaces the offending diagnostic on regression

- **WHEN** an edit introduces a new TS diagnostic to any `.vue` or `.ts` file under `web/app/` and `npm run test` runs
- **THEN** the `tests/types/typecheck-baseline.spec.ts` test MUST report a failure
- **AND** the failure message MUST contain the captured `vue-tsc` stdout/stderr (which includes the offending file path and the TS error code) so the developer can locate the regression from the vitest output alone
