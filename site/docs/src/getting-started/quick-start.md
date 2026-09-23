# Quick start

This walks through the whole loop: set up a repository, write a requirement, prove it with a test, and watch it go Suspect when it changes.

## 1. Initialise

In a git repository:

```bash
rqtk init
```

This creates `.rqtk/config.toml` with a starter policy, plus an example stakeholder and need. Add `--agents` to also install the [agent skills](../guides/agents.md).

## 2. Add a requirement

```bash
rqtk add --category SYS --type Functional \
  --title "Fast boot" \
  --statement "The system shall boot in under 5 seconds." \
  --rationale "Operators restart the unit during a pass."
```

rqtk assigns the next free ID (here `REQ-SYS-0001`) and writes `.rqtk/requirements/SYS/REQ-SYS-0001.toml`. Open it and give the requirement a verification activity:

```toml
[verification]
method = "Test"
level = "Unit"
phase = "Development"
success_criteria = "Boot completes in under 5 s on reference hardware."

[[verification.activities]]
id = "VA-SYS-001-01"
name = "Boot time test"
```

Then check it:

```bash
rqtk lint
```

## 3. Link a test

Put the activity ID above the test that proves it:

```rust
// rqtk: verifies VA-SYS-001-01
#[test]
fn boots_in_under_five_seconds() {
    // …
}
```

`rqtk scan` lists every link it finds.

## 4. Record the results

Run your tests with JUnit output, then hand the report to rqtk:

```bash
cargo nextest run          # writes target/nextest/default/junit.xml
rqtk verify --results target/nextest/default/junit.xml
rqtk coverage
```

The requirement is now **Verified**, and `.rqtk/evidence.toml` records which test passed, at which commit, against which version of the requirement. Commit it with the code.

## 5. Change the requirement

Tighten the statement to "under 3 seconds" and run `rqtk coverage` again. The requirement is now **Suspect**: its tests passed for the old wording, and nothing has proven the new one. Run the tests and `rqtk verify` again to clear it.

`rqtk coverage --strict` exits 1 while anything is Suspect, Failed or unverified, so it works as a CI gate.
