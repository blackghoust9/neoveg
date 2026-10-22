@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
for %%I in ("%SCRIPT_DIR%..") do set "APP_ROOT=%%~fI"
set "VCREDIST_DIR=%APP_ROOT%\addon\vcredist_all_in_one"
set "INSTALL_ALL=%VCREDIST_DIR%\install_all.bat"
if not exist "%INSTALL_ALL%" (
    set "VCREDIST_DIR=%APP_ROOT%\addon\vcredist2022-2015_14.36.32532"
    set "INSTALL_ALL=%VCREDIST_DIR%\install_all.bat"
)
set "SYSTEM_DIR=%SystemRoot%\System32"

if exist "%SYSTEM_DIR%\vcruntime140.dll" if exist "%SYSTEM_DIR%\vcruntime140_1.dll" if exist "%SYSTEM_DIR%\msvcp140.dll" if exist "%SYSTEM_DIR%\msvcp140_1.dll" (
    echo [OK] Visual C++ 2015-2022 runtime is already installed.
    exit /b 0
)

if not exist "%INSTALL_ALL%" (
    echo [ERROR] Visual C++ Redistributable All-in-One package was not found.
    echo Download it from:
    echo https://www.techpowerup.com/download/visual-c-redistributable-runtime-package-all-in-one/
    echo Extract install_all.bat and its accompanying files to:
    echo "%VCREDIST_DIR%"
    exit /b 2
)

if /i not "%~1"=="/elevated" (
    echo [INFO] Windows administrator approval is required to install Visual C++ runtimes.
    powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$p = Start-Process -FilePath '%~f0' -ArgumentList '/elevated' -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
    exit /b %ERRORLEVEL%
)

pushd "%VCREDIST_DIR%"
if errorlevel 1 (
    echo [ERROR] Could not access "%VCREDIST_DIR%".
    exit /b 3
)

echo [INFO] Installing Visual C++ Redistributables silently.
call "%INSTALL_ALL%"
set "INSTALL_EXIT_CODE=%ERRORLEVEL%"
popd

if "%INSTALL_EXIT_CODE%"=="0" (
    if exist "%SYSTEM_DIR%\vcruntime140.dll" if exist "%SYSTEM_DIR%\vcruntime140_1.dll" if exist "%SYSTEM_DIR%\msvcp140.dll" if exist "%SYSTEM_DIR%\msvcp140_1.dll" (
        echo [OK] Visual C++ runtime installation completed.
        exit /b 0
    )
    echo [ERROR] Installer returned success, but required runtime DLLs are still missing.
    exit /b 4
)

if "%INSTALL_EXIT_CODE%"=="3010" (
    echo [WARN] Installation completed; Windows may require a restart.
    exit /b 0
)

echo [ERROR] install_all.bat returned exit code %INSTALL_EXIT_CODE%.
exit /b %INSTALL_EXIT_CODE%
