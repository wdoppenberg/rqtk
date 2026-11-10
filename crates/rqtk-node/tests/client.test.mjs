// The TypeScript client on Node, Bun and Deno: opening repositories, querying them, and
// what the values look like. Written against node:test, which all three run.

import assert from "node:assert/strict";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";

import * as rqtk from "../index.js";
import { cli, firesat, git, large, temporary } from "./fixtures.mjs";

const root = firesat();

// rqtk: verifies VA-CLIENT-0014-01
test("opens a checkout and lists its items", async () => {
  const repo = await rqtk.open(root);
  assert.equal(repo.info().origin.kind, "checkout");

  const ids = repo.requirements().map((r) => r.id);
  assert.deepEqual(ids, [...ids].sort());
  const sys1 = repo.requirement("FOBC-SYS-0001");
  assert.equal(sys1.requirement.title, "On-Board Computer Command Execution");
  assert.equal(sys1.path, ".rqtk/requirements/SYS/FOBC-SYS-0001.toml");
  // Links in both directions.
  assert.ok(sys1.links.some((l) => l.kind === "parents" && l.direction === "incoming" && l.id === "FOBC-SW-0001"));
  assert.ok(sys1.links.some((l) => l.kind === "satisfies" && l.direction === "outgoing" && l.id === "NEED-0001"));
  assert.equal(repo.requirement("FOBC-SYS-9999"), null);

  assert.equal(repo.need("NEED-0001").satisfied, true);
  assert.deepEqual(repo.needs().map((n) => n.id), ["NEED-0001"]);
  assert.equal(repo.stakeholder("STK-001").stakeholder.name, "Mission operations");
  assert.deepEqual(repo.stakeholders().map((s) => s.id), ["STK-001"]);
  assert.equal(repo.need("NEED-9999"), null);
  assert.equal(repo.stakeholder("STK-999"), null);

  const activity = repo.activities().find((a) => a.id === "FOBC-VA-SYS-001-01");
  assert.equal(activity.requirement, "FOBC-SYS-0001");
  assert.equal(activity.state, "manual");
});

// rqtk: verifies VA-CLIENT-0014-01
test("views are exactly what the CLI prints", async () => {
  const repo = await rqtk.open(root);
  const views = [
    [["context", "FOBC-SW-0001"], repo.context("FOBC-SW-0001")],
    [["context", "NEED-0001"], repo.context("NEED-0001")],
    [["context", "STK-001"], repo.context("STK-001")],
    [["trace", "FOBC-SW-0001"], repo.trace("FOBC-SW-0001")],
    [["coverage"], repo.coverage()],
    [["lint"], repo.lint()],
    [["impact", "v1"], repo.impact("v1")],
    [["diff", "v1", "HEAD"], repo.diff("v1", "HEAD")],
    [["baseline", "--list"], repo.baselines()],
    [["log", "FOBC-SW-0002"], repo.history("FOBC-SW-0002")],
  ];
  for (const [args, view] of views) {
    assert.deepEqual(view, cli(root, ...args), args.join(" "));
  }
  assert.equal(repo.context("FOBC-SW-0001").kind, "requirement");
  assert.deepEqual(repo.diff("v1", "HEAD").modified, [{ id: "FOBC-SW-0002", change: "semantic" }]);
  assert.deepEqual(repo.baselines().baselines.map((b) => b.name), ["1.0.0"]);
  assert.equal(repo.history("FOBC-SW-0002").commits.length, 2);
});

// rqtk: verifies VA-CLIENT-0014-01
test("filters are data", async () => {
  const repo = await rqtk.open(root);
  const children = repo.findRequirements({ ancestor: "FOBC-SYS-0001", category: ["SW"] });
  assert.deepEqual(children.map((r) => r.id), ["FOBC-SW-0001", "FOBC-SW-0002", "FOBC-SW-0003"]);
  assert.deepEqual(repo.findRequirements(), repo.requirements());
  assert.throws(() => repo.findRequirements({ colour: ["red"] }), TypeError);
});

