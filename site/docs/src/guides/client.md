# Reading repositories from code

The `rqtk-client` crate reads an rqtk repository from a Rust program: its requirements, needs and stakeholders with their links, verification status, lint findings and change history. It gives the same answers as the command line, because the CLI is built on it: every view below is the same document `rqtk <command> --json` prints.

The client only reads. Changes go through the `rqtk` command line.

```toml
[dependencies]
rqtk-client = "2"
```

## Opening a repository

```rust
use rqtk_client::Repository;

// A working tree: the directory that holds `.rqtk/`.
let repo = Repository::open(".")?;

// The same repository at a tag, baseline name, branch or commit. Files are read from git,
// so the working tree is never touched.
let release = Repository::open_ref(".", "1.4.0")?;

// A repository you have no checkout of.
let users = Repository::open_remote("https://github.com/acme/user-service", "rqtk/1.4.0")?;
# Ok::<(), rqtk_client::Error>(())
```

A repository is a **snapshot**. Everything is loaded and checked when it opens, and every query answers from that state. Edits made afterwards, even to the files of a working tree, are not seen until you call `repo.reload()`, which returns a new snapshot. That keeps a dashboard or an editor from ever showing half an edit.

Verification status is computed at the snapshot's revision too: from the tests linked in that commit and the evidence committed with it. So a repository opened at a baseline shows the coverage that baseline had.

### Remote repositories

`open_remote` accepts `https://` and SSH URLs, and authenticates the way git does: credential helpers, the SSH agent and `~/.ssh`. A fetched repository is kept as a bare clone and fetched again on the next open, so only new commits are transferred:

| Platform | Cache |
|---|---|
| Linux | `~/.cache/rqtk/repos` (or `$XDG_CACHE_HOME/rqtk/repos`) |
| macOS | `~/Library/Caches/rqtk/repos` |
| Windows | `%LOCALAPPDATA%\rqtk\cache\repos` |

Set `RQTK_CACHE_DIR` to put it elsewhere. Nothing is removed automatically; `rqtk_client::clear_cache()` deletes the whole cache.

## Items and links

```rust
# let repo = rqtk_client::Repository::open(".")?;
for entry in repo.requirements() {
    println!("{} {:?} {}", entry.id, entry.status, entry.requirement.title);
    for link in &entry.links {
        // e.g. Parents Incoming DEMO-SW-0002: a child of this requirement.
        println!("  {:?} {:?} {}", link.kind, link.direction, link.id);
    }
}
# Ok::<(), rqtk_client::Error>(())
```

`requirements()`, `needs()`, `stakeholders()` and `activities()` list everything, and `requirement(id)`, `need(id)` and `stakeholder(id)` look one item up. Every entry carries its links in **both** directions: a requirement lists its parents (`outgoing`) and its children (`incoming`), a need lists its stakeholders and the requirements that satisfy it.

To list only some requirements, pass a `Filter`:

```rust
use rqtk_client::{ClosureStatus, Filter};
# let repo = rqtk_client::Repository::open(".")?;
let open_work = repo.find_requirements(&Filter {
    status: vec![ClosureStatus::Suspect, ClosureStatus::Failed],
    ancestor: Some("REQ-SYS-0001".into()),
    ..Filter::default()
});
# Ok::<(), rqtk_client::Error>(())
```

Every field that is set must match; a list matches when any of its values does. The fields are `status`, `category`, `type`, `state`, `priority`, `ancestor` (anywhere below that requirement), `consumes` (an `<alias>:<ID>`, or just an alias) and `text` (case-insensitive, in the ID, title, statement or rationale). A filter is plain data: it serialises to JSON, with the same field names in every client language.

## Views

| Method | Same as |
|---|---|
| `context(id)` | `rqtk context <ID> --json` |
| `trace(id)` | `rqtk trace <ID> --json` |
| `coverage()` | `rqtk coverage --json` |
| `lint()` | `rqtk lint --json` |
| `impact(base)` | `rqtk impact <BASE> --json` |
| `diff(from, to)` | `rqtk diff <FROM> <TO> --json` |
| `baselines()` | `rqtk baseline --list --json` |
| `history(id)` | `rqtk log <ID> --json` |

Each returns a typed document with the same fields as the JSON, including its `schema` field. `rqtk schema <name>` prints the JSON Schema of any of them.

## Errors

A repository fails to open only when it can't be read at all: there is no `.rqtk/config.toml`, the config doesn't parse, its schema version is newer than this rqtk, or the ref or URL can't be resolved. Anything else, including a requirement file that doesn't parse, loads, and the problem shows up in `lint()`.

Every error has a `code` from a fixed list (`ErrorCode`): `not_a_repository`, `invalid_config`, `unsupported_schema`, `ref_not_found`, `remote_refused`, `remote_failed`, `not_found`, `git` and `io`. Branch on the code, not on the message. The same codes appear in the CLI's JSON errors.

## Versions

`rqtk-client` is released with the same version as the `rqtk` command line, and its public API follows semantic versioning.
