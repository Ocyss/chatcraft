-- Chatcraft 示例过滤脚本。
-- 定义全局函数 filter(msg)，返回 "allow" | "hide" | "collapse" | "highlight"
-- 或 { action = "hide", reason = "..." }。
--
-- msg 字段：id, chat_id, chat_type ("private"|"group"|"supergroup"|"channel"),
--           chat_title, sender_id, sender_name, text, is_sponsored, is_forwarded

function filter(msg)
    -- 频道消息包含推广关键词则隐藏
    if msg.chat_type == "channel" then
        local spam_words = { "推广", "代开", "空投" }
        for _, w in ipairs(spam_words) do
            if msg.text:find(w, 1, true) then
                return { action = "hide", reason = "channel spam: " .. w }
            end
        end
    end

    -- 群组里的转发消息折叠显示
    if msg.chat_type == "group" and msg.is_forwarded then
        return "collapse"
    end

    return "allow"
end
