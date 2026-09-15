"use strict";
// Studio template gallery helpers: pure logic for the new-project flow.
//
// This module must never import "vscode" so it stays unit-testable under
// bun:test. All editor interaction lives in extension.ts.
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseTemplateList = parseTemplateList;
exports.isValidProjectName = isValidProjectName;
exports.shellQuote = shellQuote;
exports.buildNewCommand = buildNewCommand;
/** Parse `studio new --list-templates --format json` output. */
function parseTemplateList(output) {
    const trimmed = output.trim();
    if (trimmed.length === 0) {
        return [];
    }
    const parsed = JSON.parse(trimmed);
    if (!Array.isArray(parsed)) {
        throw new Error("template list is not an array");
    }
    return parsed.map((entry) => {
        if (typeof entry !== "object" ||
            entry === null ||
            typeof entry.id !== "string" ||
            typeof entry.kind !== "string" ||
            typeof entry.description !== "string") {
            throw new Error("template entry is malformed");
        }
        const record = entry;
        return { id: record.id, kind: record.kind, description: record.description };
    });
}
/** Mirror of the CLI slug rule: lowercase alphanumeric plus interior `-`. */
function isValidProjectName(name) {
    return (name.length > 0 &&
        name.length <= 64 &&
        /^[a-z0-9][a-z0-9-]*[a-z0-9]$|^[a-z0-9]$/.test(name));
}
/** Shell-quote one argument for the terminal scaffold command. */
function shellQuote(argument) {
    if (/^[a-zA-Z0-9_@%+=:,./-]+$/.test(argument) && argument.length > 0) {
        return argument;
    }
    return `'${argument.replace(/'/g, `'\\''`)}'`;
}
/** Build the terminal command scaffolding a project in a directory. */
function buildNewCommand(directory, name, template) {
    return `cd ${shellQuote(directory)} && studio new ${shellQuote(name)} -t ${shellQuote(template)}`;
}
