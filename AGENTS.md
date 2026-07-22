# Repository instructions

## Agent skills

### Issue tracker

Issues and PRDs live in GitHub Issues. External pull requests are not a triage surface. See `docs/agents/issue-tracker.md`.

### Triage labels

Use `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository. Read `CONTEXT.md` and relevant ADRs before working. See `docs/agents/domain.md`.

## Issue delivery workflow

For each `ready-for-agent` issue:

1. Read the complete issue, `CONTEXT.md`, relevant ADRs, and these repository instructions.
2. Run the `prd-simplicity-audit` skill. Review its recommendations and simplify the spec where appropriate without changing its accepted product outcomes.
3. Start a fresh agent session, load the `implement` skill, and implement only the approved issue.
4. Load the `code-review` skill, review the completed diff, and address every actionable finding.
5. Load the `fallow` skill and inspect only code introduced or modified by the issue. Pre-existing dead code is outside scope.
6. Run the issue's focused tests and repository gates.
7. Push a scoped branch and open a pull request against `main`.
8. Do not merge without review.

