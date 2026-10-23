-- 非默认示例：复制到实际脚本目录，完全退出（包括托盘）并重启后生效。
---@type ChatcraftScriptMetadata
script = {
    name = "会话列表隐藏与已读优先",
    version = "1.0",
    description = "主列表隐藏属于多个自定义文件夹的会话，其余已读优先",
    author = "Chatcraft",
}

-- 已读为 1，未读为 0；辅助逻辑由脚本自行实现。
---@param chat ChatcraftChatListItem
---@return integer
local function read_priority(chat)
    return not chat.unread and chat.unread_count <= 0 and 1 or 0
end

---@param chat ChatcraftChatListItem
---@return ChatcraftChatListResult
function chatcraft.filter_chat_list(chat)
    return {
        -- 0 为主列表，-1 为归档，正数为自定义文件夹；不改变服务器归属。
        hide = chat.folder_id == 0 and chat.folder_count > 1,
        priority = read_priority(chat),
    }
end
