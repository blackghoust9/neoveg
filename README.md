# Neoveg

Editor não linear de vídeo e áudio multiplataforma, desenvolvido com C# / Avalonia, Rust FFI, C++ e ferramentas Python para mídia.

Consulte [AGENTS.md](./AGENTS.md) para os protocolos de arquitetura e desenvolvimento, [RELEASE_NOTES.md](./RELEASE_NOTES.md) para o resumo da release 1.0.0 e [REQUIREMENTS.md](./REQUIREMENTS.md) para preparar o ambiente de desenvolvimento.

## Pré-requisitos para contribuidores

Instale tudo abaixo **antes** de compilar, testar ou rodar a suíte de QA. A suíte (`scripts/run_qa_suite.ps1` / `.sh`) falha de propósito se qualquer ferramenta estiver ausente.

| Ferramenta | Versão mínima | Para que serve |
| --- | --- | --- |
| .NET SDK | 8.0 | Interface C# / Avalonia (`desktop-host`) e testes (`desktop-host.Tests`) |
| Rust (via `rustup`) + `clippy` + `llvm-tools-preview` | 1.75 | Núcleo `rust-core`, lint e cobertura |
| `cargo-llvm-cov` | 0.6 | Cobertura de código do Rust |
| CMake + Ninja | 3.20 / 1.11 | Build das extensões C++ (`cpp-extensions`) |
| Compilador C/C++ **GCC ou Clang em modo GNU** (com ASan/UBSan) | C++17 | Build instrumentado do QA. **O MSVC/clang-cl é recusado** pelo `CMakeLists.txt` quando sanitizers/cobertura estão ligados |
| `clang-tidy` | 15 | Análise estática C++ |
| Python + `pip` | 3.10 | Backend Python, scripts e testes |
| `pytest`, `pytest-cov`, `gcovr` (pip) | 7.4 / 5.0 / 7.0 | Testes Python, cobertura Python e relatório de cobertura C++ |
| Docker (Desktop no Windows/macOS) | 24.0 | Build multi-stage e QA headless (`Dockerfile`, `docker-compose.yml`) |
| FFmpeg | — | Já incluso em `runtimes/<rid>/`; usado para análise de mídia e waveforms de MP3/MP4 |

### Windows 10/11

Em um PowerShell (sem administrador; o Docker Desktop pede elevação via UAC durante a instalação):

```powershell
winget install --id Microsoft.DotNet.SDK.8 --exact
winget install --id Rustlang.Rustup --exact
winget install --id Kitware.CMake --exact
winget install --id Ninja-build.Ninja --exact
winget install --id MartinStorsjo.LLVM-MinGW.UCRT --exact   # clang/clang++ (GNU), clang-tidy, llvm-cov, ASan/UBSan
winget install --id Python.Python.3.12 --exact
winget install --id Docker.DockerDesktop --exact             # requer WSL2 e ~4 GB de disco para as imagens

rustup component add clippy llvm-tools-preview
cargo install cargo-llvm-cov --locked
python -m pip install --user -r python-backend/requirements.txt gcovr

# O LLVM-MinGW não traz "gcov": o gcovr usa o llvm-cov no lugar
setx GCOV "llvm-cov gcov"
```

Depois, **abra um novo terminal** (para carregar o PATH atualizado) e confirme que os comandos `pytest`, `gcovr`, `clang`, `clang-tidy`, `ninja` e `cargo llvm-cov` respondem. Se o `pip` avisar que a pasta `...\Python\Python3xx\Scripts` não está no PATH, adicione-a às variáveis de ambiente do usuário.

**Rust instalado pelo MSI (sem `rustup`):** o comando `rustup component add llvm-tools-preview` não existe nesse caso. Aponte o `cargo-llvm-cov` para o LLVM do LLVM-MinGW, desde que a versão principal seja a mesma do LLVM do Rust (`rustc --version --verbose` → `LLVM version`):

```powershell
setx LLVM_COV "<pasta-do-llvm-mingw>\bin\llvm-cov.exe"
setx LLVM_PROFDATA "<pasta-do-llvm-mingw>\bin\llvm-profdata.exe"
```

Para **executar** o Neoveg no Windows também são necessários o Visual C++ Redistributable e o DirectX End-User Runtime: rode `scripts\install_prerequisites.bat` (pede UAC). Veja a seção 6.1 do [AGENTS.md](./AGENTS.md).

### Linux (Debian / Ubuntu)

```bash
sudo apt update && sudo apt install -y build-essential clang clang-tidy cmake ninja-build pkg-config \
    python3 python3-pip python3-venv libgl1-mesa-dev libx11-dev libxrandr-dev libxi-dev libasound2-dev \
    dotnet-sdk-8.0 docker.io docker-compose-v2
sudo usermod -aG docker "$USER"   # saia e entre na sessão para valer
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy llvm-tools-preview
cargo install cargo-llvm-cov --locked
python3 -m pip install --user -r python-backend/requirements.txt gcovr
```

### macOS (Intel e Apple Silicon)

```bash
xcode-select --install
brew install cmake ninja llvm python@3.12 gcovr
brew install --cask dotnet-sdk docker
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy llvm-tools-preview
cargo install cargo-llvm-cov --locked
python3 -m pip install --user -r python-backend/requirements.txt
# clang-tidy vem do LLVM do Homebrew: adicione "$(brew --prefix llvm)/bin" ao PATH
```

