# suture.nvim

![Version](https://img.shields.io/badge/version-5.3.1-blue)

Suture VCS integration for Neovim.

## Features

- Status signs in the gutter (added, modified, deleted files)
- Commit staging via visual selection
- Branch management commands
- Semantic merge conflict resolution

## Installation

### lazy.nvim
```lua
{
  "WyattAu/suture",
  dir = "editors/neovim/suture.nvim",
  dependencies = { "nvim-lua/plenary.nvim" },
  config = function()
    require("suture").setup()
  end,
}
```

### vim-plug
```vim
Plug 'WyattAu/suture', { 'rtp': 'editors/neovim/suture.nvim' }
```

## Commands

| Command | Description |
|---------|-------------|
| `:SutureStatus` | Show repo status in a float window |
| `:SutureStage` | Stage current file |
| `:SutureUnstage` | Unstage current file |
| `:SutureCommit` | Open commit message buffer |
| `:SutureBranch` | Show branch list |
| `:SutureCheckout <branch>` | Switch branches |
| `:SutureDiff` | Show diff for current file |
| `:SutureLog` | Show commit log |
| `:SuturePush` | Push to remote |
| `:SuturePull` | Pull from remote |

## Setup

```lua
require("suture").setup({
  -- Path to suture binary (default: "suture")
  suture_bin = "suture",
  -- Enable signs in gutter (default: true)
  signs = true,
  -- Sign icons (default: see below)
  signs_opts = {
    added = "+",
    modified = "~",
    deleted = "_",
  },
  -- Auto-refresh on BufWritePost (default: true)
  auto_refresh = true,
})
```

## License

Apache-2.0
