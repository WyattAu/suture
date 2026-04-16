import * as vscode from 'vscode';

function getSuturePath(): string {
    const config = vscode.workspace.getConfiguration('suture');
    return config.get<string>('path', 'suture');
}

function runSuture(args: string[], terminal?: vscode.Terminal): void {
    const suturePath = getSuturePath();
    if (!terminal) {
        terminal = vscode.window.createTerminal('Suture');
    }
    terminal.show();
    terminal.sendText(`${suturePath} ${args.join(' ')}`);
}

export function activate(context: vscode.ExtensionContext) {
    const disposable = vscode.commands.registerCommand('suture.semanticMerge', () => {
        const activeEditor = vscode.window.activeTextEditor;
        if (!activeEditor) {
            vscode.window.showErrorMessage('No active editor');
            return;
        }
        const filePath = activeEditor.document.uri.fsPath;
        runSuture(['merge', filePath]);
    });

    const initDisposable = vscode.commands.registerCommand('suture.init', () => {
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders || workspaceFolders.length === 0) {
            vscode.window.showErrorMessage('No workspace folder open');
            return;
        }
        runSuture(['init']);
    });

    const statusDisposable = vscode.commands.registerCommand('suture.status', () => {
        runSuture(['status']);
    });

    context.subscriptions.push(disposable, initDisposable, statusDisposable);
}

export function deactivate() {}