// rqtk: verifies VA-CLIENT-0014-01
test("a snapshot at a ref, and reload", async () => {
  const atV1 = await rqtk.openRef(root, "v1");
  assert.deepEqual(atV1.info().origin, { kind: "ref", path: root, reference: "v1" });
  const head = await rqtk.open(pathToFileURL(root));
  assert.notEqual(
    atV1.requirement("FOBC-SW-0002").requirement.statement,
    head.requirement("FOBC-SW-0002").requirement.statement,
  );

  // A working tree snapshot sees edits only after reload().
  const work = join(temporary("work"), "work");
  git(root, "clone", "-q", root, work);
  const repo = await rqtk.open(work);
  const path = join(work, ".rqtk/requirements/SYS/FOBC-SYS-0002.toml");
  writeFileSync(path, readFileSync(path, "utf8").replace('title = "', 'title = "Edited '));
  assert.ok(!repo.requirement("FOBC-SYS-0002").requirement.title.startsWith("Edited"));
  const fresh = await repo.reload();
  assert.ok(fresh.requirement("FOBC-SYS-0002").requirement.title.startsWith("Edited"));
  assert.throws(() => new rqtk.Repository(), TypeError);
});

// The cache must be set before the process starts: Bun doesn't pass later changes to
// process.env on to native code. Unset, the test skips rather than clear the real cache.
const cache = process.env.RQTK_CACHE_DIR;

// rqtk: verifies VA-CLIENT-0014-01
test("remote repositories open through the cache", { skip: !cache && "RQTK_CACHE_DIR is not set" }, async () => {
  assert.equal(rqtk.cacheDir(), cache);
  const url = pathToFileURL(root).href;
  await assert.rejects(rqtk.openRemote(url, "v1"), { name: "RqtkError", code: "remote_refused" });
  const repo = await rqtk.openRemote(url, "v1", { allowFileUrl: true });
  assert.equal(repo.info().origin.kind, "remote");
  assert.notEqual(repo.requirement("FOBC-SYS-0001"), null);
  assert.ok(readdirSync(cache).length > 0);
  rqtk.clearCache();
  assert.ok(!existsSync(cache));
});

// rqtk: verifies VA-CLIENT-0014-01
test("errors carry a code", async () => {
  const codes = [];
  const expect = async (promise, code) => {
    await assert.rejects(promise, (error) => {
      assert.ok(error instanceof rqtk.RqtkError, String(error));
      codes.push(error.code);
      return error.code === code;
    });
  };
  const empty = temporary("empty");
  await expect(rqtk.open(empty), "not_a_repository");
  const broken = temporary("broken");
  mkdirSync(join(broken, ".rqtk"));
  writeFileSync(join(broken, ".rqtk/config.toml"), "not = [toml");
  await expect(rqtk.open(broken), "invalid_config");
  await expect(rqtk.openRef(root, "no-such-ref"), "ref_not_found");
  const repo = await rqtk.open(root);
  await expect((async () => repo.context("FOBC-SYS-9999"))(), "not_found");
  await expect((async () => repo.impact("no-such-ref"))(), "ref_not_found");
  assert.equal(codes.length, 5);
});

// rqtk: verifies VA-CLIENT-0014-02
test("opening does not block the event loop", async () => {
  const big = large(2000);
  let ticks = 0;
  const timer = setInterval(() => ticks++, 1);
  const start = performance.now();
  const repo = await rqtk.open(big);
  const took = performance.now() - start;
  clearInterval(timer);
  assert.equal(repo.requirements().length, 2000);
  assert.ok(took > 20, `the open took ${took} ms, too quick to tell`);
  // A blocked loop runs no timer until the open returns.
  assert.ok(ticks >= 5, `${ticks} timer ticks during a ${took.toFixed(0)} ms open`);
});
