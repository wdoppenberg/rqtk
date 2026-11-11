---
name: rqtk-init
description: "Set up `.rqtk/` for a new or existing project by interviewing the engineer about its stakeholders, needs, user requirements and system requirements, and record the answers with rqtk. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI.
---

`/rqtk-init` sets up `.rqtk/` by **elicitation**: the requirements are in the engineer's head, and you draw them out. The engineer is the source and the authority; you propose and challenge. `/rqtk-to-requirements` does the opposite: it synthesises from a spec.

- **Reads:** the engineer's answers, and in an existing project its code and any `.rqtk/`.
- **Writes:** the configuration, stakeholders, needs, user requirements and system requirements, stage by stage.
- **Leaves to you:** every answer, and the design, which comes after this interview.

The interview treats the system as a **black box**. It settles who the system is for, where its boundary lies, what its users must be able to do, and what the box must do or be for that. It ends before the design: modules, components, storage, libraries and code structure are decided afterwards, with whatever design method the engineer prefers, and `/rqtk-decompose` turns that design into subsystem requirements. Tell the engineer this in your first round.

Call the Skill tool with "rqtk-process" first, for the way of working with the user, and with "rqtk-requirements" before stage 2, for the authoring rules.

## The interview

Grill: keep asking until each answer is concrete enough to write down. That means a stakeholder you can name, a need with a reason, and a shall statement a test could decide.

- **Outside the box.** Every question is about the system as its users and neighbours see it: what goes in, what comes out, how fast, how often, how reliably, under which constraints. When the engineer names something inside the box (a module, a database, a library, a component), ask what it achieves for the user, write that, and keep the design idea in the `notes` of the requirement it serves, for the design that comes later.
- **Make vague answers concrete.** "Fast" becomes how fast, measured where, under what load. "Users" becomes which users. "Handles errors" becomes which errors, and what happens then. "It should be secure" becomes against whom, protecting what.
- **Open questions are recorded, not guessed.** When the engineer can't answer yet, write the item with `tbd = true` (a value not known) or `tbr = true` (a value not settled), with the question in `notes`.
- **Their words.** Use the engineer's vocabulary, and the repo's glossary (`CONTEXT.md`, `GLOSSARY.md`) if it has one.

The engineer can skip a question, stop, or go back to an earlier stage at any time; edit the written files to match.

## Existing code

Code is the record of a design, so it answers how; this interview asks what. Use it for two things only:

- **Facts:** who and what talks to the system today, the interfaces other systems already rely on, the platforms and constraints already committed to.
- **Completeness:** after the engineer has answered a stage, what the code does that nobody has mentioned.

Read the README, the docs, the manifest, the entry points, the public interfaces and the tests while the engineer answers stage 0. Raise each finding at the stage it belongs to, after the engineer's own answers, as a question about the outside of the box, with where you saw it: "uploads are retried (`src/upload.rs:40`); must an upload survive a dropped connection, and for how long?" The engineer decides whether the behaviour is required, incidental or a bug.

## Stages

Each stage ends when the engineer confirms its list. Then write it with rqtk and show the IDs it assigned; later stages refer to those IDs.

### 0. The engineer

Start with the engineer, not the system. Your first round asks them:

- their role in the project;
- why they want requirements now: an audit, a certification, onboarding agents, review discipline, a rewrite;
- the scope: the whole repository, or one part of it;
- who else holds answers, and who should be consulted;
- whether to plan verification at the end (stage 7).

Present no description of the system, from the code or from anywhere else: who it is for and what it must do are what stages 2 to 6 draw out. When `.rqtk/` exists already, read `.rqtk/config.toml`, `rqtk lint`, `rqtk coverage` and the items there. Existing items don't settle who the system is for: stages 2 to 4 still run, see their first lines.

Done when the engineer has confirmed their goal and the scope.

### 1. Setup

One short round: only what `rqtk init` needs before anything can be written.

- The project name and the ID prefix. `rqtk init` defaults the prefix to an abbreviation of the project name (`firesat-obc` gives `FOBC`) and reports it as `prefix` in its JSON. It is only a default: ask whether to keep it or use any other abbreviation the engineer prefers.
- Whether to allow `tbd` and `tbr` (recommend yes while requirements are being written).
- Whether to install the pre-commit hook.

`rqtk init` defines two categories, `USR` for user requirements and `SYS` for system requirements; keep its other defaults. An existing `.rqtk/` whose categories have no `layer`: recommend adopting the layers as the requirements guide describes (`rqtk schema config` lists the fields), and ask.

Run `rqtk init`, with `--hook` if they want it, then set `allow_tbd` and `allow_tbr` in `.rqtk/config.toml`. Done when `rqtk lint` exits 0.

### 2. Stakeholders

