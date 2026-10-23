-- 非默认示例：复制到实际脚本目录，完全退出（包括托盘）并重启后生效。
---@type ChatcraftScriptMetadata
script = {
    name = "界面精简",
    version = "1.0",
    description = "隐藏宿主已接入的手机号确认、会员与星星入口",
    author = "Chatcraft",
}

---@type table<string, boolean>
local hidden = {
    ["dialogs.confirm_phone"] = true,
    ["settings.premium"] = true,
    ["settings.my_stars"] = true,
    ["main_menu.premium"] = true,
    ["main_menu.my_stars"] = true,
}

---@param item ChatcraftUIItem
---@return ChatcraftFilterResult
function chatcraft.filter_ui(item)
    if hidden[item.id] then
        return { action = "hide", reason = "界面精简示例" }
    end
    return nil
end