### Verificar o ambiente

```powershell
python scripts/check_env.py                                   # diagnóstico rápido das ferramentas
dotnet build desktop-host/Neoveg.csproj                       # interface
cargo test --manifest-path rust-core/Cargo.toml               # núcleo Rust
dotnet test desktop-host.Tests/Neoveg.Tests.csproj            # testes .NET (inclui Avalonia.Headless)
powershell -ExecutionPolicy Bypass -File scripts/run_qa_suite.ps1   # suíte completa (Windows)
./scripts/run_qa_suite.sh                                     # suíte completa (Linux/macOS)
```

## Executar a versão portátil no Windows

O pacote de referência é gerado em `dist\Neoveg-Portable-win-x64\`. Para executá-lo:

```powershell
.\dist\Neoveg-Portable-win-x64\Neoveg.exe
```

Também é possível iniciar pelo `Executar_Neoveg.cmd` na raiz ou na pasta do pacote. Ao detectar execução em unidade sincronizada ou de rede, o bootstrap pode copiar a aplicação para `%LOCALAPPDATA%\Neoveg\AppCache` e iniciá-la dali para evitar problemas de I/O e carregamento de bibliotecas.

## Gerar o pacote portátil

No Windows, com as ferramentas indicadas em [REQUIREMENTS.md](./REQUIREMENTS.md), gere o alvo Windows:

```powershell
python .\build_portable.py --targets windows
```

O pipeline cria o pacote em `dist\Neoveg-Portable-win-x64\`, publica o executável .NET em Release, monta os recursos portáteis e atualiza `dist\build_manifest.json`. No Windows, o pipeline também executa `scripts\sync_gdrive.ps1` para sincronizar o workspace e os pacotes de distribuição com o destino configurado no script. Para outros alvos, o builder aceita `linux`, `macos` ou `all`; a geração de binários nativos de cada plataforma deve ser feita em ambiente compatível.

Para uma publicação .NET self-contained separada do pacote portátil completo:

```powershell
dotnet publish desktop-host\Neoveg.csproj -c Release -r win-x64 --self-contained true -o dist\Neoveg_v1.0.0_win-x64_Portable
```

O projeto copia as DLLs nativas Windows para a saída de build/publicação. Na inicialização, o resolvedor procura primeiro junto ao executável e depois no AppCache; a validação de startup registra os caminhos encontrados. Os pacotes também carregam `Assets/`, scripts, runtimes e arquivos legais necessários ao formato portátil.

## Arquitetura

| Subsistema | Implementação | Responsabilidade |
| --- | --- | --- |
| Interface desktop | C# / Avalonia UI | Workspace de edição, timeline multifaixa, cabeçalho de eventos, régua de renderização, painéis e localização. |
| Núcleo NLE e DSP | Rust (`neoveg_core`) | Operações de timeline, timecode SMPTE, fades, crossfades, análise de áudio e cache de forma de onda. |
| Extensões nativas | C++17 / CMake (`neoveg_cpp_engine`) | Pontes para processamento de efeitos e áudio, interoperabilidade nativa e transferência de frames. |
| Mídia e automação | Python | Integração com FFmpeg/ffprobe, análise de mídia, automação e ferramentas de QA. |

## QA e estado de validação

Estado em 23/10/2026 (Windows 10, toolchain completo instalado conforme a seção de pré-requisitos):

- `dotnet test desktop-host.Tests` — 12 testes passando (inclui Avalonia.Headless e a grade de frames).
- `cargo test` (`rust-core`) — 6 testes passando. Cobertura Rust (`cargo llvm-cov`): **~39% das linhas** (`lib.rs` ~16%).
- `scripts/run_qa_suite.ps1` — **falha**: passa no build C++ instrumentado e na formatação .NET, e para no `cargo clippy -D warnings` (17 erros pré-existentes em `rust-core/src/lib.rs` e `audio_peaks.rs`). Os passos seguintes ainda não foram exercitados.
- `ctest` não tem testes C++ registrados (não há GoogleTest/`add_test` em `cpp-extensions/CMakeLists.txt`).
- `python-backend/test_vegas_engine.py` verifica a presença de símbolos/textos nos arquivos e grava `coverage/qa/qa_report.json` como "pass"; **ele não substitui a suíte de QA**. O resultado válido é o gravado por `run_qa_suite.ps1`/`.sh`.

**Limitações conhecidas:**

- Os pacotes portáteis Linux/macOS em `dist/` contêm bibliotecas nativas de placeholder (1–2 KB; a `.dylib` "arm64" é x86_64). Gere-os em ambiente nativo ou via Docker antes de distribuir.
- A prévia ainda não decodifica vídeo (clipes de vídeo aparecem como gráfico sintético) e não há saída de áudio na reprodução.
- Os pacotes em `dist/` e o atalho `Executar_Neoveg.cmd` podem estar desatualizados em relação ao código-fonte; gere o pacote novamente após mudanças.

## Créditos e licenciamento

O código do projeto é distribuído sob a licença [MIT](./LICENSE). Consulte [AUTHORS.md](./AUTHORS.md) e [third_party/README.md](./third_party/README.md) para atribuições e licenças das dependências. Preserve esses arquivos ao redistribuir os pacotes.
