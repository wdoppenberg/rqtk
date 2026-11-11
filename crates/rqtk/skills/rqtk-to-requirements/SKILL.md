---
name: rqtk-to-requirements
description: "Turn the current conversation, a spec or a set of tickets into stakeholder needs and verifiable requirements in `.rqtk/`. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

`/rqtk-to-requirements` turns what has already been discussed, or the spec or tickets the user points to, into needs, user requirements and system requirements with verification activities. It synthesises; `/rqtk-init` interviews.

- **Reads:** the conversation or the spec, and the existing items in the area.
- **Writes:** needs and requirements, after the user approves the draft.
- **Leaves to you:** the approval of the draft, and any change to a requirement that has passed its gate.

This needs an initialised `.rqtk/`. If `rqtk lint` fails with a configuration error, tell the user to run `/rqtk-init` or `rqtk init`. Call the Skill tool with "rqtk-process" first, for the way of working with the user, and with "rqtk-requirements" for the authoring rules.

## Steps

### 1. Gather

Work from the conversation. If the user passes a spec path or issue, read all of it. Read `.rqtk/config.toml` for the categories, layers, types and verification levels this project uses. Use `rqtk search <term> -i` to find existing requirements and needs in the area: extend or reword those rather than adding near-duplicates. Use the vocabulary of `CONTEXT.md` if the repo has one. Done when you have read the whole source and the items it touches.

### 2. Draft

- **Needs**: one per distinct thing a stakeholder needs, with the stakeholder and why.
- **User requirements**: what a user must be able to do, each satisfying a need. A user story gives a need (its "so that") and a user requirement (its "I want"); acceptance criteria give success criteria.
- **System requirements**: what the system must do or be at its boundary for each user requirement, each with that user requirement as its parent. One shall-sentence each, measurable, with rationale, type and success criteria. In a project without layers (see `rqtk-requirements`), the root category's requirements satisfy the needs directly.
- **Decomposition**, only where the source records design decisions: follow the `rqtk-process` skill: one category level per step, each child's rationale saying which part of its parent it takes, design decisions in the rationale they shape, and `tbd` or `tbr` with the open question in `notes` where the source leaves a value open. Allocate to components where the source says.
- **Interfaces**: behaviour other systems rely on (an API, an event, a file format) becomes an interface requirement whose `[interface]` table points at the spec; a dependency on another repository's interface becomes a `consumes` link. The `rqtk-requirements` skill has the format.
- **Verification**: success criteria, plus one activity per independently testable behaviour. Method `Test` unless the behaviour can only be inspected, analysed or demonstrated.

Mark each item **new** or **changed**. A changed requirement is Suspect until tests updated for it pass, and so are its child requirements until they are reviewed; say which. Done when every part of the source maps onto an item, or is noted as out of scope.

### 3. Quiz

Present the draft as a numbered list, needs first, then user requirements, then system requirements: ID (or "new"), statement, parent, need, activities. Ask in rounds:

- Is anything the spec asks for missing, or anything here not asked for?
- Is every statement measurable: could a test decide pass or fail?
- Should any requirement be split or merged? Are the parents right?

Done when the user approves the draft.

### 4. Write

Create needs with `rqtk add-need --json` and requirements with `rqtk add --json`, parents first, reading the assigned ID from each result. `rqtk add` takes `--parent`, `--satisfies`, `--criteria` and one `--activity "name"` per activity, so each requirement is complete in one command. Change existing requirements, and add `[interface]` tables and `consumes` links, by editing their files. Done when `rqtk lint` exits 0; use `rqtk explain <code>` for each finding.

## Hand over

- **Changed:** the IDs created and changed. `rqtk coverage` lists the new requirements as Planned: nothing verifies them yet.
- **Open:** every `tbd` and `tbr` with its question, and what the source left unanswered.
- **Next:** the gate that comes next for them (see `rqtk-process`): usually `/rqtk-design-review requirements` for new user and system requirements, `/rqtk-design-review pdr <ID>` for a new decomposition. If the work continues with tickets, put the requirement ID and its activity IDs in each ticket's acceptance criteria, so whoever implements it links the tests.
