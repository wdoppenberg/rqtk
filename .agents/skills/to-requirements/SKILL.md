---
name: to-requirements
description: "Turn the current conversation, a spec or a set of tickets into stakeholder needs and verifiable requirements in `.rqtk/`. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

Turn what has already been discussed (or the spec or tickets the user points to) into **needs** and **requirements** with verification activities. Synthesise; don't interview. Quiz the user on the draft before writing anything.

This needs an initialised `.rqtk/`. If `rqtk lint` fails with a configuration error, tell the user to run `rqtk init`.

## Process

### 1. Gather

Work from the conversation. If the user passes a spec path or issue, read all of it. Read `.rqtk/config.toml` for the categories, types and verification levels this project uses. Use `rqtk search <term> -i` to find existing requirements and needs in the area: extend or reword those rather than adding near-duplicates. Use the vocabulary of `CONTEXT.md` if the repo has one.

Call the Skill tool with "rqtk-requirements" for the authoring rules.

### 2. Draft

- **Needs**: one per distinct thing a stakeholder needs, with the stakeholder and why. User stories map onto needs.
- **Requirements**: one shall-sentence each, measurable, with rationale, category, type, parent and the need it satisfies. Testing decisions and acceptance criteria in a spec map onto requirements and their success criteria.
- **Verification**: success criteria, plus one activity per independently testable behaviour. Method `Test` unless the behaviour can only be inspected, analysed or demonstrated.

Mark each item **new** or **changed**. A changed requirement is Suspect until tests updated for it pass, and so are its child requirements until they are reviewed; say which.

### 3. Quiz

Present the draft as a numbered list: ID (or "new"), statement, parent, need, activities. Ask:

- Is anything the spec asks for missing, or anything here not asked for?
- Is every statement measurable: could a test decide pass or fail?
- Should any requirement be split or merged? Are the parents right?

Iterate until the user approves.

### 4. Write

Create needs with `rqtk add-need --json` and requirements with `rqtk add --json`, parents first, reading the assigned ID from each result. `rqtk add` takes `--parent`, `--satisfies`, `--criteria` and one `--activity "name"` per activity, so each requirement is complete in one command. Change existing requirements by editing their files.

### 5. Check

`rqtk lint` exits 0; use `rqtk explain <code>` for each finding. `rqtk coverage` lists the new requirements as Planned: nothing verifies them yet.

### 6. Hand over

Report the IDs created and changed. If the work continues with tickets, put the requirement ID and its activity IDs in each ticket's acceptance criteria, so whoever implements it links the tests.
