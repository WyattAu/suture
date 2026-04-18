import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { execFile, ExecFileException } from 'child_process';

class SutureHelper {
  private outputChannel: vscode.OutputChannel;

  constructor(outputChannel: vscode.OutputChannel) {
    this.outputChannel = outputChannel;
  }

  findSutureBinary(): string | null {
    const candidates = [
      'suture',
      path.join(process.env.HOME || '', '.cargo', 'bin', 'suture'),
      '/usr/local/bin/suture',
    ];

    for (const candidate of candidates) {
      if (fs.existsSync(candidate)) {
        return candidate;
      }
    }

    return null;
  }

  exec(command: string, args: string[], cwd?: string): Promise<string> {
    const binary = this.findSutureBinary();
    if (!binary) {
      return Promise.reject(new Error('Suture binary not found'));
    }

    return new Promise<string>((resolve, reject) => {
      execFile(binary, [command, ...args], { cwd, maxBuffer: 10 * 1024 * 1024 }, (error: ExecFileException | null, stdout: string, stderr: string) => {
        if (error) {
          reject(new Error(stderr || error.message));
        } else {
          resolve(stdout.trim());
        }
      });
    });
  }

  async isSutureRepo(dir: string): Promise<boolean> {
    return fs.existsSync(path.join(dir, '.suture'));
  }

  showOutput(text: string): void {
    this.outputChannel.clear();
    this.outputChannel.appendLine(text);
    this.outputChannel.show();
  }

  getWorkspaceRoot(): string | undefined {
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
  }

  getActiveFile(): vscode.Uri | undefined {
    return vscode.window.activeTextEditor?.document.uri;
  }
}

export function activate(context: vscode.ExtensionContext) {
  const outputChannel = vscode.window.createOutputChannel('Suture');
  context.subscriptions.push(outputChannel);

  const suture = new SutureHelper(outputChannel);

  if (!suture.findSutureBinary()) {
    vscode.window.showWarningMessage(
      'Suture binary not found. Please install Suture and ensure it is in your PATH.'
    );
  }

  function registerCommand(command: string, callback: (...args: unknown[]) => Promise<void>) {
    context.subscriptions.push(
      vscode.commands.registerCommand(command, callback)
    );
  }

  registerCommand('suture.init', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      await suture.exec('init', [], root);
      vscode.window.showInformationMessage('Suture repository initialized.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Failed to initialize: ${msg}`);
    }
  });

  registerCommand('suture.status', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      const output = await suture.exec('status', [], root);
      suture.showOutput(output || 'Nothing to commit, working tree clean.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Status failed: ${msg}`);
    }
  });

  registerCommand('suture.add', async () => {
    const fileUri = suture.getActiveFile();
    const root = suture.getWorkspaceRoot();
    if (!fileUri || !root) {
      vscode.window.showErrorMessage('No active file or workspace.');
      return;
    }

    const relativePath = path.relative(root, fileUri.fsPath);
    try {
      await suture.exec('add', [relativePath], root);
      vscode.window.showInformationMessage(`Staged: ${relativePath}`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Stage failed: ${msg}`);
    }
  });

  registerCommand('suture.addAll', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      await suture.exec('add', ['.'], root);
      vscode.window.showInformationMessage('All files staged.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Stage all failed: ${msg}`);
    }
  });

  registerCommand('suture.commit', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    const message = await vscode.window.showInputBox({
      prompt: 'Commit message',
      placeHolder: 'Enter commit message...',
    });

    if (!message) {
      return;
    }

    try {
      await suture.exec('commit', ['-m', message], root);
      vscode.window.showInformationMessage('Committed successfully.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Commit failed: ${msg}`);
    }
  });

  registerCommand('suture.push', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      await suture.exec('push', [], root);
      vscode.window.showInformationMessage('Pushed successfully.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Push failed: ${msg}`);
    }
  });

  registerCommand('suture.pull', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      await suture.exec('pull', [], root);
      vscode.window.showInformationMessage('Pulled successfully.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Pull failed: ${msg}`);
    }
  });

  registerCommand('suture.branches', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      const output = await suture.exec('branch', [], root);
      const branches = output.split('\n').filter(b => b.trim() !== '');
      if (branches.length === 0) {
        vscode.window.showInformationMessage('No branches found.');
        return;
      }

      const selected = await vscode.window.showQuickPick(branches, {
        placeHolder: 'Select a branch to checkout',
      });

      if (selected) {
        await suture.exec('checkout', [selected.trim()], root);
        vscode.window.showInformationMessage(`Switched to: ${selected.trim()}`);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Branches failed: ${msg}`);
    }
  });

  registerCommand('suture.log', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      const output = await suture.exec('log', ['-n', '20'], root);
      suture.showOutput(output || 'No commits found.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Log failed: ${msg}`);
    }
  });

  registerCommand('suture.diff', async () => {
    const root = suture.getWorkspaceRoot();
    const fileUri = suture.getActiveFile();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    const args: string[] = [];
    if (fileUri) {
      const relativePath = path.relative(root, fileUri.fsPath);
      args.push(relativePath);
    }

    try {
      const output = await suture.exec('diff', args, root);
      suture.showOutput(output || 'No differences.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Diff failed: ${msg}`);
    }
  });

  registerCommand('suture.checkout', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      const output = await suture.exec('branch', [], root);
      const branches = output.split('\n').filter(b => b.trim() !== '');
      if (branches.length === 0) {
        vscode.window.showInformationMessage('No branches found.');
        return;
      }

      const selected = await vscode.window.showQuickPick(branches, {
        placeHolder: 'Select a branch to checkout',
      });

      if (selected) {
        await suture.exec('checkout', [selected.trim()], root);
        vscode.window.showInformationMessage(`Checked out: ${selected.trim()}`);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Checkout failed: ${msg}`);
    }
  });

  registerCommand('suture.blame', async () => {
    const root = suture.getWorkspaceRoot();
    const fileUri = suture.getActiveFile();
    if (!root || !fileUri) {
      vscode.window.showErrorMessage('No active file or workspace.');
      return;
    }

    const relativePath = path.relative(root, fileUri.fsPath);
    try {
      const output = await suture.exec('blame', [relativePath], root);
      suture.showOutput(output || 'No blame information available.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Blame failed: ${msg}`);
    }
  });

  registerCommand('suture.stash', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    const action = await vscode.window.showQuickPick(['push', 'pop', 'list'], {
      placeHolder: 'Select stash action',
    });

    if (!action) {
      return;
    }

    try {
      const output = await suture.exec('stash', [action], root);
      if (action === 'list') {
        suture.showOutput(output || 'No stashes found.');
      } else {
        vscode.window.showInformationMessage(`Stash ${action} successful.`);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Stash ${action} failed: ${msg}`);
    }
  });

  registerCommand('suture.tags', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    try {
      const output = await suture.exec('tag', [], root);
      suture.showOutput(output || 'No tags found.');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Tags failed: ${msg}`);
    }
  });
}

export function deactivate() {}
