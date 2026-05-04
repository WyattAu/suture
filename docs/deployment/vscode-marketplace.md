# VS Code Marketplace Submission

## Prerequisites
- Azure DevOps account (linked to GitHub)
- `@vscode/vsce` installed: `npm install -g @vscode/vsce`

## Steps
1. Create publisher: `npx @vscode/vsce create-publisher WyattAu`
2. Get Personal Access Token from https://dev.azure.com
3. Login: `vsce login WyattAu`
4. Package: `cd vscode-extension && vsce package`
5. Publish: `vsce publish`
