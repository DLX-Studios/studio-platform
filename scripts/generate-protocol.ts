import { mkdir, readdir, unlink } from "node:fs/promises";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const schemaDirectory = join(root, "protocol/schemas/protocol-v1");
const fixtureDirectories = [join(root, "protocol/fixtures/protocol-v1/valid")];
const bindingPath = join(root, "sdk/assemblyscript/assembly/generated/protocol.ts");

const schemaFiles = [
  "action-request.schema.json",
  "action-result.schema.json",
  "guest-message.schema.json",
  "host-event.schema.json",
  "mount-tree.schema.json",
  "navigation-command.schema.json",
  "patch-batch.schema.json",
] as const;

const groups = {
  GUEST_MESSAGE: ["mount", "patch", "navigate", "action", "log"],
  HOST_EVENT: ["ui", "navigation", "action_result", "lifecycle"],
  PATCH_OPERATION: [
    "update_prop",
    "insert_child",
    "remove_node",
    "replace_node",
  ],
  NAVIGATION_OPERATION: ["push", "replace", "pop", "pop_to", "reset"],
  ACTION_STATUS: ["success", "failure"],
  LIFECYCLE_STATE: ["loading", "running", "trapped", "stopped"],
  NODE_KIND: [
    "box",
    "column",
    "row",
    "stack",
    "grid",
    "scroll_view",
    "list_view",
    "spacer",
    "divider",
    "text",
    "icon",
    "image",
    "card",
    "badge",
    "progress_indicator",
    "button",
    "icon_button",
    "checkbox",
    "switch",
    "slider",
    "select",
    "text_input",
    "secret_input",
    "dialog",
    "bottom_sheet",
    "toast",
    "tooltip",
  ],
} as const;

function collectWireStrings(value: unknown, output: Set<string>): void {
  if (Array.isArray(value)) {
    for (const child of value) collectWireStrings(child, output);
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (key === "const" && typeof child === "string") output.add(child);
    if (key === "enum" && Array.isArray(child)) {
      for (const item of child) if (typeof item === "string") output.add(item);
    }
    collectWireStrings(child, output);
  }
}

function constantName(value: string): string {
  return value.toUpperCase().replaceAll(/[^A-Z0-9]+/g, "_");
}

const observedWireStrings = new Set<string>();
for (const filename of schemaFiles) {
  const schema = await Bun.file(join(schemaDirectory, filename)).json();
  collectWireStrings(schema, observedWireStrings);
}

for (const values of Object.values(groups)) {
  for (const value of values) {
    if (!observedWireStrings.has(value)) {
      throw new Error(`Rust schema inventory is missing wire discriminant: ${value}`);
    }
  }
}

const constants = Object.entries(groups).flatMap(([group, values]) =>
  values.map(
    (value) =>
      `export const ${group}_${constantName(value)}: string = ${JSON.stringify(value)};`,
  ),
);
const bindings = `// Generated from Rust-authoritative protocol-v1 schemas. Do not edit.\n\n/** Host–guest protocol major version. */\nexport const PROTOCOL_VERSION: u16 = 1;\n\n${constants.join("\n")}\n\n/** A copied JSON envelope ready for the Studio ABI boundary. */\nexport class ProtocolEnvelopeV1 {\n  constructor(\n    public readonly type: string,\n    public readonly payloadJson: string,\n  ) {}\n}\n`;

const fixtures = {
  "guest-action.json": {
    type: "action",
    payload: {
      request_id: "req-1",
      capability: "payment.simulate",
      operation: "charge",
      payload: {},
    },
  },
  "guest-log.json": {
    type: "log",
    payload: { level: "info", message: "Catalog mounted" },
  },
  "guest-mount.json": {
    type: "mount",
    payload: {
      protocol_version: 1,
      route: "/catalog",
      root: {
        id: "root",
        kind: "column",
        props: {},
        children: [{ id: "title", kind: "text", props: {}, children: [] }],
      },
    },
  },
  "guest-navigate.json": {
    type: "navigate",
    payload: { operation: "push", route: "/checkout" },
  },
  "guest-patch.json": {
    type: "patch",
    payload: {
      sequence: 1,
      operations: [
        {
          op: "update_prop",
          node_id: "title",
          property: "text",
          value: "Catalog",
        },
      ],
    },
  },
  "host-action-result-failure.json": {
    type: "action_result",
    payload: {
      status: "failure",
      request_id: "req-2",
      code: "payment_declined",
      message: "Payment was declined",
      retryable: false,
    },
  },
  "host-action-result-success.json": {
    type: "action_result",
    payload: {
      status: "success",
      request_id: "req-1",
      payload: { result_ref: "result-1" },
    },
  },
  "host-lifecycle.json": {
    type: "lifecycle",
    payload: { state: "running", message: null },
  },
  "host-navigation.json": {
    type: "navigation",
    payload: { route: "/checkout", accepted: true, error_code: null },
  },
  "host-ui.json": {
    type: "ui",
    payload: { node_id: "checkout", event: "pressed", payload: {} },
  },
} as const;

await mkdir(join(bindingPath, ".."), { recursive: true });
await Bun.write(bindingPath, bindings);
const expectedFixtureFiles = Object.keys(fixtures).sort();
for (const fixtureDirectory of fixtureDirectories) {
  await mkdir(fixtureDirectory, { recursive: true });
  for (const filename of await readdir(fixtureDirectory)) {
    if (filename.endsWith(".json") && !expectedFixtureFiles.includes(filename)) {
      await unlink(join(fixtureDirectory, filename));
    }
  }
  for (const [filename, fixture] of Object.entries(fixtures).sort(([left], [right]) =>
    left.localeCompare(right),
  )) {
    await Bun.write(
      join(fixtureDirectory, filename),
      `${JSON.stringify(fixture, null, 2)}\n`,
    );
  }
}

for (const fixtureDirectory of fixtureDirectories) {
  const generatedFixtureFiles = (await readdir(fixtureDirectory))
    .filter((filename) => filename.endsWith(".json"))
    .sort();
  if (JSON.stringify(generatedFixtureFiles) !== JSON.stringify(expectedFixtureFiles)) {
    throw new Error(`valid fixture directory contains stale generated JSON files: ${fixtureDirectory}`);
  }
}
