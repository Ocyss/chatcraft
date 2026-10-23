-- 唯一默认启用的示例；不注册过滤回调，也不在启动时弹窗。
---@type ChatcraftScriptMetadata
script = {
    name = "Chatcraft 使用说明",
    version = "1.0",
    description = "打开管理器时演示异步确认与提示，不改变过滤结果",
    author = "Chatcraft",
}

---@param ctx ChatcraftStartupContext
---@return nil
function chatcraft.on_startup(ctx)
    return nil
end

-- 每个引擎只演示一次，避免反复打开管理器时重复提示。
local shown = false

---@param ctx ChatcraftManagerOpenContext
---@return nil
function chatcraft.on_manager_open(ctx)
    if shown then
        return nil
    end
    shown = true
    chatcraft.utils.confirm("Chatcraft", "是否查看脚本使用说明？", function(confirmed)
        if confirmed then
            chatcraft.utils.alert("Chatcraft Lua 脚本使用说明",
                "在管理器查看实际脚本目录、元数据与加载状态。\n"
                .. "仅安装可信脚本；修改后需完全退出（包括托盘）并重启。\n"
                .. "README.md 提供使用说明，types/chatcraft.lua 提供编辑器类型文档。\n"
                .. "ad_filter.lua、ui_filter.lua、chat_list.lua 为非默认示例，按需安装。\n"
                .. "更新地址仅供查看，不会自动下载或安装。")
        end
    end)
    return nil
end
