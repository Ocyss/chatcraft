@echo off
rem 将 patches\desktop 下的补丁应用到 clients\desktop 子模块。
rem 用法: scripts\apply-patches.bat
cd /d "%~dp0\.."
for %%p in (patches\desktop\*.patch) do (
    echo ==^> apply %%p
    git -C clients\desktop apply --check "..\..\%%p" || exit /b 1
    git -C clients\desktop apply "..\..\%%p" || exit /b 1
)
echo All patches applied.
