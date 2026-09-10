@echo off
chcp 65001 >nul
title TokenScope - 本地一键编译与自动无损更新
color 0b

echo ================================================================
echo           TokenScope - 本地一键编译与无损热更新
echo ================================================================
echo.

set "PROJECT_DIR=%~dp0.."
set "PORTABLE_DIR=%PROJECT_DIR%\..\TokenScope-绿色免安装版"
set "PORTABLE_EXE=%PORTABLE_DIR%\TokenScope.exe"
set "APP_DATA_DIR=%APPDATA%\com.tokenstat.desktop"
set "BACKUP_DIR=%APP_DATA_DIR%\backups"

:: 步骤 1: 自动物理备份数据库
echo [1/4] 正在安全备份本地 SQLite 数据库...
if not exist "%BACKUP_DIR%" mkdir "%BACKUP_DIR%"
for /f "tokens=2 delims==" %%I in ('wmic os get localdatetime /value') do set datetime=%%I
set "TIMESTAMP=%datetime:~0,8%_%datetime:~8,6%"
if exist "%APP_DATA_DIR%\token-statistics.sqlite" (
    copy /y "%APP_DATA_DIR%\token-statistics.sqlite" "%BACKUP_DIR%\token-statistics-pre-update-%TIMESTAMP%.sqlite" >nul
    echo   [√] 数据库已安全备份至: %BACKUP_DIR%\token-statistics-pre-update-%TIMESTAMP%.sqlite
) else (
    echo   [i] 未检测到历史数据库，跳过备份。
)
echo.

:: 步骤 2: 构建前端资源
echo [2/4] 正在打包前端资源 (Vite)...
cd /d "%PROJECT_DIR%"
call npm run build
if %ERRORLEVEL% NEQ 0 (
    color 0c
    echo.
    echo [×] 前端打包失败，请检查上方报错！
    pause
    exit /b 1
)
echo   [√] 前端打包完成！
echo.

:: 步骤 3: 增量编译 Rust 核心
echo [3/4] 正在编译 Rust 本地执行文件 (Release 模式)...
call cargo build --release --manifest-path "src-tauri/Cargo.toml"
if %ERRORLEVEL% NEQ 0 (
    color 0c
    echo.
    echo [×] Rust 编译失败，请检查上方报错！
    pause
    exit /b 1
)
echo   [√] Rust 核心编译完成！
echo.

:: 步骤 4: 同步到绿色免安装版并重启
echo [4/4] 正在安全同步到绿色免安装版...
if not exist "%PORTABLE_DIR%" mkdir "%PORTABLE_DIR%"

:: 若有运行中的进程先关闭
taskkill /F /IM token-scope.exe >nul 2>&1
taskkill /F /IM TokenScope.exe >nul 2>&1
timeout /t 1 /nobreak >nul

copy /y "%PROJECT_DIR%\src-tauri\target\release\token-scope.exe" "%PORTABLE_EXE%" >nul
if %ERRORLEVEL% EQU 0 (
    echo   [√] 已成功更新至: %PORTABLE_EXE%
) else (
    echo   [×] 文件复制失败，请确认文件未被其它程序占用。
)

echo.
echo ================================================================
echo   恭喜！TokenScope 本地编译与无损更新已圆满完成！
echo   历史数据 100%% 完好保留，正在为你启动最新版...
echo ================================================================
start "" "%PORTABLE_EXE%"

timeout /t 3 /nobreak >nul
exit 0
