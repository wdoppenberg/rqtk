# Starter `.rqtk/reviews/PROCESS.md`

Write this file when the user wants to tailor the gates, then edit it with them. Keep only the sections that differ from the defaults in `gates.md`; a section left out keeps its default.

```markdown
# Review process

How this project runs design reviews. The rqtk-process and rqtk-design-review skills read this file
and follow it in place of their defaults.

## Gates

In order. A gate left out of this table is not held.

| Gate | Key | Approves |
|---|---|---|
| Requirements review | requirements | user and system requirements |
| Preliminary design review | pdr | other non-leaf requirements |
| Critical design review | cdr | leaves |
| Readiness review | readiness | nothing; it closes the scope |

## States

- Under review: Review
- Approved: Approved

## Criteria

Criteria added to a gate, or dropped from it by number (see the defaults).

### pdr
- Drop 10: there are no design documents.
- Add: every requirement allocated to a component names the team that owns it in `notes`.
```
