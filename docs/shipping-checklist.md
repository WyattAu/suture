# Suture v2.9.0 Shipping Checklist

## Prerequisites
- [ ] Rust 1.85+ installed
- [ ] GitHub CLI (`gh`) authenticated
- [ ] crates.io token (`cargo login`)
- [ ] Docker (for MinIO integration testing, optional)

## 1. Tag and Release
- [ ] Update VERSION.md with final test count and date
- [ ] `git tag v2.9.0`
- [ ] `git push origin main --tags`
- [ ] Verify GitHub Actions CI passes on tag
- [ ] Verify GitHub Actions Release builds all 5 binaries
- [ ] Download and test Linux x86_64 binary
- [ ] Download and test macOS binary (if available)
- [ ] Verify release notes on GitHub

## 2. crates.io Publishing
- [ ] `cargo login` with crates.io token
- [ ] Follow `packaging/PUBLISH.md` dependency order
- [ ] `cargo publish -p suture-common` (first)
- [ ] `cargo publish -p suture-core`
- [ ] Continue through dependency chain...
- [ ] `cargo publish -p suture-cli` (last)
- [ ] Verify `cargo install suture-cli` works

## 3. Package Managers
- [ ] Test Homebrew: `brew install --build-from-source packaging/homebrew/suture.rb`
- [ ] Test AUR: `makepkg -si` in `packaging/aur/`
- [ ] Create AUR repository and push PKGBUILD

## 4. GitHub Pages
- [ ] Go to repo Settings → Pages
- [ ] Source: Deploy from a branch → `docs` folder
- [ ] Save and verify https://wyattau.github.io/suture/ loads

## 5. Editor Plugins
- [ ] VS Code: `cd vscode-extension && npm install && npm run compile`
- [ ] Package: `vsce package`
- [ ] JetBrains: Open in IntelliJ IDEA, verify plugin compiles
- [ ] Neovim: `:packadd path/to/neovim-plugin/`

## 6. Announcements
- [ ] Share release notes on social media / communities
- [ ] Post to r/rust, r/git, Hacker News
- [ ] Update project README badges

## Post-Release
- [ ] Monitor for issues and bug reports
- [ ] Respond to community feedback
- [ ] Plan v2.10.0 based on feedback
