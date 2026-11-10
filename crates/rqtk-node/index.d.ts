// @rqtk/client: read rqtk repositories from Node, Bun and Deno.

import type * as t from "./types.js";

export type * from "./types.js";

/** A client error. Branch on `code`, one of a fixed list, not on the message. */
export declare class RqtkError extends Error {
  readonly name: "RqtkError";
  readonly code: t.ErrorCode;
}

/**
 * An rqtk repository as it was when it opened: requirements, needs, stakeholders, their
 * links, verification status and lint findings.
 *
 * A snapshot: later edits are not seen until `reload()`, which resolves to a new one. Opening
 * and reloading run off the event loop; queries answer from memory.
 */
export declare class Repository {
  private constructor();

  /** Open the same source again: the working tree as it is now, or the ref's or URL's current commit. */
  reload(): Promise<Repository>;
  /** Where the snapshot came from and its commit. */
  info(): t.RepositoryInfo;

  /** Every requirement, in ID order, with its status and links in both directions. */
  requirements(): t.RequirementEntry[];
  /** The requirement `id`, or null. */
  requirement(id: string): t.RequirementEntry | null;
  /** The requirements matching `filter`; every field that is set must match. */
  findRequirements(filter?: t.Filter): t.RequirementEntry[];
  /** Every need, in ID order, with its links and whether a requirement satisfies it. */
  needs(): t.NeedEntry[];
  /** The need `id`, or null. */
  need(id: string): t.NeedEntry | null;
  /** Every stakeholder, in ID order, with the needs they raised. */
  stakeholders(): t.StakeholderEntry[];
  /** The stakeholder `id`, or null. */
  stakeholder(id: string): t.StakeholderEntry | null;
  /** Every verification activity, with its state, linked tests and evidence. */
  activities(): t.ActivityEntry[];

  /** Everything relevant to one item (`rqtk context`). Throws `RqtkError` (`not_found`). */
  context(id: string): t.ContextDoc;
  /** A requirement's ancestors and descendants (`rqtk trace`). */
  trace(id: string): t.TraceDoc;
  /** Verification status of every requirement and need (`rqtk coverage`). */
  coverage(): t.CoverageDoc;
  /** Lint findings (`rqtk lint`). */
  lint(): t.LintDoc;
  /** What changed since revision `base` and what to re-verify (`rqtk impact`). */
  impact(base: string): t.ImpactDoc;
  /** Requirements added, removed and modified between two revisions (`rqtk diff`). */
  diff(fromRef: string, toRef: string): t.DiffDoc;
  /** The repository's baselines (`rqtk baseline --list`). */
  baselines(): t.BaselinesDoc;
  /** The commits that changed a requirement, newest first (`rqtk log`). */
  history(id: string): t.HistoryDoc;
}

/**
 * Open the repository whose root (the directory holding `.rqtk/`) is `path`. Rejects with
 * `RqtkError` only when the repository can't be read.
 */
export declare function open(path?: string | URL): Promise<Repository>;

/** Open the repository at `path` at revision `ref` (a tag, baseline name, branch or commit), read from git. */
export declare function openRef(path: string | URL, ref: string): Promise<Repository>;

/**
 * Open the repository at git URL `url` (https or SSH, with git's own authentication), at
 * revision `ref`, through the cache. `allowFileUrl` also accepts `file://` URLs and local paths.
 */
export declare function openRemote(
  url: string,
  ref: string,
  options?: { allowFileUrl?: boolean },
): Promise<Repository>;

/** Where `openRemote` keeps fetched repositories. `RQTK_CACHE_DIR` overrides it. */
export declare function cacheDir(): string;

/** Delete every cached repository. Nothing else ever deletes them. */
export declare function clearCache(): void;
