---
name: rqtk-decompose
description: "Turn a design into derived requirements in `.rqtk/`: a category per component, child requirements for one requirement and the level below it, interfaces and allocation. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

`/rqtk-decompose <ID>` derives the requirements one level below `<ID>` from the design the user has made: what each part of the design must do for `<ID>` to hold.

- **Reads:** `<ID>` and its subtree, the design (a design document, the output of a design skill such as `codebase-design`, or the conversation), and the design ideas kept in `notes`.
- **Writes:** categories in `.rqtk/config.toml`, child requirements with their interfaces and allocation.
- **Leaves to you:** the design itself, and every statement before it is written.

Call the Skill tool with "rqtk-process" first, for the decomposition rules and the way of working with the user, and with "rqtk-requirements" for the authoring rules.

## Steps

### 1. Pin the scope and the design

Read `rqtk context <ID> --json` and `rqtk trace <ID> --json`. `<ID>` is a system requirement, or a derived requirement whose own level is already settled; without an ID, ask which system requirement to start from.

- When `<ID>` has not passed its own gate (see "Where a requirement stands" in `rqtk-process`), say so in one line and ask whether to go on.
- Find the design: ask where it is when the user hasn't said. When there is none, stop: the design comes first, and the hand-over names a design skill or method to make it.

Done when you can name `<ID>`, its gate status and the design you are reading.

### 2. Map the design onto categories

List the components the design names, and for each the parts of `<ID>` it takes on. Ask in one round:

- whether the list of components is right and complete for `<ID>`;
- the category key and name for each component that has none yet, one `level` below the category of `<ID>`.

Add the categories to `.rqtk/config.toml` (and to `identification.id_pattern` if the project sets one). Done when `rqtk lint` exits 0.

### 3. Derive the children

Split `<ID>` into its clauses: each condition, response, limit and quality it demands. For each clause, draft the child requirement of each component that takes part in it:

- the rationale says which clause of `<ID>` it takes, and cites the design decision that put it in this component;
- behaviour that crosses between two components becomes an interface requirement with an `[interface]` table (a draft spec is enough), or a `consumes` link when another repository provides it;
- `[allocation]` `components` names its component;
- a value the design leaves open is `tbd` or `tbr`, with the question in `notes`.

Present the draft as a numbered list, grouped by clause: ID (or "new"), category, statement, the clause it takes, the design decision. Then grill, in rounds:

- Do the children together demand every clause of `<ID>`, and nothing it doesn't?
- Could a test of the component decide each one?
- Is each one requirement or two?

Done when the user confirms the list.

### 4. Write

Write each child with `rqtk add --category <C> --parent <ID> --criteria … --method …`, then add `[allocation]` and `[interface]` by editing its file. Done when `rqtk lint` exits 0.

### 5. Plan verification (ask first)

Ask whether to plan the children's verification now. If so, settle the method and level for each, and add one activity per independently testable behaviour with `rqtk add-activity <ID> --name …`.

## Hand over

- **Changed:** the categories added, and the children written under `<ID>`, each with the clause it takes.
- **Open:** every `tbd` and `tbr` with its question, and any clause no child takes yet.
- **Next:** `/rqtk-decompose <child>` for each child that one component cannot build as it stands; `/rqtk-design-review pdr <ID>` once the level is complete. Leaves are allocated to `software_modules` or `source_files` before `/rqtk-design-review cdr`.
