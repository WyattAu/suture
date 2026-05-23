import * as vscode from "vscode";
import * as path from "path";
import { spawn } from "child_process";

let outputChannel: vscode.OutputChannel;

function getOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    outputChannel = vscode.window.createOutputChannel("Suture");
  }
  return outputChannel;
}

function getConfig(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("suture");
}

function getWorkspaceRoot(): string | undefined {
  if (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0) {
    return vscode.workspace.workspaceFolders[0].uri.fsPath;
  }
  return undefined;
}

function runSuture(args: string[]): Promise<{ stdout: string; stderr: string; code: number }> {
  return new Promise((resolve, reject) => {
    const executablePath = getConfig().get<string>("executablePath") || "suture";
    const cwd = getWorkspaceRoot();

    if (!cwd) {
      reject(new Error("No workspace folder open."));
      return;
    }

    const channel = getOutputChannel();
    channel.appendLine(`$ ${executablePath} ${args.join(" ")}`);

    const proc = spawn(executablePath, args, { cwd });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });

    proc.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });

    proc.on("close", (code) => {
      if (stdout.trim()) {
        channel.appendLine(stdout.trimEnd());
      }
      if (stderr.trim()) {
        channel.appendLine(`[stderr] ${stderr.trimEnd()}`);
      }
      resolve({ stdout, stderr, code: code ?? 1 });
    });

    proc.on("error", (err) => {
      channel.appendLine(`[error] ${err.message}`);
      reject(err);
    });
  });
}

async function runGit(args: string[]): Promise<{ stdout: string; stderr: string; code: number }> {
  return new Promise((resolve, reject) => {
    const cwd = getWorkspaceRoot();
    if (!cwd) {
      reject(new Error("No workspace folder open."));
      return;
    }

    const channel = getOutputChannel();
    channel.appendLine(`$ git ${args.join(" ")}`);

    const proc = spawn("git", args, { cwd });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });

    proc.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });

    proc.on("close", (code) => {
      resolve({ stdout, stderr, code: code ?? 1 });
    });

    proc.on("error", (err) => {
      channel.appendLine(`[error] ${err.message}`);
      reject(err);
    });
  });
}

async function configureMergeDriver(): Promise<boolean> {
  await runGit(["config", "merge.suture.name", "Suture semantic merge"]);
  await runGit(["config", "merge.suture.driver", "suture merge-file --driver %O %A %B -o %A"]);
  return true;
}

async function appendToGitattributes(patterns: string[]): Promise<void> {
  const cwd = getWorkspaceRoot();
  if (!cwd) {
    return;
  }

  const gitattributesPath = path.join(cwd, ".gitattributes");
  const channel = getOutputChannel();

  let existing = "";
  try {
    const uri = vscode.Uri.file(gitattributesPath);
    const bytes = await vscode.workspace.fs.readFile(uri);
    existing = new TextDecoder().decode(bytes);
  } catch {
    existing = "";
  }

  const linesToAdd = patterns.filter((p) => !existing.includes(p));
  if (linesToAdd.length === 0) {
    channel.appendLine(".gitattributes already contains the requested patterns.");
    return;
  }

  const content = (existing.trimEnd() ? existing.trimEnd() + "\n" : "") + linesToAdd.join("\n") + "\n";
  const uri = vscode.Uri.file(gitattributesPath);
  await vscode.workspace.fs.writeFile(uri, new TextEncoder().encode(content));
  channel.appendLine(`Added to .gitattributes:\n${linesToAdd.join("\n")}`);
}

