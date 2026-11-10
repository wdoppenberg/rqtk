// Git repositories for the client tests, built from rqtk's own test fixtures.

import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const REPO = join(dirname(fileURLToPath(import.meta.url)), "../../..");

const GIT = [
  "-c", "user.name=Test",
  "-c", "user.email=test@example.com",
  "-c", "commit.gpgsign=false",
  "-c", "tag.gpgsign=false",
  "-c", "init.defaultBranch=main",
];

export function git(cwd, ...args) {
  return execFileSync("git", [...GIT, ...args], { cwd, encoding: "utf8" });
}

export function temporary(name) {
  return mkdtempSync(join(tmpdir(), `rqtk-${name}-`));
}

/** What `rqtk --json <args>` prints, parsed: the CLI built from this checkout. */
export function cli(root, ...args) {
  const exe = join(REPO, "target/debug", process.platform === "win32" ? "rqtk.exe" : "rqtk");
  let stdout;
  try {
    stdout = execFileSync(exe, ["--repo-root", root, "--json", ...args], { encoding: "utf8" });
  } catch (error) {
    stdout = error.stdout;
  }
  return JSON.parse(stdout);
}

function write(root, rel, text) {
  mkdirSync(dirname(join(root, rel)), { recursive: true });
  writeFileSync(join(root, rel), text);
}

/**
 * The FireSat fixture as a git repository, with a stakeholder and a need that FOBC-SYS-0001
 * satisfies: tagged `v1` and baseline `rqtk/1.0.0`, then a second commit that rewords
 * FOBC-SW-0002. The same repository the Python tests use.
 */
export function firesat() {
  const root = join(temporary("firesat"), "obc");
  cpSync(join(REPO, "crates/rqtk/tests/fixtures/firesat-obc"), root, { recursive: true });
  write(root, ".rqtk/stakeholders/STK-001.toml", 'id = "STK-001"\nname = "Mission operations"\n');
  write(
    root,
    ".rqtk/needs/NEED-0001.toml",
    'id = "NEED-0001"\ntitle = "Commanding"\nstate = "Approved"\n' +
      'statement = "Operators need commands executed promptly."\nstakeholders = ["STK-001"]\n',
  );
  const sys1 = join(root, ".rqtk/requirements/SYS/FOBC-SYS-0001.toml");
  writeFileSync(sys1, readFileSync(sys1, "utf8") + '\n[trace]\nsatisfies = ["NEED-0001"]\n');
  git(root, "init", "-q");
  git(root, "add", "-A");
  git(root, "commit", "-q", "-m", "v1");
  git(root, "tag", "v1");
  git(root, "tag", "-a", "rqtk/1.0.0", "-m", "Requirements baseline 1.0.0");
  const sw2 = join(root, ".rqtk/requirements/SW/FOBC-SW-0002.toml");
  writeFileSync(sw2, readFileSync(sw2, "utf8").replace("shall", "shall always"));
  git(root, "commit", "-q", "-am", "v2");
  return root;
}

const LARGE_CONFIG = `schema_version = 1
[project]
name = "large"
version = "0.1.0"
[identification]
prefix = "BIG"
zero_padding = 4
[categories.SW]
name = "Software"
level = 1
is_root = true
[types]
allowed = ["Functional"]
[verification]
methods = ["Test"]
levels = ["System"]
phases = ["Development"]
[lifecycle]
states = ["Draft"]
default_state = "Draft"
[priority]
levels = ["High"]
[criticality]
levels = ["Non-Critical"]
[validation]
require_rationale = false
require_verification_method = true
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
`;

/** A repository of `count` generated requirements, slow enough to open to notice blocking. */
export function large(count) {
  const root = join(temporary("large"), "large");
  write(root, ".rqtk/config.toml", LARGE_CONFIG);
  for (let i = 1; i <= count; i++) {
    const n = String(i).padStart(4, "0");
    write(
      root,
      `.rqtk/requirements/SW/BIG-SW-${n}.toml`,
      `id = "BIG-SW-${n}"\ntitle = "Requirement ${i}"\ncategory = "SW"\n` +
        `type = "Functional"\nstate = "Draft"\npriority = "High"\n` +
        `statement = "The system shall do thing ${i}."\n` +
        `[verification]\nmethod = "Test"\nlevel = "System"\nphase = "Development"\n` +
        `success_criteria = "Thing ${i} happens."\n` +
        `[[verification.activities]]\nid = "VA-SW-${n}-01"\nname = "Thing ${i}"\n`,
    );
  }
  return root;
}
