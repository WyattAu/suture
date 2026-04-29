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

interface ConflictBlock {
  ours: string;
  theirs: string;
  header: string;
  footer: string;
  startIndex: number;
  endIndex: number;
}

const CONFLICT_PATTERN = /^<{7}\s+(.*?)\n([\s\S]*?)^={7}\n([\s\S]*?)^>{7}\s+(.*?)$/gm;

function findConflicts(text: string): ConflictBlock[] {
  const conflicts: ConflictBlock[] = [];
  let match: RegExpExecArray | null;

  while ((match = CONFLICT_PATTERN.exec(text)) !== null) {
    const startIndex = match.index;
    const endIndex = startIndex + match[0].length;
    const ours = match[2].replace(/\n$/, '');
    const theirs = match[3].replace(/\n$/, '');
    const header = match[0].split('\n')[0];
    const footer = match[0].split('\n').pop() || '';

    conflicts.push({ ours, theirs, header, footer, startIndex, endIndex });
  }

  return conflicts;
}

function resolveConflicts(text: string, conflicts: ConflictBlock[], resolution: 'ours' | 'theirs' | 'both'): string {
  let result = text;
  for (let i = conflicts.length - 1; i >= 0; i--) {
    const conflict = conflicts[i];
    let replacement: string;
    if (resolution === 'ours') {
      replacement = conflict.ours;
    } else if (resolution === 'theirs') {
      replacement = conflict.theirs;
    } else {
      replacement = conflict.ours + '\n' + conflict.theirs;
    }
    result = result.substring(0, conflict.startIndex) + replacement + result.substring(conflict.endIndex);
  }
  return result;
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

  const statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  statusBarItem.text = '$(git-merge) Suture: checking...';
  statusBarItem.tooltip = 'Suture merge status';
  statusBarItem.command = 'suture.showMergeStatus';
  context.subscriptions.push(statusBarItem);
  statusBarItem.show();

  async function updateMergeStatus() {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      statusBarItem.text = '$(git-merge) Suture: no workspace';
      statusBarItem.color = undefined;
      return;
    }

    if (!await suture.isSutureRepo(root)) {
      statusBarItem.text = '$(git-merge) Suture: no repo';
      statusBarItem.color = undefined;
      return;
    }

    try {
      const output = await suture.exec('status', [], root);
      const conflictMatch = output.match(/(\d+)\s+conflict/i);
      if (conflictMatch) {
        const count = parseInt(conflictMatch[1], 10);
        statusBarItem.text = `$(git-merge) Suture: ${count} conflict${count !== 1 ? 's' : ''}`;
        statusBarItem.color = new vscode.ThemeColor('statusBarItem.errorForeground');
      } else {
        statusBarItem.text = '$(git-merge) Suture: no conflicts';
        statusBarItem.color = new vscode.ThemeColor('statusBarItem.warningForeground');
      }
    } catch {
      statusBarItem.text = '$(git-merge) Suture: unknown';
      statusBarItem.color = undefined;
    }
  }

  updateMergeStatus();
  const statusInterval = setInterval(updateMergeStatus, 30000);
  context.subscriptions.push({ dispose: () => clearInterval(statusInterval) });

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

  registerCommand('suture.resolveConflict', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showErrorMessage('No active editor.');
      return;
    }

    const document = editor.document;
    const text = document.getText();
    const conflicts = findConflicts(text);

    if (conflicts.length === 0) {
      vscode.window.showInformationMessage('No Suture conflict markers found in the current file.');
      return;
    }

    const selected = await vscode.window.showQuickPick(
      [
        { label: 'Ours', description: `Keep ${conflicts.length} "ours" section(s)`, value: 'ours' as const },
        { label: 'Theirs', description: `Keep ${conflicts.length} "theirs" section(s)`, value: 'theirs' as const },
        { label: 'Both', description: `Keep both sections for all ${conflicts.length} conflict(s)`, value: 'both' as const },
      ],
      { placeHolder: `Resolve ${conflicts.length} conflict(s)` }
    );

    if (!selected) {
      return;
    }

    const resolved = resolveConflicts(text, conflicts, selected.value);
    const edit = new vscode.WorkspaceEdit();
    const fullRange = new vscode.Range(
      document.positionAt(0),
      document.positionAt(text.length)
    );
    edit.replace(document.uri, fullRange, resolved);
    await vscode.workspace.applyEdit(edit);
    vscode.window.showInformationMessage(`Resolved ${conflicts.length} conflict(s) using "${selected.label}".`);
    updateMergeStatus();
  });

  registerCommand('suture.mergeCurrentFile', async () => {
    const fileUri = suture.getActiveFile();
    const root = suture.getWorkspaceRoot();
    if (!fileUri || !root) {
      vscode.window.showErrorMessage('No active file or workspace.');
      return;
    }

    const relativePath = path.relative(root, fileUri.fsPath);

    try {
      const statusOutput = await suture.exec('status', [], root);
      const statusLines = statusOutput.split('\n');
      const fileMentioned = statusLines.some(
        line => line.toLowerCase().includes('conflict') && line.includes(relativePath)
      );

      if (!fileMentioned) {
        const inStatus = statusLines.some(line => line.includes(relativePath));
        if (!inStatus) {
          vscode.window.showInformationMessage(`${relativePath} is not listed as an unmerged file.`);
        } else {
          vscode.window.showInformationMessage(`${relativePath} does not appear to be in conflict.`);
        }
        return;
      }

      const mergeOutput = await suture.exec('merge', [relativePath], root);
      const doc = await vscode.workspace.openTextDocument({
        content: mergeOutput,
        language: 'suture-conflict',
      });
      await vscode.window.showTextDocument(doc, { preview: true });
      vscode.window.showInformationMessage(`Merge result for ${relativePath} opened.`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      vscode.window.showErrorMessage(`Merge failed: ${msg}`);
    }
  });

  registerCommand('suture.showMergeStatus', async () => {
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
}

export function deactivate() {}
