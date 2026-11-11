# Writing requirements

rqtk tracks three kinds of item, each a TOML file named after its ID:

| Item | Lives in | Says |
|---|---|---|
| **Stakeholder** | `.rqtk/stakeholders/` | who cares about the system |
| **Need** | `.rqtk/needs/` | what a stakeholder needs, and why |
| **Requirement** | `.rqtk/requirements/<CATEGORY>/` | what the system shall do, and how that is verified |

Requirements *satisfy* needs, *decompose* into child requirements through `trace.parents`, and are proven by *verification activities*. Every link is by ID, and `rqtk lint` checks that every link resolves.

## User and system requirements

Requirements come in layers, each a category in `.rqtk/config.toml`:

| Layer | Category (by default) | Says | Links |
|---|---|---|---|
| **User** | `USR` | what a user must be able to do with the system, seen from outside it: "The operator shall be able to …" | `satisfies` the needs it serves |
| **System** | `SYS` | what the system must do or be for the user requirements to be met | `parents` = the user requirements it serves |
| below | your subsystems | what each part of the design must do | `parents` = the requirement it decomposes |

Both user and system requirements say *what*, never *how*: the design comes after them, and the categories below `SYS` follow the design. Lint warns when a user requirement satisfies no need (RQ044), when a system requirement satisfies a need directly (RQ045), and when it has no user requirement as a parent (RQ046).

A category joins a layer with `layer = "user"` or `layer = "system"`. A configuration without layers, as `rqtk init` wrote before 1.3, keeps working unchanged: needs are then satisfied by the root category's requirements. To adopt layers, add a `USR` category at level 1 with `is_root = true` and `layer = "user"`, move `SYS` to level 2 with `layer = "system"`, and give each system requirement the user requirement it serves as a parent.

## Creating items

```bash
rqtk add-stakeholder --name "Operator" --role "Runs the ground station"
rqtk add-need --title "Fast recovery" \
  --statement "Operators need the unit back within seconds after a restart." \
  --stakeholders STK-0001
rqtk add --category USR --type Performance --title "Back in service quickly" \
  --statement "The operator shall be able to use the unit again within 10 seconds of restarting it." \
  --rationale "Operators restart the unit during a pass." \
  --satisfies NEED-0001 --method Demonstration
rqtk add --category SYS --type Functional --title "Fast boot" \
  --statement "The system shall boot in under 5 seconds." \
  --rationale "Leaves half of the operator's 10 seconds for reconnecting." \
  --parent REQ-USR-0001 \
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
# consumes = ["users:USR-SYS-0007"]: an interface of another repository

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

To add a category, add a table for it:

```toml
[categories.SW]
name = "Software"
level = 2
```

Requirement IDs are checked against `<prefix>-<CATEGORY>-<NUMBER>` for the configured categories (RQ001). Set `identification.id_pattern` only if your IDs follow another scheme; then new categories must be added to it too.
