import { describe, expect, it } from "vitest";

import { byteToCodeUnit } from "./selection";

describe("byteToCodeUnit", () => {
  it("maps UTF-8 byte offsets to browser UTF-16 offsets", () => {
    const source = "A🙂文B";

    expect(byteToCodeUnit(source, 0)).toBe(0);
    expect(byteToCodeUnit(source, 1)).toBe(1);
    expect(byteToCodeUnit(source, 5)).toBe(3);
    expect(byteToCodeUnit(source, 8)).toBe(4);
    expect(byteToCodeUnit(source, 9)).toBe(5);
  });

  it("does not split a multibyte character", () => {
    expect(byteToCodeUnit("🙂x", 2)).toBe(0);
  });
});
