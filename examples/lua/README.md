# Lua 过滤示例

此目录存放客户端运行时过滤脚本，不参与客户端编译，也不会自动加载。
仓库的 `scripts/` 目录只存放开发和 CI 工具。

## 使用

完全退出 Telegram（包括托盘进程），然后在 PowerShell 中指定脚本的绝对路径：

```powershell
$env:CHATCRAFT_LUA_SCRIPT = "E:\personal\chatcraft\examples\lua\example.lua"
& "H:\Telegram-Windows-x64\Telegram.exe"
```

文件也可以放在其他位置；目前不会自动扫描 EXE 旁的目录。
脚本在首次消息过滤时加载，修改后需完全退出再启动，没有热重载。
过滤只控制显示，不删除服务端消息。

`example.lua` 展示 `hide` 和 `collapse` 返回值。当前桌面端仅实现 `hide`；
`collapse` 和 `highlight` 尚未实现显示效果。普通群为 `group`，超级群为
`supergroup`，规则需根据实际会话类型编写。