export async function activate(context: vscode.ExtensionContext) {
  const channel = getOutputChannel();
  channel.appendLine("Suture extension activated.");

  const cwd = getWorkspaceRoot();
  if (cwd) {
    try {
      const sutureDir = vscode.Uri.file(path.join(cwd, ".suture"));
      await vscode.workspace.fs.stat(sutureDir);
      if (getConfig().get<boolean>("autoConfigure")) {
        const answer = await vscode.window.showInformationMessage(
          "Suture repository detected. Configure as git merge driver?",
          "Yes", "No"
        );
        if (answer === "Yes") {
          await configureMergeDriver();
          vscode.window.showInformationMessage("Suture configured as git merge driver. Add file patterns to .gitattributes.");
        }
      }
    } catch {
      // .suture directory does not exist, nothing to do
    }
  }

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.configureMergeDriver", async () => {
      try {
        await configureMergeDriver();
        vscode.window.showInformationMessage("Suture configured as git merge driver. Add file patterns to .gitattributes.");
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to configure merge driver: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.configureMergeDriverJson", async () => {
      try {
        await configureMergeDriver();
        await appendToGitattributes(["*.json merge=suture"]);
        vscode.window.showInformationMessage("Semantic merge enabled for JSON files.");
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to configure: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.configureMergeDriverYaml", async () => {
      try {
        await configureMergeDriver();
        await appendToGitattributes(["*.yaml merge=suture", "*.yml merge=suture"]);
        vscode.window.showInformationMessage("Semantic merge enabled for YAML files.");
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to configure: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.configureMergeDriverAll", async () => {
      try {
        await configureMergeDriver();
        await appendToGitattributes([
          "*.json merge=suture",
          "*.yaml merge=suture",
          "*.yml merge=suture",
          "*.toml merge=suture",
          "*.csv merge=suture",
          "*.xml merge=suture",
          "*.md merge=suture",
          "*.html merge=suture",
          "*.svg merge=suture",
          "*.docx merge=suture",
          "*.xlsx merge=suture",
          "*.pptx merge=suture",
          "*.sql merge=suture",
        ]);
        vscode.window.showInformationMessage("Semantic merge enabled for all supported formats.");
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to configure: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.initRepo", async () => {
      try {
        const { stdout } = await runSuture(["init"]);
        channel.appendLine(stdout);
        channel.show();
        vscode.window.showInformationMessage("Suture repository initialized.");
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to initialize: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.status", async () => {
      try {
        const { stdout } = await runSuture(["status"]);
        channel.appendLine(stdout);
        channel.show();
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to get status: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.commit", async () => {
      try {
        const message = await vscode.window.showInputBox({
          prompt: "Enter commit message",
          placeHolder: "Describe your changes...",
        });
        if (!message) {
          return;
        }
        await runSuture(["add", "."]);
        const { stdout } = await runSuture(["commit", message]);
        channel.appendLine(stdout);
        channel.show();
        vscode.window.showInformationMessage("Changes committed.");
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to commit: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.log", async () => {
      try {
        const { stdout } = await runSuture(["log", "--oneline", "-20"]);
        channel.appendLine(stdout);
        channel.show();
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to get log: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.diff", async () => {
      try {
        const { stdout } = await runSuture(["diff"]);
        channel.appendLine(stdout);
        channel.show();
      } catch (err: any) {
        vscode.window.showErrorMessage(`Failed to get diff: ${err.message}`);
        channel.show();
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("suture.mergePreview", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.showWarningMessage("No active editor.");
        return;
      }

      const doc = editor.document;
      const text = doc.getText();
      const conflicts = parseConflicts(text);

      if (conflicts.length === 0) {
        vscode.window.showInformationMessage("No merge conflicts found in this file.");
        return;
      }

      const panel = vscode.window.createWebviewPanel(
        "sutureMergePreview",
        `Merge Preview — ${path.basename(doc.fileName)}`,
        vscode.ViewColumn.Beside,
        { enableScripts: true }
      );

      panel.webview.html = renderMergePreview(conflicts);

      panel.webview.onDidReceiveMessage(async (msg: { action: string; index: number }) => {
        const resolved = resolveConflict(conflicts[msg.index], msg.action);
        if (!resolved) {
          return;
        }

        conflicts[msg.index] = { ...conflicts[msg.index], resolved };

        const fullText = doc.getText();
        const newText = applyResolutions(fullText, conflicts);
        const edit = new vscode.WorkspaceEdit();
        const fullRange = new vscode.Range(doc.positionAt(0), doc.positionAt(fullText.length));
        edit.replace(doc.uri, fullRange, newText);
        await vscode.workspace.applyEdit(edit);

        panel.webview.html = renderMergePreview(conflicts);

        if (conflicts.every((c) => c.resolved !== undefined)) {
          vscode.window.showInformationMessage("All conflicts resolved.");
          panel.dispose();
        }
      });
    })
  );
}

interface ConflictBlock {
  start: number;
  end: number;
  ours: string;
  base: string;
  theirs: string;
  resolved?: string;
}

function parseConflicts(text: string): ConflictBlock[] {
  const conflicts: ConflictBlock[] = [];
  const lines = text.split("\n");
  let i = 0;

  while (i < lines.length) {
    if (lines[i].startsWith("<<<<<<<")) {
      const start = i;
      i++;
      const oursLines: string[] = [];
      while (i < lines.length && !lines[i].startsWith("=======")) {
        oursLines.push(lines[i]);
        i++;
      }

      i++; // skip =======

      let baseLines: string[] = [];
      if (i < lines.length && lines[i].startsWith("|||||||")) {
        i++;
        while (i < lines.length && !lines[i].startsWith("=======")) {
          baseLines.push(lines[i]);
          i++;
        }
        i++; // skip second =======
      }

      const theirsLines: string[] = [];
      while (i < lines.length && !lines[i].startsWith(">>>>>>>")) {
        theirsLines.push(lines[i]);
        i++;
      }

      if (i < lines.length) {
        i++; // skip >>>>>>>
      }

      conflicts.push({
        start,
        end: i,
        ours: oursLines.join("\n"),
        base: baseLines.join("\n"),
        theirs: theirsLines.join("\n"),
      });
    } else {
      i++;
    }
  }

  return conflicts;
}

function resolveConflict(conflict: ConflictBlock, action: string): string | undefined {
  switch (action) {
    case "ours":
      return conflict.ours;
    case "theirs":
      return conflict.theirs;
    case "both":
      return conflict.ours + (conflict.ours && conflict.theirs ? "\n" : "") + conflict.theirs;
    default:
      return undefined;
  }
}

function applyResolutions(text: string, conflicts: ConflictBlock[]): string {
  const lines = text.split("\n");
  const sorted = [...conflicts].sort((a, b) => b.start - a.start);

  for (const conflict of sorted) {
    if (conflict.resolved === undefined) {
      continue;
    }
    const replacement = conflict.resolved.split("\n");
    lines.splice(conflict.start, conflict.end - conflict.start, ...replacement);
  }

  return lines.join("\n");
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function renderMergePreview(conflicts: ConflictBlock[]): string {
  const sections = conflicts.map((c, i) => {
    const status = c.resolved !== undefined
      ? `<div style="padding:8px;background:#1a4a1a;color:#4f4;color:var(--vscode-editor-foreground);border-radius:4px;margin-bottom:12px;">Resolved</div>`
      : "";

    const baseSection = c.base
      ? `<div style="flex:1;"><div style="font-weight:bold;margin-bottom:4px;color:#888;">Base</div>
         <pre style="background:var(--vscode-textBlockQuote-background, #2a2a2a);padding:8px;border-radius:4px;white-space:pre-wrap;margin:0;min-height:40px;">${escapeHtml(c.base)}</pre></div>`
      : "";

    return `
      <div style="margin-bottom:16px;border:1px solid var(--vscode-panel-border, #444);border-radius:6px;padding:12px;">
        <div style="margin-bottom:8px;font-weight:bold;">Conflict ${i + 1} of ${conflicts.length}</div>
        ${status}
        <div style="display:flex;gap:8px;margin-bottom:8px;">
          <div style="flex:1;"><div style="font-weight:bold;margin-bottom:4px;color:#6a6;">Ours</div>
            <pre style="background:var(--vscode-diffEditor-insertedTextBackground, rgba(40,80,40,0.4));padding:8px;border-radius:4px;white-space:pre-wrap;margin:0;min-height:40px;">${escapeHtml(c.ours)}</pre></div>
          ${baseSection}
          <div style="flex:1;"><div style="font-weight:bold;margin-bottom:4px;color:#66a;">Theirs</div>
            <pre style="background:var(--vscode-diffEditor-removedTextBackground, rgba(80,40,80,0.4));padding:8px;border-radius:4px;white-space:pre-wrap;margin:0;min-height:40px;">${escapeHtml(c.theirs)}</pre></div>
        </div>
        ${c.resolved === undefined ? `
        <div style="display:flex;gap:8px;">
          <button onclick="resolve(${i},'ours')" style="flex:1;padding:6px;cursor:pointer;background:var(--vscode-button-background, #0e639c);color:var(--vscode-button-foreground, #fff);border:none;border-radius:3px;">Accept Ours</button>
          <button onclick="resolve(${i},'theirs')" style="flex:1;padding:6px;cursor:pointer;background:var(--vscode-button-secondaryBackground, #3a3d41);color:var(--vscode-button-secondaryForeground, #fff);border:none;border-radius:3px;">Accept Theirs</button>
          <button onclick="resolve(${i},'both')" style="flex:1;padding:6px;cursor:pointer;background:var(--vscode-button-background, #0e639c);color:var(--vscode-button-foreground, #fff);border:none;border-radius:3px;">Accept Both</button>
        </div>` : ""}
      </div>`;
  });

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Merge Preview</title>
</head>
<body style="font-family:var(--vscode-font-family,sans-serif);padding:16px;color:var(--vscode-editor-foreground,#ccc);background:var(--vscode-editor-background,#1e1e1e);">
  <h2 style="margin-top:0;">Merge Conflict Preview</h2>
  ${sections.join("")}
  <script>
    const vscode = acquireVsCodeApi();
    function resolve(index, action) {
      vscode.postMessage({ action, index });
    }
  </script>
</body>
</html>`;
}

export function deactivate() {
  if (outputChannel) {
    outputChannel.dispose();
  }
}
