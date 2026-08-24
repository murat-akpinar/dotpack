-- This file needs to have same structure as nvconfig.lua 
-- https://github.com/NvChad/ui/blob/v3.0/lua/nvconfig.lua
-- Please read that file to know all available options :( 

---@type ChadrcConfig
local M = {}

M.base46 = {
	theme = "catppuccin",

	hl_add = {},
}

-- indent cubuklari: IblRainbowN soluk, IblScopeN canli (imlecin oldugu blok)
-- hl_add "defaults" cache'ine girer, tema degisiminde otomatik yeniden uygulanir
local FADE = 45 -- 0 = tam renk, 100 = gorunmez
for i, color in ipairs { "red", "orange", "yellow", "green", "cyan", "blue", "purple" } do
	M.base46.hl_add["IblRainbow" .. i] = { fg = { color, "black", FADE } }
	M.base46.hl_add["IblScope" .. i] = { fg = color }
end

-- M.nvdash = { load_on_startup = true }
-- M.ui = {
--       tabufline = {
--          lazyload = false
--      }
-- }

return M
