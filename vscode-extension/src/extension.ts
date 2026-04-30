import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as http from 'http';
import * as https from 'https';
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

const DRIVER_MAP: Record<string, string> = {
  '.json': 'json',
  '.yaml': 'yaml',
  '.yml': 'yaml',
  '.toml': 'toml',
  '.xml': 'xml',
  '.csv': 'csv',
};

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
  CONFLICT_PATTERN.lastIndex = 0;
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

class ConflictDecorationProvider implements vscode.Disposable {
  private oursDecoration: vscode.TextEditorDecorationType;
  private theirsDecoration: vscode.TextEditorDecorationType;
  private separatorDecoration: vscode.TextEditorDecorationType;
  private lastEditor: vscode.TextEditor | undefined;

  constructor() {
    this.oursDecoration = vscode.window.createTextEditorDecorationType({
      backgroundColor: new vscode.ThemeColor('suture.conflictOursBackground'),
      isWholeLine: true,
      overviewRulerColor: new vscode.ThemeColor('suture.conflictOursOverviewRuler'),
      overviewRulerLane: vscode.OverviewRulerLane.Left,
    });
    this.theirsDecoration = vscode.window.createTextEditorDecorationType({
      backgroundColor: new vscode.ThemeColor('suture.conflictTheirsBackground'),
      isWholeLine: true,
      overviewRulerColor: new vscode.ThemeColor('suture.conflictTheirsOverviewRuler'),
      overviewRulerLane: vscode.OverviewRulerLane.Left,
    });
    this.separatorDecoration = vscode.window.createTextEditorDecorationType({
      backgroundColor: new vscode.ThemeColor('suture.conflictSeparatorBackground'),
      isWholeLine: true,
    });
  }

  update(editor: vscode.TextEditor) {
    this.clear();
    this.lastEditor = editor;

    const document = editor.document;
    const oursRanges: vscode.Range[] = [];
    const theirsRanges: vscode.Range[] = [];
    const separatorRanges: vscode.Range[] = [];
    let inConflict = false;
    let inOurs = false;

    for (let i = 0; i < document.lineCount; i++) {
      const line = document.lineAt(i);
      if (line.text.startsWith('<<<<<<<')) {
        oursRanges.push(new vscode.Range(i, 0, i, line.text.length));
        inConflict = true;
        inOurs = true;
      } else if (line.text.startsWith('=======')) {
        separatorRanges.push(new vscode.Range(i, 0, i, line.text.length));
        inOurs = false;
      } else if (line.text.startsWith('>>>>>>>')) {
        theirsRanges.push(new vscode.Range(i, 0, i, line.text.length));
        inConflict = false;
      } else if (inConflict) {
        if (inOurs) {
          oursRanges.push(new vscode.Range(i, 0, i, line.text.length));
        } else {
          theirsRanges.push(new vscode.Range(i, 0, i, line.text.length));
        }
      }
    }

    editor.setDecorations(this.oursDecoration, oursRanges);
    editor.setDecorations(this.theirsDecoration, theirsRanges);
    editor.setDecorations(this.separatorDecoration, separatorRanges);
  }

  clear() {
    if (this.lastEditor) {
      const empty: vscode.Range[] = [];
      this.lastEditor.setDecorations(this.oursDecoration, empty);
      this.lastEditor.setDecorations(this.theirsDecoration, empty);
      this.lastEditor.setDecorations(this.separatorDecoration, empty);
      this.lastEditor = undefined;
    }
  }

