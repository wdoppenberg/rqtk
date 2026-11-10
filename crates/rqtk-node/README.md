# @rqtk/client

> **rqtk**: requirements toolkit

Read an [rqtk](https://rqtk.dev) repository from Node (18 or later), Bun or Deno: requirements, needs and stakeholders with their links, verification status, lint findings and change history. Every view is the same document `rqtk <command> --json` prints.

```bash
npm install @rqtk/client
```

```ts
import { open } from "@rqtk/client";

const repo = await open(".");               // or openRef(".", "1.4.0"), openRemote(url, ref)
for (const entry of repo.findRequirements({ status: ["suspect", "failed"] })) {
  console.log(entry.id, entry.status, entry.requirement.title);
}
console.log(repo.coverage().summary);
```

Opening runs off the event loop. A repository is a snapshot: later edits are seen after `await repo.reload()`. The data types are in `@rqtk/client/types`, generated from rqtk's published JSON Schemas, with the JSON's field names.

See [Reading repositories from code](https://rqtk.dev/docs/guides/client.html).
