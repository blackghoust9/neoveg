# Instala o LLVM 19 e o FFmpeg 7.1 (LGPL, compartilhado) que a feature `ffmpeg` do Neoveg exige no Windows.
# Rode no PowerShell COMO ADMINISTRADOR. Baixa dos enderecos oficiais, confere o SHA256, instala o LLVM em
# Program Files\LLVM, extrai o FFmpeg em C:\Tools e define FFMPEG_DIR e LIBCLANG_PATH para a maquina
# (nao mexe no PATH). O LLVM tem de ser o 19: o 22 ou mais novo quebra a `ffmpeg-sys-next` 7.1.
# O endereco do FFmpeg e uma versao fixa da BtbN; se ela sair do ar, troque a URL e o SHA256 abaixo.
# Depois use scripts\cargo-ffmpeg.ps1 para compilar e testar.
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$log = Join-Path $here 'install_llvm_ffmpeg.log'
Start-Transcript -Path $log -Force | Out-Null
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $ProgressPreference = 'SilentlyContinue'
    $work = Join-Path $env:TEMP 'neoveg_llvm_ffmpeg'
    New-Item -ItemType Directory -Force $work | Out-Null

    function Get-Verified($url, $sha256, $name) {
        $file = Join-Path $work $name
        Write-Host "Baixando $name ..."
        Invoke-WebRequest -Uri $url -OutFile $file -UseBasicParsing
        $hash = (Get-FileHash $file -Algorithm SHA256).Hash
        if ($hash -ne $sha256.ToUpper()) { throw "SHA256 de $name nao confere: $hash" }
        Write-Host "SHA256 de $name confere"
        return $file
    }

    "== LLVM 19.1.7 =="
    $llvm = Get-Verified 'https://github.com/llvm/llvm-project/releases/download/llvmorg-19.1.7/LLVM-19.1.7-win64.exe' 'f19ae5bc4823ac69ec01dc2ded503ec80a04ad2208dda1595d1f0413c148ef90' 'LLVM-19.1.7-win64.exe'
    $p = Start-Process -FilePath $llvm -ArgumentList '/S' -Wait -PassThru
    "instalador do LLVM saiu com $($p.ExitCode)"
    if (-not (Test-Path "$env:ProgramFiles\LLVM\bin\clang.exe")) { throw 'clang.exe nao apareceu em Program Files\LLVM\bin' }

    "== FFmpeg 7.1.5 compartilhado (LGPL) =="
    $zip = Get-Verified 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-07-31-14-10/ffmpeg-n7.1.5-12-g1fdbca85aa-win64-lgpl-shared-7.1.zip' '0f376f96fb38554ccefb1b2ae9c7c6a7b351f0e60a372b38262c320e8392c5d0' 'ffmpeg-7.1.5-lgpl-shared.zip'
    $tools = 'C:\Tools'
    New-Item -ItemType Directory -Force $tools | Out-Null
    Expand-Archive -Path $zip -DestinationPath $tools -Force
    $ffmpegDir = Get-ChildItem $tools -Directory -Filter 'ffmpeg-n7.1*' |
        Where-Object { Test-Path (Join-Path $_.FullName 'include\libavcodec\avcodec.h') } |
        Select-Object -First 1 -ExpandProperty FullName
    if (-not $ffmpegDir) { throw 'Nao achei include\libavcodec\avcodec.h depois de extrair' }

    [Environment]::SetEnvironmentVariable('FFMPEG_DIR', $ffmpegDir, 'Machine')
    [Environment]::SetEnvironmentVariable('LIBCLANG_PATH', "$env:ProgramFiles\LLVM\bin", 'Machine')

    "== Conferencia =="
    & "$env:ProgramFiles\LLVM\bin\clang.exe" --version
    "FFMPEG_DIR = $ffmpegDir"
    "avcodec.lib existe: " + (Test-Path "$ffmpegDir\lib\avcodec.lib")
    "LIBCLANG_PATH = $env:ProgramFiles\LLVM\bin"
    "CONCLUIDO"
} catch {
    "ERRO: $_"
} finally {
    Stop-Transcript | Out-Null
    Start-Sleep -Seconds 8
}
