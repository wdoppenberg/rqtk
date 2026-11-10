// Type-checked, not run: the declarations in index.d.ts and types.d.ts describe a typical use.

import { open, openRemote, RqtkError, type ContextDoc, type Filter, type RequirementEntry } from "../index.js";

export async function dashboard(path: string): Promise<string[]> {
  const repo = await open(path);
  const filter: Filter = { status: ["suspect", "failed"], ancestor: "REQ-SYS-0001" };
  const open_work: RequirementEntry[] = repo.findRequirements(filter);
  const summary: Readonly<Record<string, number>> = repo.coverage().summary;
  const context: ContextDoc = repo.context("REQ-SYS-0001");
  // Tagged unions narrow on their tag.
  if (context.kind === "requirement") {
    context.requirement.statement satisfies string;
  }
  const origin = repo.info().origin;
  if (origin.kind === "remote") {
    origin.url satisfies string;
  }
  try {
    await openRemote("https://example.com/repo.git", "main", { allowFileUrl: false });
  } catch (error) {
    if (error instanceof RqtkError && error.code === "remote_failed") {
      return [];
    }
    throw error;
  }
  const fresh = await repo.reload();
  return [...open_work.map((r) => `${r.id} ${r.status}`), String(summary.verified), fresh.info().commit ?? ""];
}
