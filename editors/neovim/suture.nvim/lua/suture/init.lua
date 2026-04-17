---@class suture.Config
---@field suture_bin string Path to suture binary
---@field signs boolean Enable gutter signs
---@field signs_opts table Sign icons
---@field auto_refresh boolean Auto-refresh on save

---@type suture.Config
local default_config = {
  suture_bin = "suture",
  signs = true,
  signs_opts = {
    added = "+",
    modified = "~",
    deleted = "_",
  },
  auto_refresh = true,
}

---@type suture.Config
local config = {}

local M = {}

--- Setup the plugin with user configuration
---@param opts? suture.Config
function M.setup(opts)
  config = vim.tbl_deep_extend("force", default_config, opts or {})

  -- Register user commands
  M._register_commands()

  -- Set up autocommands
  if config.auto_refresh then
    M._register_autocmds()
  end

  -- Set up signs
  if config.signs then
    M._setup_signs()
  end
end

--- Run a suture command and return stdout
---@param args string[] Command arguments
---@param cwd? string Working directory
---@return string[] stdout_lines
---@return integer exit_code
function M.run(args, cwd)
  local cmd = { config.suture_bin }
  vim.list_extend(cmd, args)
  local output = vim.system(cmd, { cwd = cwd, text = true }):wait()
  local lines = {}
  if output.stdout and output.stdout ~= "" then
    lines = vim.split(output.stdout, "\n")
  end
  return lines, output.code or 1
end

--- Check if current directory is a suture repo
---@return boolean
function M.is_repo()
  local lines, code = M.run({ "status" })
  return code == 0
end

--- Get repo status
---@return table[] List of {path, status} entries
function M.get_status()
  local lines, code = M.run({ "status", "--porcelain" })
  if code ~= 0 then
    return {}
  end
  local result = {}
  for _, line in ipairs(lines) do
    if line ~= "" then
      local status = line:sub(1, 2)
      local path = line:sub(4)
      table.insert(result, { status = status, path = path })
    end
  end
  return result
end

function M._register_commands()
  vim.api.nvim_create_user_command("SutureStatus", function()
    M.show_status()
  end, { desc = "Show Suture repo status" })

  vim.api.nvim_create_user_command("SutureStage", function()
    M.stage_file()
  end, { desc = "Stage current file" })

  vim.api.nvim_create_user_command("SutureUnstage", function()
    M.unstage_file()
  end, { desc = "Unstage current file" })

  vim.api.nvim_create_user_command("SutureCommit", function()
    M.open_commit()
  end, { desc = "Open commit message buffer" })

  vim.api.nvim_create_user_command("SutureBranch", function()
    M.show_branches()
  end, { desc = "Show branches" })

  vim.api.nvim_create_user_command("SutureCheckout", function(opts)
    M.checkout(opts.args)
  end, { nargs = 1, desc = "Switch branches" })

  vim.api.nvim_create_user_command("SutureDiff", function()
    M.show_diff()
  end, { desc = "Show diff for current file" })

  vim.api.nvim_create_user_command("SutureLog", function()
    M.show_log()
  end, { desc = "Show commit log" })

  vim.api.nvim_create_user_command("SuturePush", function()
    M.push()
  end, { desc = "Push to remote" })

  vim.api.nvim_create_user_command("SuturePull", function()
    M.pull()
  end, { desc = "Pull from remote" })
end

function M._register_autocmds()
  vim.api.nvim_create_autocmd("BufWritePost", {
    callback = function()
      if M.is_repo() then
        M._refresh_signs()
      end
    end,
    desc = "Refresh suture signs after save",
  })
end

function M._setup_signs()
  local ns = vim.api.nvim_create_namespace("suture")
  vim.fn.sign_define("SutureAdded", { text = config.signs_opts.added, texthl = "SutureSignAdd" })
  vim.fn.sign_define("SutureModified", { text = config.signs_opts.modified, texthl = "SutureSignChange" })
  vim.fn.sign_define("SutureDeleted", { text = config.signs_opts.deleted, texthl = "SutureSignDelete" })

  -- Define highlight groups
  vim.api.nvim_set_hl(0, "SutureSignAdd", { link = "GitSignsAdd", default = true })
  vim.api.nvim_set_hl(0, "SutureSignChange", { link = "GitSignsChange", default = true })
  vim.api.nvim_set_hl(0, "SutureSignDelete", { link = "GitSignsDelete", default = true })
end

