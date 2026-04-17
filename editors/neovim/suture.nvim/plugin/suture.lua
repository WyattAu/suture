-- Auto-load suture.nvim when entering a Suture repository
vim.api.nvim_create_autocmd("VimEnter", {
  callback = function()
    local status = vim.system({ "suture", "status" }, { text = true }):wait()
    if status.code == 0 then
      -- Inside a suture repo — the plugin was likely loaded via lazy.nvim
      -- with opts = function() ... end, so setup is already called.
      -- If not, attempt to load it:
      local ok, _ = pcall(require, "suture")
      if ok then
        require("suture")._refresh_signs()
      end
    end
  end,
  once = true,
})
