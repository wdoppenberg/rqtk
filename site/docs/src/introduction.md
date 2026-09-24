# rqtk

**Requirements that live in your repository, proven by tests.**

rqtk keeps requirements, stakeholder needs and verification evidence as TOML files next to your code. A requirement change is a commit, a baseline is a git tag, and history is `git log`. There is no separate tool or database, and no export step between your requirements and your version control.

```
.rqtk/
  config.toml                ← project policy: categories, states, lint rules
  requirements/SYS/SYS-0001.toml
  needs/NEED-0001.toml
  stakeholders/STK-0001.toml
  evidence.toml              ← which tests passed, against which version of each requirement
```

## Why

Most requirements tools are separate systems: documents, spreadsheets or SaaS platforms, disconnected from source control. Engineers end up maintaining two ledgers, and the two drift apart.

rqtk treats requirements as source code, and it holds them to the same standard as code:

- **Checked like code.** `rqtk lint` rejects typos, unknown references, cycles and vague statements, with the file and line of every finding.
- **Proven, not claimed.** A requirement counts as *verified* only when the tests linked to it passed against its current wording. Reword the requirement and it becomes *Suspect* until those tests pass again. Nobody can mark a requirement done by editing a file.
- **Operable by agents.** Every command prints JSON with `--json`, exits with documented codes, and never prompts. Agent skills plug rqtk into a coding agent's normal workflow.

## Where to go next

- [Install rqtk](getting-started/installation.md), then follow the [quick start](getting-started/quick-start.md).
- Read how [verification from test evidence](guides/verification.md) works: it is the core idea.
- Using Claude Code, Codex, Cursor or another coding agent? See [Coding agents](guides/agents.md).
