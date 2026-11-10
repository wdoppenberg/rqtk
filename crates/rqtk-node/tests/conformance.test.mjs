// The conformance suite in TypeScript, on Node, Bun and Deno: each case in
// conformance/cases.json, compared with its conformance/expected/<name>.json. Rust writes
// those files and Python checks the same ones; see conformance/README.md.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";

import * as rqtk from "../index.js";
import { REPO, temporary } from "./fixtures.mjs";

const CONFORMANCE = join(REPO, "conformance");
const suite = JSON.parse(readFileSync(join(CONFORMANCE, "cases.json"), "utf8"));

// Remote cases write to the cache, which has to be set before the process starts (Bun
// doesn't pass later changes to process.env on to native code). Unset, they are skipped
// rather than fill the real cache.
const cache = process.env.RQTK_CACHE_DIR;
const remote = (c) => !("fixture" in c.open);

const root = temporary("conformance");
for (const [name, spec] of Object.entries(suite.fixtures)) {
  const target = join(root, name);
  if (spec.bundle) {
    execFileSync("git", ["clone", "-q", join(REPO, spec.bundle), target]);
  } else if (spec.copy) {
    cpSync(join(REPO, spec.copy), target, { recursive: true });
  } else {
    mkdirSync(target);
    for (const [rel, text] of Object.entries(spec.files ?? {})) {
      mkdirSync(dirname(join(target, rel)), { recursive: true });
      writeFileSync(join(target, rel), text);
    }
    for (const rel of spec.directories ?? []) {
      mkdirSync(join(target, rel), { recursive: true });
    }
  }
}

function fileUrl(path) {
  const text = path.replaceAll("\\", "/");
  return text.startsWith("/") ? `file://${text}` : `file:///${text}`;
}

function opened(spec) {
  if ("fixture" in spec) {
    const path = join(root, spec.fixture);
    return spec.ref === undefined ? rqtk.open(path) : rqtk.openRef(path, spec.ref);
  }
  if ("remote" in spec) {
    const url = fileUrl(join(root, spec.remote));
    return rqtk.openRemote(url, spec.ref, { allowFileUrl: spec.allow_file_url ?? true });
  }
  return rqtk.openRemote(spec.url, spec.ref);
}

const camel = (name) => name.replace(/_(\w)/g, (_, c) => c.toUpperCase());

async function run(c) {
  try {
    const repo = await opened(c.open);
    return { result: repo[camel(c.call)](...c.args) };
  } catch (error) {
    if (error instanceof rqtk.RqtkError) {
      return { error: error.code };
    }
    throw error;
  }
}

function expected(c) {
  let text = readFileSync(join(CONFORMANCE, "expected", `${c.name}.json`), "utf8");
  const name = c.open.fixture ?? c.open.remote;
  const escaped = (s) => JSON.stringify(s).slice(1, -1);
  if (name !== undefined) {
    text = text.replaceAll("$URL", escaped(fileUrl(join(root, name))));
    text = text.replaceAll("$ROOT", escaped(join(root, name)));
  }
  return JSON.parse(text);
}

const isError = (c) => c.name.startsWith("error-");
const isFilter = (c) => c.call === "find_requirements";

async function check(select) {
  const cases = suite.cases.filter((c) => select(c) && (cache || !remote(c)));
  assert.ok(cases.length > 0);
  const failures = [];
  for (const c of cases) {
    const actual = await run(c);
    try {
      assert.deepEqual(actual, expected(c));
    } catch {
      failures.push(c.name);
    }
  }
  assert.deepEqual(failures, [], "differ from conformance/expected");
}

// rqtk: verifies VA-CLIENT-0017-03
// rqtk: verifies VA-SYS-0007-01
test("every view matches the suite", async () => {
  await check((c) => !isError(c) && !isFilter(c));
});

// rqtk: verifies VA-CLIENT-0011-02
test("every filter matches the suite", async () => {
  await check(isFilter);
});

// rqtk: verifies VA-CLIENT-0009-01
test("every error code matches the suite", async () => {
  await check(isError);
});
