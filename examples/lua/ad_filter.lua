---@type ChatcraftScriptMetadata
script = {
    name = "频道推广过滤",
    version = "1.0",
    description = "隐藏频道推广关键词，演示群转发消息的折叠意图",
    author = "Chatcraft",
}

---@param msg ChatcraftMessage
---@return ChatcraftFilterResult
function chatcraft.filter_message(msg)
    if msg.chat_type == "channel" then
        local spam_words = { "推广", "代开", "空投" }
        for _, word in ipairs(spam_words) do
            if msg.text:find(word, 1, true) then
                return { action = "hide", reason = "频道推广关键词：" .. word }
            end
        end
    end

    -- 保留折叠意图示例；当前桌面尚无 collapse 显示效果。
    if msg.chat_type == "group" and msg.is_forwarded then
        return "collapse"
    end

    -- 不截断后续脚本的消息决策。
    return nil
end
