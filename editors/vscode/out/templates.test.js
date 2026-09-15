"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const bun_test_1 = require("bun:test");
const templates_1 = require("./templates");
(0, bun_test_1.describe)("parseTemplateList", () => {
    (0, bun_test_1.test)("parses the JSON gallery contract", () => {
        const templates = (0, templates_1.parseTemplateList)(`[{"id":"blank","kind":"playground","description":"Start here."}]`);
        (0, bun_test_1.expect)(templates).toEqual([
            { id: "blank", kind: "playground", description: "Start here." },
        ]);
    });
    (0, bun_test_1.test)("empty output lists nothing", () => {
        (0, bun_test_1.expect)((0, templates_1.parseTemplateList)("  \n")).toEqual([]);
    });
    (0, bun_test_1.test)("malformed entries fail closed", () => {
        (0, bun_test_1.expect)(() => (0, templates_1.parseTemplateList)(`{"id":"blank"}`)).toThrow();
        (0, bun_test_1.expect)(() => (0, templates_1.parseTemplateList)(`[{"id":"blank"}]`)).toThrow();
        (0, bun_test_1.expect)(() => (0, templates_1.parseTemplateList)(`not json`)).toThrow();
    });
});
(0, bun_test_1.describe)("isValidProjectName", () => {
    (0, bun_test_1.test)("accepts CLI slugs", () => {
        for (const name of ["shop", "pos-clothing-store", "a1", "x"]) {
            (0, bun_test_1.expect)((0, templates_1.isValidProjectName)(name)).toBe(true);
        }
    });
    (0, bun_test_1.test)("rejects the CLI rejects", () => {
        for (const name of ["", "Bad Name", "UPPER", "-lead", "trail-", "a_b", "a.b"]) {
            (0, bun_test_1.expect)((0, templates_1.isValidProjectName)(name)).toBe(false);
        }
    });
});
(0, bun_test_1.describe)("buildNewCommand", () => {
    (0, bun_test_1.test)("quotes paths with spaces", () => {
        (0, bun_test_1.expect)((0, templates_1.buildNewCommand)("/tmp/my dir", "shop", "social")).toBe(`cd '/tmp/my dir' && studio new shop -t social`);
    });
    (0, bun_test_1.test)("escapes single quotes", () => {
        (0, bun_test_1.expect)((0, templates_1.shellQuote)(`o'clock`)).toBe(`'o'\\''clock'`);
    });
});
