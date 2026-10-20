-- Copy into the script directory next to the Desktop executable, then restart.
-- Stable semantic IDs: no translated labels, phone numbers or list positions.
local hidden = {
    ["dialogs.confirm_phone"] = true,
    ["settings.premium"] = true,
    ["settings.my_stars"] = true,
    ["main_menu.premium"] = true,
    ["main_menu.my_stars"] = true,
}

function filter_ui(item)
    if hidden[item.id] then
        return { action = "hide", reason = "Hidden by ui_filter.lua" }
    end
    -- nil leaves this item unchanged and lets other scripts evaluate it.
end
