local M = {}

M.check = function()
  vim.health.start("suture report")

  local bin = vim.fn.exepath("suture")
  if bin ~= "" then
    vim.health.ok("suture binary found: " .. bin)
    local handle = io.popen("suture --version 2>/dev/null")
    if handle then
      local version = handle:read("*a"):match("%S+")
      handle:close()
      if version then
        vim.health.ok("suture version: " .. version)
      end
    end
  else
    vim.health.error("suture binary not found in PATH")
  end

  local ok, _ = pcall(require, "plenary")
  if ok then
    vim.health.ok("plenary.nvim installed")
  else
    vim.health.warn("plenary.nvim not found (required for some features)")
  end
end

return M
