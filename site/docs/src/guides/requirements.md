# Writing requirements

rqtk tracks three kinds of item, each a TOML file named after its ID:

| Item | Lives in | Says |
|---|---|---|
| **Stakeholder** | `.rqtk/stakeholders/` | who cares about the system |
| **Need** | `.rqtk/needs/` | what a stakeholder needs, and why |
| **Requirement** | `.rqtk/requirements/<CATEGORY>/` | what the system shall do, and how that is verified |

Requirements *satisfy* needs, *decompose* into child requirements through `trace.parents`, and are proven by *verification activities*. Every link is by ID, and `rqtk lint` checks that every link resolves.

## Creating items

```bash
rqtk add-stakeholder --name "Operator" --role "Runs the ground station"
rqtk add-need --title "Fast recovery" \
  --statement "Operators need the unit back within seconds after a restart." \
  --stakeholders STK-001
rqtk add --category SYS --type Functional --title "Fast boot" \
  --statement "The system shall boot in under 5 seconds." \
  --rationale "Operators restart the unit during a pass." \
  --satisfies NEED-0001 \
  --criteria "Boot completes in under 5 s on reference hardware." \
  --activity "Boot time test"
```

rqtk assigns the next free ID in each case. Pass `--dry-run` to see the file without writing it, and `--json` to get the assigned ID in machine-readable form.

`rqtk add` also takes `--parent` (repeatable) for the requirement it decomposes, `--priority`, and `--method`, `--level` and `--phase` for how it is verified. A category listed in `validation.require_parent_for_categories` needs `--parent`.

Each `--activity` gets an ID derived from the requirement's: `REQ-SYS-0001` gets `VA-SYS-0001-01`, `VA-SYS-0001-02`, and so on. Tests cite these IDs. Add an activity to an existing requirement with:

```bash
rqtk add-activity REQ-SYS-0001 --name "Cold boot test"
```

## Anatomy of a requirement

```toml
id = "REQ-SYS-0001"
title = "Fast boot"
category = "SYS"
type = "Functional"
state = "Approved"
priority = "High"
statement = "The system shall boot in under 5 seconds."
rationale = "Operators restart the unit during a pass."

[trace]
satisfies = ["NEED-0001"]
# parents, derived_from, refines, depends_on, conflicts_with, related: all by ID

[verification]
method = "Test"
level = "System"
phase = "Development"
success_criteria = "Boot completes in under 5 s on reference hardware."

[[verification.activities]]
id = "VA-SYS-0001-01"
name = "Boot time test"
```

The full list of fields is on the [File formats](../reference/file-formats.md) page, and `rqtk schema requirement` prints the JSON Schema. Unknown fields are errors, so a typo is reported instead of silently ignored.

## Good statements

`rqtk lint` enforces the basics, and the project configuration can tighten them:

- **One sentence, with the shall keyword** (RQ010). Split compound requirements.
- **Measurable.** A test must be able to decide pass or fail. Forbid vague words such as "fast" or "user-friendly" with `validation.forbidden_keywords` (RQ011).
- **Justified.** A `rationale` says why the requirement exists (RQ007).
- **Traced.** Set `trace.parents` to the requirement it decomposes, and `trace.satisfies` to the need it serves. With `forbid_orphans`, every requirement must trace back to a root category (RQ018).

## Finding your way around

```bash
rqtk search boot -i            # substring search across requirements, needs and stakeholders
rqtk trace REQ-SYS-0001        # parents and children
rqtk context REQ-SYS-0001      # everything about one item: links, tests, status, findings
rqtk graph > trace.dot         # the whole graph, for Graphviz
```

## Configuration

`.rqtk/config.toml` decides the vocabulary: categories and their hierarchy, requirement types, lifecycle states, priorities, verification methods, and which lint rules apply. See [Configuration](../reference/configuration.md).
