# Reading repositories from code

The client reads an rqtk repository from a program: its requirements, needs and stakeholders with their links, verification status, lint findings and change history. It gives the same answers as the command line, because the CLI is built on it: every view below is the same document `rqtk <command> --json` prints.

It is one Rust library, `rqtk-client`, also available in Python as part of the `rqtk` package and in TypeScript as `@rqtk/client` (Node 18 or later, Bun and Deno). The client only reads. Changes go through the `rqtk` command line.

```toml
# Rust
[dependencies]
rqtk-client = "2"
```

```bash
# Python 3.9 or later
pip install rqtk

# Node, Bun or Deno
npm install @rqtk/client
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

```python
import rqtk

repo = rqtk.open(".")
release = rqtk.open(".", ref="1.4.0")
users = rqtk.open_remote("https://github.com/acme/user-service", "rqtk/1.4.0")
```

```ts
import { open, openRef, openRemote } from "@rqtk/client";

const repo = await open(".");
const release = await openRef(".", "1.4.0");
const users = await openRemote("https://github.com/acme/user-service", "rqtk/1.4.0");
```

A repository is a **snapshot**. Everything is loaded and checked when it opens, and every query answers from that state. Edits made afterwards, even to the files of a working tree, are not seen until you call `repo.reload()`, which returns a new snapshot. That keeps a dashboard or an editor from ever showing half an edit. A repository can be shared between threads (it is `Send + Sync`) and queried from all of them at once.

Verification status is computed at the snapshot's revision too: from the tests linked in that commit and the evidence committed with it. So a repository opened at a baseline shows the coverage that baseline had.

### Remote repositories

`open_remote` accepts `https://` and SSH URLs, and authenticates the way git does: credential helpers, the SSH agent and `~/.ssh`. A fetched repository is kept as a bare clone and fetched again on the next open, so only new commits are transferred:

| Platform | Cache |
|---|---|
| Linux | `~/.cache/rqtk/repos` (or `$XDG_CACHE_HOME/rqtk/repos`) |
| macOS | `~/Library/Caches/rqtk/repos` |
| Windows | `%LOCALAPPDATA%\rqtk\cache\repos` |

Set `RQTK_CACHE_DIR` to put it elsewhere. Nothing is removed automatically; `clear_cache()` (`rqtk_client::clear_cache()`, `rqtk.clear_cache()`) deletes the whole cache.

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

```python
for entry in repo.requirements():
    print(entry.id, entry.status, entry.requirement.title)
    for link in entry.links:
        print(" ", link.kind, link.direction, link.id)
```

```ts
for (const entry of repo.requirements()) {
  console.log(entry.id, entry.status, entry.requirement.title);
}
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

```python
open_work = repo.find_requirements(
    rqtk.Filter(status=("suspect", "failed"), ancestor="REQ-SYS-0001")
)
# or as a dict, e.g. straight from a JSON request:
open_work = repo.find_requirements({"status": ["suspect", "failed"], "ancestor": "REQ-SYS-0001"})
```

```ts
const openWork = repo.findRequirements({ status: ["suspect", "failed"], ancestor: "REQ-SYS-0001" });
```

Every field that is set must match; a list matches when any of its values does. The fields are `status`, `category`, `type`, `state`, `priority`, `ancestor` (anywhere below that requirement), `consumes` (an `<alias>:<ID>`, or just an alias) and `text` (case-insensitive, in the ID, title, statement or rationale). A filter is plain data: it serialises to JSON, with the same field names in every client language.

## Views

Method names follow each language: `find_requirements` in Rust and Python is `findRequirements` in TypeScript. The data's field names are the JSON's everywhere.

| Method | Same as |
|---|---|
| `context(id)` | `rqtk context <ID> --json` |
| `trace(id)` | `rqtk trace <ID> --json` |
| `coverage()` | `rqtk coverage --json` |
| `lint()` | `rqtk lint --json` |
| `impact(base)` | `rqtk impact <BASE> --json` |
| `diff(from_ref, to_ref)` | `rqtk diff <FROM> <TO> --json` |
| `baselines()` | `rqtk baseline --list --json` |
| `history(id)` | `rqtk log <ID> --json` |

Each returns a typed document with the same fields as the JSON, including its `schema` field. `rqtk schema <name>` prints the JSON Schema of any of them. A missing ID (`context`, `trace`, `history`) or ref (`impact`, `diff`) is an error.

## Errors

A repository fails to open only when it can't be read at all: there is no `.rqtk/config.toml`, the config doesn't parse, its schema version is newer than this rqtk, or the ref or URL can't be resolved. Anything else, including a requirement file that doesn't parse, loads, and the problem shows up in `lint()`.

Every error has a `code` from a fixed list (`ErrorCode`): `not_a_repository`, `invalid_config`, `unsupported_schema`, `ref_not_found`, `remote_refused`, `remote_failed`, `not_found`, `git` and `io`. Branch on the code, not on the message. The same codes appear in the CLI's JSON errors.

```python
try:
    repo = rqtk.open(path)
except rqtk.Error as e:
    if e.code == "not_a_repository":
        ...
```

## Python

- **Values are frozen dataclasses** from `rqtk.types`, generated from the published JSON Schemas, with the JSON's field names. Lists are tuples. `dataclasses.asdict(value)` gives JSON-ready data, since every enumeration is a `Literal` string (`entry.status == "suspect"`).
- **A field the JSON can leave out** has its default: `()`, `{}`, `False`, or `None`.
- **Tagged unions are one dataclass per variant.** `repo.context(id)` returns a `ContextDocRequirement`, `ContextDocNeed`, `ContextDocStakeholder` or `ContextDocInterface`, each with a `kind` field. Branch with `isinstance` or on `kind`. The same holds for activity states (`ActivityEntryPassed`, …) and `RepositoryInfo.origin`.
- **Calls release the GIL**, so other threads run while a repository loads or a view is computed, and one `Repository` answers calls from several threads at once. A web server can open a repository once and share it between all its requests.

## TypeScript

- **Values are the JSON**, parsed, and typed by the interfaces in `@rqtk/client/types`, generated from the published schemas. Every field is `readonly`; a field marked `?` is left out when it is empty, `false` or unset.
- **Tagged unions narrow on their tag**: `if (context.kind === "requirement") { context.requirement … }`.
- **`open`, `openRef`, `openRemote` and `reload` return Promises** and run on a worker thread, so the event loop keeps running while a repository loads. Queries answer from memory and return directly.
- **Errors are `RqtkError`**, with the client's `code`. A filter with an unknown field throws a `TypeError`.
- **Under Bun, set `RQTK_CACHE_DIR` before the process starts**: Bun doesn't pass later changes to `process.env` on to native code.

## Versions

`rqtk-client`, the `rqtk` Python package and `@rqtk/client` are released with the same version as the `rqtk` command line, and their public APIs follow semantic versioning.