  dispose() {
    this.clear();
    this.oursDecoration.dispose();
    this.theirsDecoration.dispose();
    this.separatorDecoration.dispose();
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

  const conflictStatusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  conflictStatusBarItem.text = '$(git-merge) Suture';
  conflictStatusBarItem.command = 'suture.resolveConflict';
  conflictStatusBarItem.tooltip = 'Resolve merge conflicts with Suture';
  context.subscriptions.push(conflictStatusBarItem);
  conflictStatusBarItem.show();

  const decorationProvider = new ConflictDecorationProvider();
  context.subscriptions.push(decorationProvider);

  function updateEditorConflicts(editor?: vscode.TextEditor) {
    if (editor) {
      const text = editor.document.getText();
      const hasConflicts = /<<<<<<< /.test(text);
      const config = vscode.workspace.getConfiguration('suture');

      if (hasConflicts) {
        conflictStatusBarItem.text = '$(git-merge) Suture: $(alert) Conflicts';
        conflictStatusBarItem.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
        if (config.get<boolean>('highlightConflicts') !== false) {
          decorationProvider.update(editor);
        }
      } else {
        conflictStatusBarItem.text = '$(git-merge) Suture';
        conflictStatusBarItem.backgroundColor = undefined;
        decorationProvider.clear();
      }
    } else {
      conflictStatusBarItem.text = '$(git-merge) Suture';
      conflictStatusBarItem.backgroundColor = undefined;
      decorationProvider.clear();
    }
  }

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor(editor => {
      updateEditorConflicts(editor);
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument(event => {
      if (vscode.window.activeTextEditor && event.document === vscode.window.activeTextEditor.document) {
        updateEditorConflicts(vscode.window.activeTextEditor);
      }
    })
  );

  context.subscriptions.push(
    vscode.workspace.onWillSaveTextDocument(event => {
      const config = vscode.workspace.getConfiguration('suture');
      if (!config.get<boolean>('autoResolveOnSave')) return;

      const document = event.document;
      const text = document.getText();
      const conflicts = findConflicts(text);
      if (conflicts.length === 0) return;

      const ext = path.extname(document.fileName).toLowerCase();
      const driver = DRIVER_MAP[ext];
      if (!driver) return;

      const drivers = config.get<string[]>('autoResolveDrivers', ['json', 'yaml', 'toml']);
      if (!drivers.includes(driver)) return;

      const resolution = config.get<string>('autoResolveStrategy', 'ours') as 'ours' | 'theirs' | 'both';
      const resolved = resolveConflicts(text, conflicts, resolution);

      const fullRange = new vscode.Range(
        document.positionAt(0),
        document.positionAt(text.length)
      );

      event.waitUntil(Promise.resolve([vscode.TextEdit.replace(fullRange, resolved)]));
    })
  );

  if (vscode.window.activeTextEditor) {
    updateEditorConflicts(vscode.window.activeTextEditor);
  }

  function registerCommand(command: string, callback: (...args: unknown[]) => Promise<void>) {
    context.subscriptions.push(
      vscode.commands.registerCommand(command, callback)
    );
  }

  async function showConflictResolutionPicker(): Promise<string | undefined> {
    const options: Array<{ label: string; description: string; action: string }> = [
      { label: '$(check) Auto-merge (Suture)', description: 'Use semantic merge to resolve', action: 'auto' },
      { label: '$(arrow-right) Keep Ours', description: 'Accept current branch changes', action: 'ours' },
      { label: '$(arrow-left) Keep Theirs', description: 'Accept incoming branch changes', action: 'theirs' },
      { label: '$(diff) Keep Both', description: 'Concatenate both versions', action: 'both' },
      { label: '$(eye) Open Merge Demo', description: 'Open Suture web merge tool', action: 'demo' },
    ];

    const pick = await vscode.window.showQuickPick(options, {
      placeHolder: 'How would you like to resolve this merge conflict?',
    });

    return pick?.action;
  }

  function mergeViaApi(apiUrl: string, conflicts: ConflictBlock[], driver: string): Promise<string[]> {
    return new Promise((resolve, reject) => {
      const body = JSON.stringify({
        driver,
        conflicts: conflicts.map(c => ({ ours: c.ours, theirs: c.theirs })),
      });

      const url = new URL(apiUrl);
      const transport = url.protocol === 'https:' ? https : http;
      const reqOptions = {
        hostname: url.hostname,
        port: url.port || (url.protocol === 'https:' ? 443 : 80),
        path: url.pathname + url.search,
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': Buffer.byteLength(body),
        },
        timeout: 10000,
      };

      const req = transport.request(reqOptions, (res) => {
        let data = '';
        res.on('data', (chunk: Buffer) => { data += chunk.toString(); });
        res.on('end', () => {
          if (res.statusCode === 200) {
            try {
              const parsed = JSON.parse(data);
              if (Array.isArray(parsed.results)) {
                resolve(parsed.results);
              } else if (Array.isArray(parsed.merged)) {
                resolve(parsed.merged);
              } else if (typeof parsed.result === 'string') {
                resolve([parsed.result]);
              } else {
                reject(new Error('Unexpected API response format'));
              }
            } catch {
              reject(new Error('Invalid JSON response from API'));
            }
          } else {
            reject(new Error(`API returned status ${res.statusCode}`));
          }
        });
      });

      req.on('error', reject);
      req.on('timeout', () => {
        req.destroy();
        reject(new Error('API request timed out'));
      });
      req.write(body);
      req.end();
    });
  }

  async function resolveWithSutureAuto(fileUri: vscode.Uri, conflicts: ConflictBlock[]): Promise<boolean> {
    const root = suture.getWorkspaceRoot();
    const relativePath = root ? path.relative(root, fileUri.fsPath) : null;

    if (root && relativePath && suture.findSutureBinary()) {
      try {
        const output = await suture.exec('merge', [relativePath], root);
        if (output && !/<<<<<<< /.test(output)) {
          const doc = vscode.workspace.textDocuments.find(d => d.uri.toString() === fileUri.toString());
          if (doc && !doc.isClosed) {
            const text = doc.getText();
            const edit = new vscode.WorkspaceEdit();
            edit.replace(fileUri, new vscode.Range(doc.positionAt(0), doc.positionAt(text.length)), output);
            await vscode.workspace.applyEdit(edit);
          } else {
            fs.writeFileSync(fileUri.fsPath, output, 'utf-8');
          }
          return true;
        }
      } catch {
        // fallthrough to API
      }
    }

    const config = vscode.workspace.getConfiguration('suture');
    const apiUrl = config.get<string>('apiUrl');
    if (apiUrl) {
      try {
        const ext = path.extname(fileUri.fsPath).toLowerCase();
        const driver = DRIVER_MAP[ext];
        if (!driver) return false;

        const merged = await mergeViaApi(apiUrl, conflicts, driver);
        if (merged && merged.length > 0) {
          const doc = vscode.workspace.textDocuments.find(d => d.uri.toString() === fileUri.toString());
          const text = doc ? doc.getText() : fs.readFileSync(fileUri.fsPath, 'utf-8');
          let result = text;
          for (let i = conflicts.length - 1; i >= 0; i--) {
            if (i < merged.length) {
              result = result.substring(0, conflicts[i].startIndex) + merged[i] + result.substring(conflicts[i].endIndex);
            }
          }
          if (doc && !doc.isClosed) {
            const edit = new vscode.WorkspaceEdit();
            edit.replace(fileUri, new vscode.Range(doc.positionAt(0), doc.positionAt(text.length)), result);
            await vscode.workspace.applyEdit(edit);
          } else {
            fs.writeFileSync(fileUri.fsPath, result, 'utf-8');
          }
          return true;
        }
      } catch {
        // fallthrough
      }
    }

    return false;
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
      vscode.window.showInformationMessage('No merge conflicts found in this file.');
      return;
    }

    const action = await showConflictResolutionPicker();
    if (!action) return;

    if (action === 'demo') {
      vscode.env.openExternal(vscode.Uri.parse('https://suture.dev/#/merge'));
      return;
    }

    if (action === 'auto') {
      const success = await resolveWithSutureAuto(document.uri, conflicts);
      if (success) {
        if (vscode.workspace.getConfiguration('suture').get<boolean>('showNotifications') !== false) {
          vscode.window.showInformationMessage(`Auto-merged ${conflicts.length} conflict(s) in ${path.basename(document.fileName)}.`);
        }
        updateEditorConflicts(editor);
        updateMergeStatus();
      } else {
        vscode.window.showErrorMessage('Auto-merge failed. Try a different resolution strategy or configure suture.apiUrl.');
      }
      return;
    }

    const resolved = resolveConflicts(text, conflicts, action as 'ours' | 'theirs' | 'both');
    const edit = new vscode.WorkspaceEdit();
    const fullRange = new vscode.Range(
      document.positionAt(0),
      document.positionAt(text.length)
    );
    edit.replace(document.uri, fullRange, resolved);
    await vscode.workspace.applyEdit(edit);

    if (vscode.workspace.getConfiguration('suture').get<boolean>('showNotifications') !== false) {
      vscode.window.showInformationMessage(`Resolved ${conflicts.length} conflict(s) using "${action}".`);
    }
    updateEditorConflicts(editor);
    updateMergeStatus();
  });

  registerCommand('suture.resolveAll', async () => {
    const root = suture.getWorkspaceRoot();
    if (!root) {
      vscode.window.showErrorMessage('No workspace folder open.');
      return;
    }

    const conflictUris: vscode.Uri[] = [];
    const conflictMarkerPattern = /^<{7}\s+/m;
    const supportedExts = Object.keys(DRIVER_MAP);

    for (const folder of vscode.workspace.workspaceFolders || []) {
      const files = await vscode.workspace.findFiles('**/*', '**/node_modules/**');
      for (const fileUri of files) {
        const ext = path.extname(fileUri.fsPath).toLowerCase();
        if (!supportedExts.includes(ext)) continue;

        try {
          const content = fs.readFileSync(fileUri.fsPath, 'utf-8');
          if (conflictMarkerPattern.test(content)) {
            conflictUris.push(fileUri);
          }
        } catch {
          // skip unreadable files
        }
      }
    }

    if (conflictUris.length === 0) {
      vscode.window.showInformationMessage('No files with merge conflicts found in workspace.');
      return;
    }

    const action = await showConflictResolutionPicker();
    if (!action) return;

    if (action === 'demo') {
      vscode.env.openExternal(vscode.Uri.parse('https://suture.dev/#/merge'));
      return;
    }

    let resolved = 0;
    let failed = 0;

    for (const uri of conflictUris) {
      const uriStr = uri.toString();
      try {
        let text: string;
        const doc = vscode.workspace.textDocuments.find(d => d.uri.toString() === uriStr);
        if (doc && !doc.isClosed) {
          text = doc.getText();
        } else {
          text = fs.readFileSync(uri.fsPath, 'utf-8');
        }

        const fileConflicts = findConflicts(text);
        if (fileConflicts.length === 0) continue;

        if (action === 'auto') {
          const success = await resolveWithSutureAuto(uri, fileConflicts);
          if (success) {
            resolved++;
          } else {
            failed++;
          }
          continue;
        }

        const merged = resolveConflicts(text, fileConflicts, action as 'ours' | 'theirs' | 'both');

        if (doc && !doc.isClosed) {
          const edit = new vscode.WorkspaceEdit();
          edit.replace(uri, new vscode.Range(doc.positionAt(0), doc.positionAt(text.length)), merged);
          await vscode.workspace.applyEdit(edit);
          await doc.save();
        } else {
          fs.writeFileSync(uri.fsPath, merged, 'utf-8');
        }

        resolved++;
      } catch {
        failed++;
      }
    }

    if (vscode.workspace.getConfiguration('suture').get<boolean>('showNotifications') !== false) {
      vscode.window.showInformationMessage(`Resolved ${resolved} file(s), ${failed} failed.`);
    }
    if (vscode.window.activeTextEditor) {
      updateEditorConflicts(vscode.window.activeTextEditor);
    }
    updateMergeStatus();
  });

  registerCommand('suture.openDemo', async () => {
    vscode.env.openExternal(vscode.Uri.parse('https://suture.dev/#/merge'));
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
