# Interfaces across repositories

Systems rarely live alone. A service exposes an API that other systems call, and consumes APIs that other teams own. rqtk lets a **provider** repository mark some of its requirements as interfaces, backed by the definition files it already has (OpenAPI, protobuf, …), and lets a **consumer** repository pin those requirements, keep a reviewed copy, and trace its own requirements to them.

The provider owns the interface. Consumers pin a version of it and never edit it. A constraint a consumer places on the provider ("we need a p99 under 200 ms") stays a consumer requirement that traces to the interface.

## Exposing an interface

Any requirement with an `[interface]` table is an interface. The table is the marker, not the `type`, because type names differ between projects. Each `[[interface.specs]]` entry names a definition file:

```toml
id = "USR-SYS-0007"
title = "Get user"
type = "Interface"
statement = "The user service shall expose GET /users/{id} returning a User."
# …

[[interface.specs]]
format = "openapi"
path = "api/openapi.yaml"
element = "#/paths/~1users~1{id}/get"
```

| Field | Meaning |
|---|---|
| `format` | `openapi`, `protobuf`, `asyncapi`, `jsonschema` or `other` |
| `path` | Repository-relative path. For formats other than `openapi`, a glob such as `proto/**/*.proto` |
| `element` | The part of the definition the requirement governs. For OpenAPI, a JSON pointer (`~1` encodes `/`); free-form otherwise |

Many requirements can share one spec file, each pointing at a different element.

The spec content is part of the requirement's [content hash](../reference/stability.md), so editing the contract puts the requirement under review exactly like editing its statement:

- **OpenAPI 3.0 and 3.1** (YAML or JSON) are parsed. rqtk follows every `$ref`, within the file and to other files, so a contract split across files is complete. It checks that `element` exists, and hashes only that element plus everything it references, in a canonical form. When the element is an operation, the hash also covers what applies to it from outside its own node: parameters declared on the path, `servers`, the document's `security`, and the security schemes the operation or document uses. Changing `GET /users/{id}`, the `User` schema it returns, or the `{id}` parameter declared on the path changes this requirement's hash. Changing an unrelated operation, reformatting, reordering keys or switching between YAML and JSON does not.
- **Every other format** is hashed as whole files, with line endings normalised. rqtk doesn't follow imports in these formats, so list every file of a multi-file definition, usually with a glob. A glob skips files your repository's `.gitignore` excludes, so it matches what a commit holds.

`rqtk lint` reports spec paths that match nothing ([RQ032](../reference/lint-rules.md#rq032)), unknown formats ([RQ033](../reference/lint-rules.md#rq033)), an `[interface]` table without specs ([RQ034](../reference/lint-rules.md#rq034)), OpenAPI documents it can't parse ([RQ035](../reference/lint-rules.md#rq035)), references to URLs or outside the repository ([RQ036](../reference/lint-rules.md#rq036)) and pointers that don't resolve ([RQ037](../reference/lint-rules.md#rq037)).

Publish a version by tagging a commit, ideally with [`rqtk baseline`](change-control.md#baselines).

## Consuming an interface

> **Only vendor interfaces from providers you trust, and review what you pull in.** Vendored requirements and spec files are written by the provider. rqtk treats them strictly as data: it parses and hashes them, never executes them, escapes their text in reports, and refuses symbolic links, references outside the provider's repository, oversized files and file names containing control characters. But their *content* is still the provider's. `rqtk interfaces add` and `sync` print this warning every time.

Declare the provider under a local alias, then sync it:

```bash
rqtk interfaces add users https://github.com/acme/user-service --ref rqtk/1.4.0
rqtk interfaces sync
git diff -- .rqtk/vendor .rqtk/interfaces.lock   # review, then commit
```

`add` writes the source to `.rqtk/config.toml`:

```toml
[interfaces.users]
git = "https://github.com/acme/user-service"
ref = "rqtk/1.4.0"
```

Only `https://` and SSH (`ssh://…`, `git@host:path`) URLs are accepted. `file://` URLs and local paths need `--allow-file-url`.

`sync` fetches each source at its ref and writes, all or nothing:

```
.rqtk/
  interfaces.lock               ← resolved commit, and the hash of every vendored file
  vendor/users/
    requirements/USR-SYS-0007.toml
    files/api/openapi.yaml      ← every file the specs match or reference
    status.toml                 ← the provider's verification status at that commit
```

`sync` refuses a provider whose interface specs have a problem its own `rqtk lint` would report, such as an element that doesn't resolve, or a `$ref` that runs through a symbolic link. Copying such a spec would vendor a contract that differs from the provider's. If a sync is interrupted while it swaps the new snapshot in, the next `sync` restores the previous one first.

Commit both. `lint` and `coverage` read only these files, so CI needs no network access and no credentials for the provider. `sync <alias>` updates one source and leaves the others alone. `rqtk interfaces list` shows what each alias is pinned to, and `rqtk interfaces remove <alias>` drops a source together with its snapshot.

Pin a tag or a commit SHA. A branch is allowed, but lint warns about it ([RQ042](../reference/lint-rules.md#rq042)) because the same config would mean a different contract tomorrow.

## Tracing to an interface

Link a consumer requirement with `consumes`, qualifying the provider's ID with its alias:

```toml
[trace]
consumes = ["users:USR-SYS-0007"]
```

From then on:

- **Upstream changes are reviewed.** When a sync brings in a different hash for a consumed requirement (its statement, or the part of the spec it governs), every requirement that consumes it becomes **Suspect** with reason `upstream_changed`. Check that it still fits, then [`rqtk review`](change-control.md#review-debt) it. `rqtk impact <base>` lists the consumed interfaces that changed and the requirements downstream of them.
- **Broken links are lint errors.** A link to an undeclared alias ([RQ038](../reference/lint-rules.md#rq038)) or to a requirement the provider no longer exposes ([RQ039](../reference/lint-rules.md#rq039)).
- **The provider's status is visible but not yours.** `context`, `trace`, `graph`, `report` and `coverage` show consumed interfaces as read-only `users:USR-SYS-0007` nodes with the provider's status. That status never counts toward your coverage or `--strict`: your requirement is proven by your tests.

`rqtk context users:USR-SYS-0007` shows a vendored requirement and who consumes it. Commands that change requirements (`review`, `open`, `add-activity`) refuse vendored IDs, and no command except `interfaces sync` and `interfaces remove` writes under `.rqtk/vendor/`.

## Cycles and self-references

Two systems may consume each other, for example an API in one direction and a webhook back, and longer rings work the same way. Each side pins the other independently and `sync` never follows a provider's own dependencies, so a cycle can't loop. A change makes Suspect the requirements that consume it and their descendants, just as a changed parent does, and nothing in the repository it came from. It reaches another repository only when a consumer changes *its own* interface and publishes a new version, so nothing echoes back to where it started. Cross-repository cycles are not trace cycles: [RQ017](../reference/lint-rules.md#rq017) only looks at links within one repository.

An alias that points back at the repository itself, or at a fork of it, is almost always a mistake. `sync` warns when the commit a provider resolves to is already in your own history, but still syncs.

## Lint checks on vendored files

`rqtk lint` compares the vendored files with the lockfile and the config, and reports:

- a vendored file edited, added or deleted by hand ([RQ040](../reference/lint-rules.md#rq040));
- a source declared but not synced, synced from a different URL or ref than configured, or vendored but no longer declared ([RQ041](../reference/lint-rules.md#rq041));
- a vendored requirement that can't be read ([RQ043](../reference/lint-rules.md#rq043)).

Vendored requirements are checked against rqtk's built-in schema only, never against your config: the provider's ID scheme and types are its own.