Elicit the stakeholders whenever they are not yet clear: there are none, or the engineer can't confirm that the existing ones are complete and current. Show the existing ones and ask what is missing or wrong before you go on. Never infer stakeholders from requirements that are already written.

Find everyone with a stake: who uses it, operates it, maintains it, pays for it, approves its releases, integrates with it, or regulates it. Ask who would complain if it broke, and who has to sign off a change. Another system is not a stakeholder; the team that owns it is.

Write each with `rqtk add-stakeholder --name … --role …`. Done when the engineer confirms nobody is missing and each stakeholder has a role saying what they care about.

### 3. Boundary

Draw the edge of the box:

- what the system is responsible for, and what it leaves to others;
- who and what acts on it from outside: people, other systems, schedules, hardware;
- what crosses the boundary in each direction;
- which interfaces already exist and have consumers that depend on them;
- which constraints are already fixed, and who fixed them: a platform, a regulation, a contract, a mandated technology.

The owner of every outside system is a stakeholder: add any that stage 2 missed. Nothing else is written here; stages 5 and 6 write what this stage settles. Done when the engineer confirms the list of outside actors, existing interfaces and fixed constraints.

### 4. Needs

Elicit the needs whenever they are not yet clear. That covers any of these:

- there are none;
- a need has no stakeholder or no reason;
- a stakeholder raises none;
- a requirement in the root category satisfies none (`rqtk coverage` and `rqtk context <ID>` show this).

For each existing requirement without a need, ask which need it serves. If none does, ask whether a need is missing or the requirement should go. Link the answer with `satisfies`.

For each stakeholder, ask what they need from the system and why, in their terms and without a solution. "Operators need to know within a minute that ingestion has stopped" is a need; "a Grafana alert" is not. Probe:

- what goes wrong for them today;
- what would make them stop using it;
- the constraints they live under: platforms, regulation, budget, deadlines.

Write one need per distinct thing, with `rqtk add-need --title … --statement … --stakeholders … --rationale …`. Done when every stakeholder raises a need, and every need has a stakeholder and a reason.

### 5. User requirements

A **user requirement** says what someone outside the box must be able to do with it: "The operator shall be able to see within 60 s that ingestion has stopped." Draw them out with **scenarios**: for each need, ask the stakeholder's story from the trigger to the outcome. What starts it, what do they do, what do they get, and what goes wrong on the way? Each thing they must be able to do, or be protected from, is a candidate. Grill each one:

- How would they know it worked? What number, under what conditions?
- What happens on failure, at the limits, and with bad input?
- Is it one requirement or two?
- Does it name a solution? Ask what it achieves until the answer is a capability and a measure.

An existing system requirement that satisfies a need directly (RQ045) gets a user requirement between them: find or write it, and move the link.

Write each with `rqtk add --category USR` and `--satisfies` the need, `--criteria` and `--method` (usually `Demonstration` or `Test` at the `Acceptance` level), with the scenario in its rationale. Done when every need is satisfied by a user requirement and the engineer has confirmed every statement.

### 6. System requirements

A **system requirement** says what the box must do or be for a user requirement to be met: its inputs, outputs, responses, timings and qualities, seen from its boundary. For each user requirement, ask what the system must do for it, and grill each candidate as in stage 5. Then:

- Sweep the qualities nobody volunteers: performance, reliability, security, portability, operability, maintainability.
- Write each fixed constraint from stage 3 with type `Constraint`, its rationale naming who imposed it.
- Write each existing interface from stage 3 with type `Interface`. Where the system must exchange something with an outside party through an interface that doesn't exist yet, write what is exchanged and how well; the interface's form is a design decision. Behaviour this system takes from another repository becomes a `consumes` link.

Write each with `rqtk add --category SYS`, `--parent` the user requirements it serves, `--criteria` and `--method`. Done when every user requirement is served by a system requirement, the engineer has confirmed every statement, and `rqtk lint` exits 0.

### 7. Verification (if asked in stage 0)

For each user and system requirement, settle the method, the level, and one activity per independently testable behaviour with `rqtk add-activity <ID> --name …`. In a project with tests, propose linking the tests that check a requirement through the system's public interface (see the `rqtk-verification` skill); tests of internal parts wait for the decomposition.

## Hand over

Leave every item in its default state.

- **Changed:** the stakeholders, needs, user requirements and system requirements written, with their IDs.
- **Open:** every `tbd` and `tbr` with its question; the design ideas kept in `notes`, by requirement; what `rqtk coverage` lists as Gap or Planned.
- **Next:** `/rqtk-design-review requirements`, whose gate moves the user and system requirements on. Then the design, with whatever method the engineer prefers (a design skill such as `codebase-design` where one is available), starting from the system requirements and the ideas in their `notes`. Then `/rqtk-decompose <ID>` for each system requirement.
