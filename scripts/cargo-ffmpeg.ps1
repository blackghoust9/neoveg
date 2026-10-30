# Roda o cargo com o ambiente que a feature `ffmpeg` do Neoveg exige no Windows.
# Uso (PowerShell comum, na pasta do repositório):
#   .\scripts\cargo-ffmpeg.ps1 check -p neoveg_video --features ffmpeg -j 1
#   .\scripts\cargo-ffmpeg.ps1 test  -p neoveg_ui --features ffmpeg -j 2 -- --test-threads=1
#
# O que ele acerta (conferido em 30/10/2026, `cargo check` e `clippy` da feature passaram):
#  - FFMPEG_DIR e LIBCLANG_PATH vêm da sessão, do usuário ou da máquina, nessa ordem
#    (install_llvm_ffmpeg.ps1 as define para a máquina);
#  - o clang do LLVM 19 vai na frente do PATH: um clang do LLVM-MinGW no PATH faz o bindgen ler
#    os cabeçalhos do MinGW e falhar em time.h;
#  - INCLUDE aponta para o Visual Studio e o SDK do Windows: sem ele o libclang não acha errno.h;
#  - as DLLs do FFmpeg 7 entram no PATH desta sessão, para os testes carregarem as bibliotecas.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'

function Get-Setting($name) {
    foreach ($scope in 'Process', 'User', 'Machine') {
        $value = [Environment]::GetEnvironmentVariable($name, $scope)
        if ($value) { return $value }
    }
    return $null
}
$ffmpeg = Get-Setting 'FFMPEG_DIR'
$libclang = Get-Setting 'LIBCLANG_PATH'
if (-not $ffmpeg -or -not (Test-Path "$ffmpeg\include\libavcodec\avcodec.h")) { throw 'FFMPEG_DIR nao definido: rode install_llvm_ffmpeg.ps1 como administrador' }
if (-not $libclang -or -not (Test-Path "$libclang\libclang.dll")) { throw 'LIBCLANG_PATH nao definido: rode install_llvm_ffmpeg.ps1 como administrador' }

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw 'Visual Studio (ou as Build Tools) nao encontrado: o libclang precisa dos cabecalhos do MSVC' }
$vs = & $vswhere -latest -products * -property installationPath
$msvc = (Get-ChildItem "$vs\VC\Tools\MSVC" -Directory | Sort-Object Name | Select-Object -Last 1).FullName
$kits = "${env:ProgramFiles(x86)}\Windows Kits\10\Include"
$sdk = (Get-ChildItem $kits -Directory | Sort-Object Name | Select-Object -Last 1).FullName

$env:FFMPEG_DIR = $ffmpeg
$env:LIBCLANG_PATH = $libclang
$env:CLANG_PATH = "$libclang\clang.exe"
$env:INCLUDE = "$msvc\include;$sdk\ucrt;$sdk\shared;$sdk\um"
$env:Path = "$libclang;$ffmpeg\bin;$env:Path"

& cargo @CargoArgs
exit $LASTEXITCODE