function M._refresh_signs()
  if not config.signs then
    return
  end
  -- Clear existing signs
  vim.fn.sign_unplace("suture")
  local bufnr = vim.api.nvim_get_current_buf()
  local filepath = vim.api.nvim_buf_get_name(bufnr)
  if filepath == "" then
    return
  end

  local status = M.get_status()
  for _, entry in ipairs(status) do
    local abs_path = vim.fn.fnamemodify(entry.path, ":p")
    local buf_abs = vim.fn.fnamemodify(filepath, ":p")
    if abs_path == buf_abs then
      local s = entry.status
      if s:match("A") or s:match("?") then
        vim.fn.sign_place(0, "suture", "SutureAdded", bufnr)
      elseif s:match("M") then
        vim.fn.sign_place(0, "suture", "SutureModified", bufnr)
      elseif s:match("D") then
        vim.fn.sign_place(0, "suture", "SutureDeleted", bufnr)
      end
    end
  end
end

--- Show status in a float window
function M.show_status()
  local lines, code = M.run({ "status" })
  if code ~= 0 then
    vim.notify("Not a Suture repository", vim.log.levels.WARN)
    return
  end
  M._float_window("Suture Status", lines)
end

--- Stage current file
function M.stage_file()
  local filepath = vim.api.nvim_buf_get_name(0)
  if filepath == "" then
    return
  end
  M.run({ "add", filepath })
  M._refresh_signs()
  vim.notify("Staged: " .. filepath)
end

--- Unstage current file
function M.unstage_file()
  local filepath = vim.api.nvim_buf_get_name(0)
  if filepath == "" then
    return
  end
  M.run({ "restore", "--staged", filepath })
  M._refresh_signs()
  vim.notify("Unstaged: " .. filepath)
end

--- Open a commit message buffer
function M.open_commit()
  local buf = vim.api.nvim_create_buf(true, false)
  vim.api.nvim_buf_set_name(buf, "SUTURE_COMMIT_MSG")
  vim.api.nvim_set_option_value("buftype", "nofile", { buf = buf })
  vim.api.nvim_set_option_value("filetype", "suturecommit", { buf = buf })
  vim.cmd("split | buffer " .. buf)

  -- Set up commit keybinding
  vim.api.nvim_buf_set_keymap(buf, "n", "<CR>", "", {
    noremap = true,
    callback = function()
      local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
      local msg = table.concat(lines, "\n")
      if msg:match("^%s*$") then
        vim.notify("Empty commit message", vim.log.levels.WARN)
        return
      end
      vim.cmd("close")
      M.run({ "commit", "-m", msg })
      M._refresh_signs()
      vim.notify("Committed successfully")
    end,
  })
end

--- Show branches in a float window
function M.show_branches()
  local lines, code = M.run({ "branch" })
  if code ~= 0 then
    vim.notify("Failed to list branches", vim.log.levels.ERROR)
    return
  end
  M._float_window("Suture Branches", lines)
end

--- Checkout a branch
---@param branch string Branch name
function M.checkout(branch)
  if branch == "" then
    vim.notify("Branch name required", vim.log.levels.WARN)
    return
  end
  local _, code = M.run({ "checkout", branch })
  if code == 0 then
    vim.notify("Switched to: " .. branch)
    M._refresh_signs()
  else
    vim.notify("Failed to checkout: " .. branch, vim.log.levels.ERROR)
  end
end

--- Show diff for current file
function M.show_diff()
  local filepath = vim.api.nvim_buf_get_name(0)
  if filepath == "" then
    return
  end
  local lines, _ = M.run({ "diff", "--", filepath })
  M._float_window("Diff: " .. filepath, lines)
end

--- Show commit log
function M.show_log()
  local lines, code = M.run({ "log", "--oneline", "-20" })
  if code == 0 then
    M._float_window("Suture Log", lines)
  end
end

--- Push to remote
function M.push()
  local lines, code = M.run({ "push" })
  if code == 0 then
    vim.notify("Push successful")
  else
    M._float_window("Push Output", lines)
  end
end

--- Pull from remote
function M.pull()
  local lines, code = M.run({ "pull" })
  if code == 0 then
    vim.notify("Pull successful")
    M._refresh_signs()
  else
    M._float_window("Pull Output", lines)
  end
end

--- Create a floating window with content
---@param title string Window title
---@param content string[] Lines of content
function M._float_window(title, content)
  local width = math.min(80, vim.o.columns - 4)
  local height = math.min(#content + 2, vim.o.lines - 4)
  local col = math.floor((vim.o.columns - width) / 2)
  local row = math.floor((vim.o.lines - height) / 2)

  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, content)
  vim.api.nvim_set_option_value("bufhidden", "wipe", { buf = buf })
  vim.api.nvim_set_option_value("modifiable", false, { buf = buf })

  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = width,
    height = height,
    col = col,
    row = row,
    style = "minimal",
    border = "rounded",
    title = " " .. title .. " ",
  })

  vim.api.nvim_set_option_value("winhl", "NormalFloat:SutureFloat", { win = win })
  vim.api.nvim_set_hl(0, "SutureFloat", { link = "NormalFloat", default = true })
end

return M
