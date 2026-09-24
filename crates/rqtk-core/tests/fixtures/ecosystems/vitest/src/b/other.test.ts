import { expect, test } from "vitest";

// Unrelated and failing, with the same title as a linked test in src/a.
test("same title in two files", () => {
  expect(1).toBe(2);
});
