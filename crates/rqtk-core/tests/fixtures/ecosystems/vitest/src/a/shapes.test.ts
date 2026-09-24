import { describe, expect, it, test } from "vitest";

// rqtk: verifies VA-TS-01
test("plain test with spaces", () => {
  expect(1).toBe(1);
});

describe("outer", () => {
  describe("inner", () => {
    // rqtk: verifies VA-TS-02
    it("nested test", () => {
      expect(true).toBe(true);
    });
  });
});

// rqtk: verifies VA-TS-03
it.each([[450, 480], [1080, 1110]])("rejects %i-%i outside hours", (s, e) => {
  expect(s).toBeLessThan(e);
});

// rqtk: verifies VA-TS-04
it("never accepts two concurrent bookings", () => {
  expect(1).toBe(1);
});

// rqtk: verifies VA-TS-05
test.skip("skipped test", () => {});

// rqtk: verifies VA-TS-06
describe("grouped", () => {
  it("first", () => expect(1).toBe(1));
  it("second", () => expect(2).toBe(3));
});

// rqtk: verifies VA-TS-07
test("same title in two files", () => {
  expect(1).toBe(1);
});
