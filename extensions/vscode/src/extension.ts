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
}

export function deactivate() {
  if (outputChannel) {
    outputChannel.dispose();
  }
}
