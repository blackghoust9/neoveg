@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul 2>&1

:: ============================================================================
:: Neoveg — Lancador Rapido para Windows (win-x64)
:: Configura o PATH das bibliotecas FFI (.dll) e FFmpeg isolado, prepara o
:: diretorio de cache local (%LOCALAPPDATA%\Neoveg\) e inicia o Neoveg.exe.
:: ============================================================================

set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%..") do set "ROOT_DIR=%%~fI"

set "NEOVEG_CACHE_ROOT=%LOCALAPPDATA%\Neoveg"
set "NEOVEG_APP_CACHE=%NEOVEG_CACHE_ROOT%\AppCache"
set "NEOVEG_LOGS_DIR=%ROOT_DIR%\logs"

if not exist "%NEOVEG_APP_CACHE%" mkdir "%NEOVEG_APP_CACHE%" >nul 2>&1
if not exist "%NEOVEG_LOGS_DIR%" mkdir "%NEOVEG_LOGS_DIR%" >nul 2>&1

set "NATIVE_LIB_DIR=%ROOT_DIR%\runtimes\win-x64\native"
set "FFMPEG_BIN_DIR=%ROOT_DIR%\runtimes\win-x64\ffmpeg\bin"
set "PATH=%NATIVE_LIB_DIR%;%FFMPEG_BIN_DIR%;%ROOT_DIR%;%PATH%"
set "SEE_MASK_NOZONECHECKS=1"

if exist "%ROOT_DIR%\dist\Neoveg-Portable-win-x64\Neoveg.exe" (
    start "" "%ROOT_DIR%\dist\Neoveg-Portable-win-x64\Neoveg.exe" %*
    exit /b 0
)

if exist "%ROOT_DIR%\Neoveg.exe" (
    start "" "%ROOT_DIR%\Neoveg.exe" %*
    exit /b 0
)

if exist "%ROOT_DIR%\desktop-host\bin\Release\net8.0\win-x64\publish\Neoveg.exe" (
    start "" "%ROOT_DIR%\desktop-host\bin\Release\net8.0\win-x64\publish\Neoveg.exe" %*
    exit /b 0
)

if exist "%ROOT_DIR%\desktop-host\bin\Debug\net8.0\Neoveg.exe" (
    start "" "%ROOT_DIR%\desktop-host\bin\Debug\net8.0\Neoveg.exe" %*
    exit /b 0
)

where dotnet >nul 2>&1
if "%ERRORLEVEL%"=="0" (
    dotnet run --project "%ROOT_DIR%\desktop-host\Neoveg.csproj" -- %*
    exit /b %ERRORLEVEL%
)

echo [ERROR] Executavel Neoveg.exe nao encontrado. Execute 'python scripts/build_portable.py --targets windows'.
exit /b 1
