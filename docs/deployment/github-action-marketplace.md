# GitHub Action Marketplace Submission

## Prerequisites
- VS Code Marketplace publisher account (same as GitHub account)
- Action tested and published in repo

## Steps
1. Go to https://github.com/marketplace/actions/new
2. Fill in:
   - Name: Suture Merge Driver
   - Description: Semantic merge for structured files — JSON, YAML, TOML, CSV, XML, and 12 more formats. Resolves merge conflicts automatically.
   - Category: Utilities
   - Owner: WyattAu
   - Repository: WyattAu/suture
   - File path: .github/actions/merge/action.yml (relative to repo root)
3. Read and accept the terms
4. Submit

## Action.yml
The action.yml should reference:
- Uses `dist/index.js` (already built)
- Inputs: base-ref, head-ref, formats (comma-separated), api-url
- Outputs: merge-status, conflicts-resolved, conflicts-remaining
