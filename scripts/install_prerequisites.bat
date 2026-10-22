@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul 2>&1

echo ============================================================================
echo  Neoveg — Instalador Unificado Silencioso de Pre-Requisitos
echo  1. Visual C++ Redistributable 2015-2022 (vcredist2022_2015 / All-in-One)
echo  2. DirectX End-User Runtimes June 2010 (d3d11.dll / XAudio2_7.dll)
echo ============================================================================

:: Solicita elevacao de administrador (UAC) caso ainda nao esteja elevado
net session >nul 2>&1
if not "%errorLevel%"=="0" (
    echo [UAC] Solicitando aprovacao de Administrador para instalar pre-requisitos...
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%~f0' -WorkingDirectory '%~dp0..' -Verb RunAs -Wait"
    exit /b %ERRORLEVEL%
)

set "SCRIPT_DIR=%~dp0"
set "ROOT_DIR=%SCRIPT_DIR%.."
set "VCREDIST_REL=addon\vcredist2022-2015_14.36.32532"
set "VCREDIST_DIR=%ROOT_DIR%\%VCREDIST_REL%"
set "VCREDIST_AIO_BAT=%ROOT_DIR%\addon\vcredist_all_in_one\install_all.bat"
set "DX_REDIST_DIR=%ROOT_DIR%\addon\directx_redist"

if not exist "%VCREDIST_DIR%" (
    if exist "%CD%\%VCREDIST_REL%" (
        set "VCREDIST_DIR=%CD%\%VCREDIST_REL%"
    )
)

echo [INFO] Pasta VC++ Redistributable: "%VCREDIST_DIR%"
echo [INFO] Nota: O pacote legado .NET 4.8 (ndp48-web.exe) e ignorado pois o
echo        Neoveg e distribuido em modo Self-Contained (.NET 8.0 embutido).

:: ----------------------------------------------------------------------------
:: ETAPA 1: Visual C++ Redistributable (vcredist2022_2015 / All-in-One)
:: ----------------------------------------------------------------------------
set /a FOUND_COUNT=0
if exist "%VCREDIST_AIO_BAT%" (
    set /a FOUND_COUNT+=1
    echo [EXEC] Executando pacote TechPowerUp Visual C++ All-in-One: "%VCREDIST_AIO_BAT%" ...
    call "%VCREDIST_AIO_BAT%"
)

if exist "%VCREDIST_DIR%" (
    for %%F in ("%VCREDIST_DIR%\*vcredist*.exe" "%VCREDIST_DIR%\*VC_redist*.exe") do (
        if exist "%%~fF" (
            set /a FOUND_COUNT+=1
            echo [EXEC] Instalando silenciosamente: "%%~nxF" /q /norestart ...
            start /wait "" "%%~fF" /q /norestart
            set "EXIT_CODE=!ERRORLEVEL!"
            if "!EXIT_CODE!"=="0" (
                echo [OK] "%%~nxF" instalado com sucesso ^(Codigo 0^).
            ) else if "!EXIT_CODE!"=="1638" (
                echo [OK] Uma versao mais recente do VC++ Redistributable ja esta instalada ^(Codigo 1638^).
            ) else if "!EXIT_CODE!"=="3010" (
                echo [OK] "%%~nxF" instalado com sucesso — reinicializacao suprimida por /norestart ^(Codigo 3010^).
            ) else (
                echo [WARN] "%%~nxF" retornou o codigo !EXIT_CODE!.
            )
        )
    )
)

if !FOUND_COUNT! EQU 0 (
    echo [INFO] Nenhum instalador executavel vcredist2022_2015 encontrado em "%VCREDIST_REL%".
    if exist "%SystemRoot%\System32\vcruntime140.dll" (
        echo [OK] Runtime Microsoft Visual C++ 2015-2022 ^(vcruntime140.dll^) ja presente no sistema.
    ) else (
        echo [WARN] Baixe o pacote em https://www.techpowerup.com/download/visual-c-redistributable-runtime-package-all-in-one/
    )
)

:: ----------------------------------------------------------------------------
:: ETAPA 2: DirectX End-User Runtimes (June 2010 / TechPowerUp)
:: Comando padrao: directx_Jun2010_redist.exe /q /t:%TEMP%\dxredist && %TEMP%\dxredist\DXSETUP.exe /silent
:: ----------------------------------------------------------------------------
echo.
echo [INFO] Verificando pacote DirectX End-User Runtimes (June 2010)...
set "DX_REDIST_EXE="
for %%D in ("%DX_REDIST_DIR%" "%ROOT_DIR%\addon" "%VCREDIST_DIR%" "%CD%\addon\directx_redist" "%CD%\addon") do (
    if exist "%%~fD\directx_Jun2010_redist.exe" (
        set "DX_REDIST_EXE=%%~fD\directx_Jun2010_redist.exe"
    )
)

if defined DX_REDIST_EXE (
    echo [EXEC] Extraindo e instalando silenciosamente DirectX Runtimes:
    echo        "%DX_REDIST_EXE%" /q /t:"%TEMP%\dxredist" ^&^& "%TEMP%\dxredist\DXSETUP.exe" /silent
    if not exist "%TEMP%\dxredist" mkdir "%TEMP%\dxredist"
    "%DX_REDIST_EXE%" /q /t:"%TEMP%\dxredist" && "%TEMP%\dxredist\DXSETUP.exe" /silent
    set "DX_EXIT=!ERRORLEVEL!"
    echo [OK] DirectX End-User Runtimes (June 2010) concluido ^(Codigo !DX_EXIT!^).
) else if exist "%DX_REDIST_DIR%\DXSETUP.exe" (
    echo [EXEC] Executando "%DX_REDIST_DIR%\DXSETUP.exe" /silent ...
    "%DX_REDIST_DIR%\DXSETUP.exe" /silent
) else (
    echo [INFO] Instalador directx_Jun2010_redist.exe nao encontrado localmente em addon\directx_redist\.
    echo        Fonte oficial: https://www.techpowerup.com/download/directx-redistributable-runtime/
    if exist "%SystemRoot%\System32\d3d11.dll" (
        echo [OK] Aceleracao Visual DirectX 11 ^(d3d11.dll^) detectada em System32.
    )
    if exist "%SystemRoot%\System32\XAudio2_7.dll" (
        echo [OK] Aceleracao de Audio DirectX June 2010 ^(XAudio2_7.dll^) detectada em System32.
    ) else (
        echo [INFO] XAudio2_7.dll ausente ^(opcional caso XAudio2_9.dll do Windows 10/11 esteja ativo^).
    )
)

echo ============================================================================
echo  Pre-requisitos do Neoveg (Visual C++ ^& DirectX Runtimes) verificados.
echo ============================================================================
exit /b 0
