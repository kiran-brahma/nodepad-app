# Issue tracker: GitHub

Issues and PRDs for this repository live in GitHub Issues at `kiran-brahma/nodepad-app`. Use the `gh` CLI for all operations.

## Conventions

- Create an issue with `gh issue create` and a body file for multiline content.
- Read an issue with `gh issue view <number> --comments` and include labels.
- List issues with `gh issue list`, using explicit state and label filters.
- Comment with `gh issue comment <number>`.
- Apply or remove labels with `gh issue edit <number> --add-label` or `--remove-label`.
- Close an issue with `gh issue close <number> --comment "..."`.
- Infer the repository from the `origin` remote when commands run inside this checkout.

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub Issues define work. Pull requests are implementation outcomes created after the issue audit, implementation, review, Fallow, and verification gates. External pull requests do not enter the issue triage queue.

GitHub shares one number space across issues and pull requests. Resolve ambiguous references with `gh pr view <number>` and fall back to `gh issue view <number>`.

## Skill routing

- When a skill says to publish to the issue tracker, create a GitHub issue.
- When a skill says to fetch a ticket, read the complete GitHub issue and comments.
- A fully specified issue receives only the `ready-for-agent` triage label unless the task explicitly requires another label.

