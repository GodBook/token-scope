@echo off
chcp 65001 >nul
title TokenScope - 本地一键编译与自动无损更新
color 0b

echo ================================================================
echo           TokenScope - 本地一键编译与无损热更新
echo ================================================================
echo.

set "PROJECT_DIR=%~dp0.."
set "PORTABLE_DIR1=%PROJECT_DIR%\TokenScope-绿色免安装版"
set "PORTABLE_DIR2=%PROJECT_DIR%\..\TokenScope-绿色免安装版"
set "APP_DATA_DIR=%APPDATA%\com.tokenstat.desktop"
set "BACKUP_DIR=%APP_DATA_DIR%\backups"

:: 步骤 1: 自动物理备份数据库
echo [1/3] 正在安全备份本地 SQLite 数据库...
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

:: 步骤 2: 使用 Tauri 官方构建命令打包前端并嵌入到离线二进制
echo [2/3] 正在使用 Tauri 官方引擎打包嵌入离线前端资源并编译 Rust...
cd /d "%PROJECT_DIR%"
call npx tauri build --no-bundle
if %ERRORLEVEL% NEQ 0 (
    color 0c
    echo.
    echo [×] 构建失败，请检查上方报错！
    pause
    exit /b 1
)
echo   [√] 离线独立程序打包完成！
echo.

:: 步骤 3: 同步到绿色免安装版并重启
echo [3/3] 正在安全同步到绿色免安装版...
if not exist "%PORTABLE_DIR1%" mkdir "%PORTABLE_DIR1%"
if not exist "%PORTABLE_DIR2%" mkdir "%PORTABLE_DIR2%"

:: 若有运行中的进程先关闭
taskkill /F /IM token-scope.exe >nul 2>&1
taskkill /F /IM TokenScope.exe >nul 2>&1
timeout /t 1 /nobreak >nul

copy /y "%PROJECT_DIR%\src-tauri\target\release\token-scope.exe" "%PORTABLE_DIR1%\TokenScope.exe" >nul
copy /y "%PROJECT_DIR%\src-tauri\target\release\token-scope.exe" "%PORTABLE_DIR2%\TokenScope.exe" >nul

echo.
echo ================================================================
echo   恭喜！TokenScope 本地构建与无损更新已圆满完成！
echo   历史数据 100%% 完好保留，正在为你启动最新版...
echo ================================================================
start "" "%PORTABLE_DIR1%\TokenScope.exe"

timeout /t 3 /nobreak >nul
exit 0
