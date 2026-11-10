// @rqtk/client: read rqtk repositories from Node, Bun and Deno.
//
// The Rust client does the work; this module parses its JSON (already the shape of the types
// in types.d.ts) and turns its errors into RqtkError.

import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const native = createRequire(import.meta.url)("./native.cjs");
const token = Symbol("rqtk.Repository");

/** A client error. Branch on `code`, one of a fixed list, not on the message. */
export class RqtkError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "RqtkError";
    this.code = code;
  }
}

/** The native error carries the client's error as JSON in its message. */
function converted(error) {
  if (error instanceof Error) {
    try {
      const body = JSON.parse(error.message);
      if (body && typeof body.code === "string" && typeof body.message === "string") {
        return new RqtkError(body.code, body.message);
      }
    } catch {
      // Not a client error.
    }
  }
  return error;
}

function call(f) {
  try {
    return f();
  } catch (error) {
    throw converted(error);
  }
}

const parsed = (text) => JSON.parse(text);
const pathOf = (path) => (path instanceof URL ? fileURLToPath(path) : String(path));
const found = (text) => (text === null ? null : JSON.parse(text));

async function opened(promise) {
  try {
    return new Repository(token, await promise);
  } catch (error) {
    throw converted(error);
  }
}

export class Repository {
  #native;

  /** Use `open`, `openRef` or `openRemote`. */
  constructor(key, handle) {
    if (key !== token) {
      throw new TypeError("open a repository with open(), openRef() or openRemote()");
    }
    this.#native = handle;
  }

  reload() {
    return opened(this.#native.reload());
  }

  info() {
    return parsed(this.#native.info());
  }

  requirements() {
    return parsed(this.#native.requirements());
  }

  requirement(id) {
    return found(this.#native.requirement(id));
  }

  findRequirements(filter = {}) {
    try {
      return parsed(this.#native.findRequirements(JSON.stringify(filter)));
    } catch (error) {
      throw new TypeError(error.message);
    }
  }

  needs() {
    return parsed(this.#native.needs());
  }

  need(id) {
    return found(this.#native.need(id));
  }

  stakeholders() {
    return parsed(this.#native.stakeholders());
  }

  stakeholder(id) {
    return found(this.#native.stakeholder(id));
  }

  activities() {
    return parsed(this.#native.activities());
  }

  context(id) {
    return parsed(call(() => this.#native.context(id)));
  }

  trace(id) {
    return parsed(call(() => this.#native.trace(id)));
  }

  coverage() {
    return parsed(this.#native.coverage());
  }

  lint() {
    return parsed(this.#native.lint());
  }

  impact(base) {
    return parsed(call(() => this.#native.impact(base)));
  }

  diff(fromRef, toRef) {
    return parsed(call(() => this.#native.diff(fromRef, toRef)));
  }

  baselines() {
    return parsed(call(() => this.#native.baselines()));
  }

  history(id) {
    return parsed(call(() => this.#native.history(id)));
  }
}

export function open(path = ".") {
  return opened(native.open(pathOf(path)));
}

export function openRef(path, ref) {
  return opened(native.openRef(pathOf(path), ref));
}

export function openRemote(url, ref, options = {}) {
  return opened(native.openRemote(url, ref, Boolean(options.allowFileUrl)));
}

export function cacheDir() {
  return native.cacheDir();
}

export function clearCache() {
  call(() => native.clearCache());
}
