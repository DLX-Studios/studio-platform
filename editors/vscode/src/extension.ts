// Studio Script VS Code client: thin launcher for the Rust language server.
//
// This module contains no compiler logic. It spawns `studio-language-server`
// over stdio, forwards JSON-RPC messages, runs `studio` binary commands for
// check/build/dev/restart, and degrades to highlighting plus local commands
// when the server is unavailable.
import * as vscode from "vscode";
import { execFile } from "node:child_process";
import { spawn, ChildProcess } from "node:child_process";

import {
  buildNewCommand,
  isValidProjectName,
  parseTemplateList,
} from "./templates";

let server: ChildProcess | null = null;
let degraded = false;

function serverBinary(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace
    .getConfiguration("studio")
    .get<string>("languageServerPath", "");
  if (configured.length > 0) {
    return configured;
  }
  // Alongside the extension install: ../../target/debug/studio-language-server
  // in a checkout, or a bundled binary in a packaged install.
  return context.asAbsolutePath("../../target/debug/studio-language-server");
}

function startServer(context: vscode.ExtensionContext): void {
  try {
    server = spawn(serverBinary(context), [], { stdio: ["pipe", "pipe", "inherit"] });
    server.on("error", () => degrade("server binary missing or not executable"));
    server.on("exit", () => degrade("server process exited"));
  } catch {
    degrade("server spawn threw");
  }
}

function degrade(reason: string): void {
  if (!degraded) {
    degraded = true;
    void vscode.window.showWarningMessage(
      `Studio language features degraded (${reason}); highlighting and local commands still work.`,
    );
  }
  server = null;
}

function sendRequest(method: string, params: unknown): Promise<unknown> {
  if (server?.stdin == null || server?.stdout == null) {
    return Promise.reject(new Error("language server unavailable"));
  }
  return new Promise((resolve, reject) => {
    const id = Math.floor(Math.random() * 1_000_000_000);
    const body = Buffer.from(
      JSON.stringify({ jsonrpc: "2.0", id, method, params }),
      "utf8",
    );
    const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8");
    const child = server;
    if (child?.stdin == null || child?.stdout == null) {
      reject(new Error("language server unavailable"));
      return;
    }
    const chunks: Buffer[] = [];
    const onData = (chunk: Buffer): void => {
      chunks.push(chunk);
      const text = Buffer.concat(chunks).toString("utf8");
      const boundary = text.indexOf("\r\n\r\n");
      if (boundary < 0) {
        return;
      }
      const length = Number.parseInt(
        text.slice(0, boundary).replace("Content-Length:", "").trim(),
        10,
      );
      if (text.length < boundary + 4 + length) {
        return;
      }
      child.stdout?.off("data", onData);
      try {
        const message = JSON.parse(text.slice(boundary + 4, boundary + 4 + length)) as {
          result?: unknown;
          error?: { message?: string };
        };
        if (message.error !== undefined) {
          reject(new Error(message.error.message ?? "server error"));
        } else {
          resolve(message.result ?? null);
        }
      } catch (error) {
        reject(error);
      }
    };
    child.stdout.on("data", onData);
    child.stdin.write(Buffer.concat([header, body]));
  });
}

function activeStudioDocument(): vscode.TextDocument | undefined {
  const document = vscode.window.activeTextEditor?.document;
  return document?.languageId === "studio" ? document : undefined;
}

export function activate(context: vscode.ExtensionContext): void {
  startServer(context);

  context.subscriptions.push(
    vscode.commands.registerCommand("studio.format", async () => {
      const document = activeStudioDocument();
      if (document === undefined) {
        return;
      }
      try {
        const result = (await sendRequest("textDocument/formatting", {
          textDocument: { uri: document.uri.toString() },
        })) as { formatted?: string; diagnostic?: string };
        if (result.formatted !== undefined) {
          const edit = new vscode.WorkspaceEdit();
          edit.replace(
            document.uri,
            new vscode.Range(0, 0, document.lineCount, 0),
            result.formatted,
          );
          await vscode.workspace.applyEdit(edit);
        } else {
          void vscode.window.showWarningMessage(
            `Studio formatting unavailable: ${result.diagnostic ?? "unknown"}`,
          );
        }
      } catch {
        degrade("format request failed");
      }
    }),
    vscode.commands.registerCommand("studio.check", () => {
      void runStudioCommand(["check", "."]);
    }),
    vscode.commands.registerCommand("studio.build", () => {
      void runStudioCommand(["build"]);
    }),
    vscode.commands.registerCommand("studio.dev", () => {
      void runStudioCommand(["dev"]);
    }),
    vscode.commands.registerCommand("studio.restartSession", () => {
      void runStudioCommand(["restart-session"]);
    }),
    vscode.commands.registerCommand("studio.newProject", () => {
      void newProjectFromGallery();
    }),
  );
}

function runStudioCommand(args: string[]): void {
  const terminal =
    vscode.window.terminals.find((candidate) => candidate.name === "studio") ??
    vscode.window.createTerminal("studio");
  terminal.show();
  terminal.sendText(`studio ${args.join(" ")}`);
}

/// Studio binary for gallery reads: configured path, else PATH lookup.
function studioBinary(): string {
  const configured = vscode.workspace
    .getConfiguration("studio")
    .get<string>("cliPath", "");
  return configured.length > 0 ? configured : "studio";
}

async function newProjectFromGallery(): Promise<void> {
  const templates = await readGalleryTemplates();
  if (templates === null) {
    void vscode.window.showWarningMessage(
      "Studio templates unavailable: the `studio` binary is missing or has no gallery.",
    );
    return;
  }
  const picked = await vscode.window.showQuickPick(
    templates.map((template) => ({
      label: template.id,
      description: `[${template.kind}] ${template.description}`,
      id: template.id,
    })),
    { placeHolder: "Pick a Studio template" },
  );
  if (picked === undefined) {
    return;
  }
  const name = await vscode.window.showInputBox({
    prompt: "Project directory name",
    validateInput: (value) =>
      isValidProjectName(value)
        ? undefined
        : "lowercase alphanumeric plus `-`",
  });
  if (name === undefined) {
    return;
  }
  const folders = await vscode.window.showOpenDialog({
    canSelectFiles: false,
    canSelectFolders: true,
    canSelectMany: false,
    openLabel: "Scaffold here",
    defaultUri: vscode.workspace.workspaceFolders?.[0]?.uri,
  });
  if (folders === undefined || folders.length === 0) {
    return;
  }
  const terminal =
    vscode.window.terminals.find((candidate) => candidate.name === "studio") ??
    vscode.window.createTerminal("studio");
  terminal.show();
  terminal.sendText(buildNewCommand(folders[0].fsPath, name, picked.id));
  const open = "Open Folder";
  const choice = await vscode.window.showInformationMessage(
    `Scaffolding ${name} — reopen VS Code inside it to continue.`,
    open,
  );
  if (choice === open) {
    await vscode.commands.executeCommand(
      "vscode.openFolder",
      vscode.Uri.joinPath(folders[0], name),
    );
  }
}

function readGalleryTemplates(): Promise<
  { id: string; kind: string; description: string }[] | null
> {
  return new Promise((resolve) => {
    execFile(
      studioBinary(),
      ["new", "--list-templates", "--format", "json"],
      { timeout: 15_000 },
      (error, stdout) => {
        if (error !== null) {
          resolve(null);
          return;
        }
        try {
          resolve(parseTemplateList(stdout));
        } catch {
          resolve(null);
        }
      },
    );
  });
}

export function deactivate(): void {
  server?.kill();
  server = null;
}
