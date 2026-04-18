import * as vscode from 'vscode';
import { execFile } from 'child_process';
import { promisify } from 'util';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    TransportKind,
} from 'vscode-languageclient/node';

const execFileAsync = promisify(execFile);

function escapeHtml(str: string): string {
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}

function getSuturePath(): string {
    return vscode.workspace.getConfiguration('suture').get<string>('path') || 'suture';
}

function getLspEnabled(): boolean {
    return vscode.workspace.getConfiguration('suture').get<boolean>('lsp.enabled') ?? true;
}

function getLspPath(): string {
    return vscode.workspace.getConfiguration('suture').get<string>('lsp.path') || 'suture-lsp';
}

async function runSuture(args: string[], cwd?: string): Promise<string> {
    const suturePath = getSuturePath();
    const result = await execFileAsync(suturePath, args, {
        cwd: cwd || vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        maxBuffer: 10 * 1024 * 1024,
    });
    return result.stdout;
}

let lspClient: LanguageClient | undefined;

function startLspClient(context: vscode.ExtensionContext): void {
    if (!getLspEnabled()) {
        return;
    }

    const lspPath = getLspPath();
    const serverOptions: ServerOptions = {
        run: { command: lspPath, transport: TransportKind.stdio },
        debug: { command: lspPath, transport: TransportKind.stdio },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file' },
        ],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/.suture/**'),
        },
    };

    lspClient = new LanguageClient(
        'suture-lsp',
        'Suture Language Server',
        serverOptions,
        clientOptions,
    );

    lspClient.start().then(() => {
        vscode.window.setStatusBarMessage('Suture LSP started', 3000);
    }, (err) => {
        vscode.window.showWarningMessage(`Suture LSP failed to start: ${err}`);
    });

    context.subscriptions.push(
        new vscode.Disposable(() => {
            if (lspClient) {
                lspClient.stop();
                lspClient = undefined;
            }
        }),
    );
}

export function activate(context: vscode.ExtensionContext) {
    const blameDisposable = vscode.commands.registerCommand('suture.blame', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showWarningMessage('No active editor');
            return;
        }

        const relativePath = vscode.workspace.asRelativePath(editor.document.uri);
        try {
            const output = await runSuture(['blame', relativePath]);
            const doc = await vscode.workspace.openTextDocument({
                content: output,
                language: 'plaintext',
            });
            await vscode.window.showTextDocument(doc, vscode.ViewColumn.Beside);
        } catch (err: any) {
            vscode.window.showErrorMessage(`Suture blame failed: ${err.message}`);
        }
    });

    const logDisposable = vscode.commands.registerCommand('suture.log', async () => {
        try {
            const output = await runSuture(['log', '--oneline']);
            const panel = vscode.window.createWebviewPanel(
                'sutureLog',
                'Suture Log',
                vscode.ViewColumn.One,
                {}
            );
            const html = `<!DOCTYPE html>
<html><head><style>
body { font-family: monospace; padding: 10px; background: var(--vscode-editor-background); color: var(--vscode-editor-foreground); }
.commit { padding: 4px 0; border-bottom: 1px solid var(--vscode-panel-border); }
.hash { color: var(--vscode-terminal-ansiYellow); cursor: pointer; }
.msg { color: var(--vscode-editor-foreground); }
</style></head><body>
${output.split('\n').map(line => {
    const match = line.match(/^(\w+)\s+(.*)/);
    if (match) {
        return `<div class="commit"><span class="hash">${escapeHtml(match[1])}</span> <span class="msg">${escapeHtml(match[2])}</span></div>`;
    }
    return `<div class="commit">${escapeHtml(line)}</div>`;
}).join('\n')}
</body></html>`;
            panel.webview.html = html;
        } catch (err: any) {
            vscode.window.showErrorMessage(`Suture log failed: ${err.message}`);
        }
    });

    const initDisposable = vscode.commands.registerCommand('suture.init', async () => {
        const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        if (!workspaceRoot) {
            vscode.window.showErrorMessage('No workspace folder open');
            return;
        }
        try {
            await runSuture(['init', workspaceRoot]);
            vscode.window.showInformationMessage('Suture repository initialized');
        } catch (err: any) {
            vscode.window.showErrorMessage(`Suture init failed: ${err.message}`);
        }
    });

    const statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBarItem.command = 'suture.log';
    context.subscriptions.push(statusBarItem);

    async function updateBranch() {
        try {
            const output = await runSuture(['status']);
            for (const line of output.split('\n')) {
                const m = line.match(/^On branch (.+)/);
                if (m) {
                    statusBarItem.text = `$(git-branch) ${m[1]}`;
                    statusBarItem.tooltip = 'Suture: Click to view log';
                    statusBarItem.show();
                    return;
                }
            }
            statusBarItem.hide();
        } catch {
            statusBarItem.hide();
        }
    }

    updateBranch();
    vscode.workspace.onDidSaveTextDocument(() => updateBranch());

    context.subscriptions.push(blameDisposable, logDisposable, initDisposable);

    startLspClient(context);
}

export async function deactivate(): Promise<void> {
    if (lspClient) {
        await lspClient.stop();
    }
}
