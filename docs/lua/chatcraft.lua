---@meta chatcraft

-- LuaLS 类型文档唯一来源；仅供编辑器索引，不执行、不 require，也不要作为运行脚本安装。
-- 宿主提供 chatcraft 及 utils，脚本只添加回调，不得替换整张表。
-- 各脚本状态独立；显式脚本先加载，再按路径排序加载目录直接子文件 .lua。
-- 缺失回调、nil、无效结果或回调错误均不决策；不是撤销其他有效规则。
-- 消息顺序为广告、配置规则、首个有效 Lua 结果；allow 也会终止消息脚本链。
-- 初始化、过滤及异步确认回调有一百万条指令限制，不是墙钟或内存限制。

---@alias ChatcraftChatType 'private'|'group'|'supergroup'|'channel'

-- collapse、highlight 是协议意图，当前桌面尚无对应显示效果。
---@alias ChatcraftAction 'allow'|'hide'|'collapse'|'highlight'

---@class ChatcraftMessage
---@field id integer 有符号 64 位消息 ID；不要假设与 Bot API 编码相同。
---@field chat_id integer 有符号 64 位会话 ID。
---@field chat_type ChatcraftChatType 普通群和超级群分别为 group 和 supergroup。
---@field chat_title? string 会话标题；可空字段按 nil 处理。
---@field sender_id? integer 可空有符号 64 位发送者 ID。
---@field sender_name? string 发送者名称。
---@field text string 消息文本。
---@field is_sponsored boolean 是否为官方赞助消息，输入缺失时默认 false。
---@field is_forwarded boolean 是否转发，输入缺失时默认 false。

---@class ChatcraftUIItem
---@field id string 宿主已接入的稳定语义 ID，不是翻译文案、电话号码或列表位置。
---@field kind string 宿主提供的控件类别。

---@class ChatcraftActionResult
---@field action ChatcraftAction 有效动作；未知字符串不决策。
---@field reason? string 可选原因。

---@alias ChatcraftFilterResult ChatcraftAction|ChatcraftActionResult|nil

---@class ChatcraftChatListItem
---@field chat_id integer 有符号 64 位会话 ID。
---@field chat_type ChatcraftChatType 会话类型。
---@field title string 会话标题。
---@field folder_ids integer[] 有符号 64 位文件夹 ID 数组，输入缺失默认空；桌面仅列出匹配的非零自定义文件夹，不补主列表或归档 ID。
---@field folder_count integer 非负整数，等于 folder_ids 长度，不是全部文件夹总数。
---@field folder_id integer 当前视图的有符号 64 位 ID；桌面 0 为主列表，-1 为归档，正数为自定义文件夹。
---@field unread boolean 默认 false；桌面包括未读消息或手动未读标记。
---@field unread_count integer 无符号 32 位整数，范围 0..4294967295，默认 0。
---@field muted boolean 是否静音，默认 false。
---@field pinned boolean 当前筛选视图下是否置顶，默认 false。

-- 隐藏取所有脚本的逻辑 OR，后续不能撤销；第一个有效表确定优先级。
-- 即使只返回 hide 也会确定 priority 为 0；字段类型错误使整个结果无效。
-- 分数越大越靠前，同分保留原生顺序；置顶优先需脚本自行赋分。
---@class ChatcraftChatListResult
---@field hide? boolean 默认 false，只改变本地显示。
---@field priority? integer 有符号 32 位整数，范围 -2147483648..2147483647，默认 0。

---@class ChatcraftStartupContext
---@field event 'startup' 宿主请求时每个引擎仅触发一次；重建引擎会重置，不是每次安装一次。

---@class ChatcraftManagerOpenContext
---@field event 'manager_open' 每次宿主请求打开管理器时触发。

---@class ChatcraftPopup
---@field title string 兼容的生命周期返回通知标题。
---@field text string 通知纯文本；每脚本每事件最多返回一条，宿主可汇总展示。

-- 五个字段均可省略；非字符串被忽略。加载时快照，运行中修改不会刷新管理器。
---@class ChatcraftScriptMetadata
---@field name? string 展示名；状态接口顶层 name 仍为文件名。
---@field version? string 脚本版本。
---@field description? string 脚本说明。
---@field author? string 作者。
---@field update_url? string 仅展示，不联网检查、下载、更新或安装。

-- 两个函数均为点调用，严格要求参数数量和类型，异步入队且无返回值。
-- 不要在高频过滤回调中弹窗；宿主处理队列后才显示，不可同步等待用户。
-- 队列中待处理 alert 与未答复 confirm 合计上限 128，超限抛错。
---@class ChatcraftUtils
---@field alert fun(title: string, text: string) 异步纯文本提示，无返回值。
---@field confirm fun(title: string, text: string, callback: fun(confirmed: boolean)) 异步确认，无返回值；宿主答复后回调一次，取消为 false，回调不应返回结果或反复创建确认框。

-- 新回调优先于对应 on_*，再兼容全局 filter/filter_ui；已选回调即使
-- 返回 nil 或报错也不会回退到同脚本的旧回调。没有全局 chat 或生命周期别名。
---@class ChatcraftAPI
---@field utils ChatcraftUtils 仅宿主弹窗 API；不提供 util 或过滤辅助函数。
---@field filter_message? fun(msg: ChatcraftMessage): ChatcraftFilterResult 首个有效结果胜出；不匹配应返回 nil。
---@field filter_ui? fun(item: ChatcraftUIItem): ChatcraftFilterResult 仅 hide 生效，第一个 hide 胜出；allow 不阻止其他脚本隐藏。
---@field filter_chat_list? fun(chat: ChatcraftChatListItem): ChatcraftChatListResult|nil 隐藏与排序仅作用于宿主接入的会话列表投影。
---@field on_startup? fun(ctx: ChatcraftStartupContext): ChatcraftPopup|nil 可用 utils 异步提示，返回 nil 避免额外兼容通知。
---@field on_manager_open? fun(ctx: ChatcraftManagerOpenContext): ChatcraftPopup|nil 可用 utils 异步提示，不应重复注册或无限弹窗。
---@field on_message? fun(msg: ChatcraftMessage): ChatcraftFilterResult 已弃用（deprecated），请使用 filter_message。
---@field on_ui? fun(item: ChatcraftUIItem): ChatcraftFilterResult 已弃用（deprecated），请使用 filter_ui。
---@field on_chat? fun(chat: ChatcraftChatListItem): ChatcraftChatListResult|nil 已弃用（deprecated），请使用 filter_chat_list。

---@type ChatcraftAPI
chatcraft = chatcraft

---@type ChatcraftScriptMetadata|nil
script = script

---@deprecated
---@type fun(msg: ChatcraftMessage): ChatcraftFilterResult
filter = filter

---@deprecated
---@type fun(item: ChatcraftUIItem): ChatcraftFilterResult
filter_ui = filter_ui
