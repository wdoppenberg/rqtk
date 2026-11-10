// Opens a fixture with an installed @rqtk/client: the check that a prebuilt package works on
// the platform it was built for. Copy it next to the node_modules that hold the package, then
//   node smoke-client.mjs <repository>
import * as rqtk from "@rqtk/client";

const repo = await rqtk.open(process.argv[2]);
const ids = repo.requirements().map((r) => r.id);
if (!ids.includes("FOBC-SYS-0001") || repo.coverage().requirements.length !== ids.length) {
  throw new Error(`unexpected requirements: ${ids}`);
}
console.log(`@rqtk/client on ${process.platform}-${process.arch}: ${ids.length} requirements`);
