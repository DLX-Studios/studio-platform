import { describe, expect, test } from "bun:test";

import {
  buildNewCommand,
  isValidProjectName,
  parseTemplateList,
  shellQuote,
} from "./templates";

describe("parseTemplateList", () => {
  test("parses the JSON gallery contract", () => {
    const templates = parseTemplateList(
      `[{"id":"blank","kind":"playground","description":"Start here."}]`,
    );
    expect(templates).toEqual([
      { id: "blank", kind: "playground", description: "Start here." },
    ]);
  });

  test("empty output lists nothing", () => {
    expect(parseTemplateList("  \n")).toEqual([]);
  });

  test("malformed entries fail closed", () => {
    expect(() => parseTemplateList(`{"id":"blank"}`)).toThrow();
    expect(() => parseTemplateList(`[{"id":"blank"}]`)).toThrow();
    expect(() => parseTemplateList(`not json`)).toThrow();
  });
});

describe("isValidProjectName", () => {
  test("accepts CLI slugs", () => {
    for (const name of ["shop", "pos-clothing-store", "a1", "x"]) {
      expect(isValidProjectName(name)).toBe(true);
    }
  });

  test("rejects the CLI rejects", () => {
    for (const name of ["", "Bad Name", "UPPER", "-lead", "trail-", "a_b", "a.b"]) {
      expect(isValidProjectName(name)).toBe(false);
    }
  });
});

describe("buildNewCommand", () => {
  test("quotes paths with spaces", () => {
    expect(buildNewCommand("/tmp/my dir", "shop", "social")).toBe(
      `cd '/tmp/my dir' && studio new shop -t social`,
    );
  });

  test("escapes single quotes", () => {
    expect(shellQuote(`o'clock`)).toBe(`'o'\\''clock'`);
  });
});
