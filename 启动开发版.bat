@echo off
chcp 65001 >nul
setlocal
cd /d "%~dp0"

where.exe node >nul 2>&1
if errorlevel 1 (
    echo [错误] 找不到 Node.js。请先安装 Node.js，并确保 node 命令已加入 PATH。
    goto :failed
)

where.exe pnpm.cmd >nul 2>&1
if errorlevel 1 (
    echo [错误] 找不到 pnpm。请先安装 pnpm，并确保 pnpm.cmd 已加入 PATH。
    goto :failed
)

if not exist "node_modules\.bin\tauri.cmd" (
    echo 未发现本地 Tauri CLI，正在按锁文件安装前端依赖...
    call pnpm.cmd install --frozen-lockfile
    if errorlevel 1 (
        echo [错误] 依赖安装失败，请检查上方 pnpm 输出后重试。
        goto :failed
    )
)

echo 正在启动 Tauri 开发版...
call pnpm.cmd tauri dev
set "APP_EXIT_CODE=%ERRORLEVEL%"
if not "%APP_EXIT_CODE%"=="0" (
    echo Tauri 开发版已退出，退出码：%APP_EXIT_CODE%。
)
pause
exit /b %APP_EXIT_CODE%

:failed
pause
exit /b 1
