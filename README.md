# Neoveg

Editor não linear de vídeo e áudio multiplataforma, escrito em **Rust puro** (Cargo workspace com `egui`/`eframe`, `wgpu`, `cpal`, `symphonia`).

> **Em migração.** A branch `refactor/100-percent-rust` substitui a arquitetura anterior (C# / Avalonia, Rust FFI, C++ e Python) por um único workspace Rust. Os diretórios legados (`desktop-host/`, `desktop-host.Tests/`, `rust-core/`, `cpp-extensions/`, `python-backend/`) continuam no repositório apenas como referência até cada recurso ser portado e, em seguida, serão removidos.

Consulte [AGENTS.md](./AGENTS.md) para os protocolos de arquitetura (seção 22: *Pure Rust Architecture & Single-Toolchain Protocol*) e [REQUIREMENTS.md](./REQUIREMENTS.md) para o detalhe de versões.

## Pré-requisitos para contribuidores

A única dependência **obrigatória** é a toolchain oficial do Rust. Não são necessários .NET, CMake, compilador C++ nem Python. Os itens opcionais só são exigidos para cobertura de código e para a decodificação de vídeo com FFmpeg.

| Componente | Versão | Obrigatório? | Para que serve | Windows (`winget` / `rustup`) |
| --- | --- | --- | --- | --- |
| Rust (`rustup`, `cargo`, `rustc`) | 1.88+ estável | **Sim** | Compilar tudo (`crates/neoveg_*`) | `winget install --id Rustlang.Rustup --exact` |
| `clippy`, `rustfmt` | da toolchain | **Sim** | Lint e formatação (`-D warnings` na QA) | `rustup component add clippy rustfmt` |
| Visual Studio com *Desktop development with C++* (linker MSVC) | 2022 ou 2026 | **Sim, no Windows** | Linker do alvo `x86_64-pc-windows-msvc` | instalador do Visual Studio / Build Tools |
| `llvm-tools-preview` | da toolchain | Opcional | Relatórios de cobertura | `rustup component add llvm-tools-preview` |
| `cargo-llvm-cov` | 0.9+ | Opcional | `cargo llvm-cov --workspace` | `cargo install cargo-llvm-cov --locked` |
| FFmpeg **compartilhado** com `include/` e `lib/` | **7.1.x** (não 8/9) | Só para `--features ffmpeg` | Decodificação de vídeo (`ffmpeg-next` 7.1) | `winget install --id BtbN.FFmpeg.LGPL.Shared.7.1 --exact` |
| LLVM / `libclang` | **19.x** (22+ quebra a `ffmpeg-sys-next` 7.1.3) | Só para `--features ffmpeg` | `bindgen` gera as ligações do FFmpeg | `winget install --id LLVM.LLVM --exact --version 19.1.7` |

Variáveis de ambiente para o build com FFmpeg (Windows, escopo do usuário; abra um terminal novo depois de defini-las):

```powershell
# pasta extraída do pacote, a que contém bin\, include\ e lib\
[Environment]::SetEnvironmentVariable('FFMPEG_DIR', "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BtbN.FFmpeg.LGPL.Shared.7.1_Microsoft.Winget.Source_8wekyb3d8bbwe\ffmpeg-n7.1.5-12-g1fdbca85aa-win64-lgpl-shared-7.1", 'User')
[Environment]::SetEnvironmentVariable('LIBCLANG_PATH', 'C:\Program Files\LLVM\bin', 'User')
```

O nome da pasta do FFmpeg muda a cada versão do pacote: confira o caminho real em `%LOCALAPPDATA%\Microsoft\WinGet\Packages`. Em tempo de execução (inclusive `cargo test --features ffmpeg`) as DLLs do FFmpeg (`bin\`) precisam estar no `PATH` do terminal; o pacote do `winget` adiciona essa pasta ao `PATH`, então abra um terminal novo depois de instalar. Os testes de decodificação geram um clipe com o executável `ffmpeg` do `PATH` e são ignorados em silêncio quando ele não existe. Sem a feature `ffmpeg` o Neoveg abre normalmente e avisa na barra de status que vídeo está desabilitado.

**Linux (Debian / Ubuntu):**

```bash
sudo apt update && sudo apt install -y build-essential pkg-config \
    libasound2-dev libudev-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    libxkbcommon-dev libwayland-dev libvulkan-dev
# opcional, para --features ffmpeg:
sudo apt install -y libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libclang-dev
```

**macOS:** `xcode-select --install` e, para o FFmpeg, `brew install ffmpeg@7 llvm pkg-config`.

## Compilar, testar e executar

```bash
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo llvm-cov --workspace --summary-only           # opcional: cobertura
cargo run --release -p neoveg_ui                    # abre o Neoveg com um projeto vazio (sem vídeo)
cargo run --release -p neoveg_ui --features ffmpeg  # com decodificação de vídeo na prévia
cargo test --workspace --features neoveg_ui/ffmpeg  # inclui os testes de decodificação reais
```

Logs: `logs/neoveg.log` e `logs/crash.log` ao lado do executável.

## Estrutura do workspace

| Crate | Responsabilidade |
| --- | --- |
| `crates/neoveg_common` | Tipos de projeto, timecode SMPTE (drop-frame), log e preferências. |
| `crates/neoveg_audio` | Decodificação (`symphonia`), mixer multifaixa, saída de áudio (`cpal`), medidores de pico reais e waveform amostra a amostra. |
| `crates/neoveg_video` | Quadros RGBA, geradores procedurais (barras SMPTE), upload para GPU (`wgpu`) e decodificação opcional (`ffmpeg-next`). |
| `crates/neoveg_engine` | Timeline, faixas, eventos, comandos com Desfazer/Refazer, projetos `.neoveg` (ZIP) e relink de mídia ausente. |
| `crates/neoveg_ui` | Aplicação `egui` com painéis acopláveis, slider de precisão, régua com grade de frames adaptativa e cabeçalhos de faixa. |

## Roteiro (o que já está pronto e o que falta)

### Feito

- [x] Workspace Cargo com as cinco crates, `clippy -D warnings`, `rustfmt` e testes passando
- [x] Janela principal (`eframe` + `wgpu`) com projeto vazio e painéis acopláveis/destacáveis
- [x] Waveform amostra a amostra (min/max/RMS por coluna; amostras individuais com pontos em zoom alto)
- [x] Grade de frames adaptativa e régua SMPTE com timecode drop-frame
- [x] Playhead de 1 pixel físico com piscar do cursor em pausa
- [x] Slider de precisão (polegar quadrado, duplo clique restaura, `Ctrl` = ajuste fino, menu de contexto)
- [x] Timeline: faixas de áudio/vídeo, dividir (`S`), mover/remover eventos, Desfazer/Refazer
- [x] Importação de áudio (WAV, MP3, FLAC, OGG, AAC) em thread de fundo
- [x] Importação de vídeo (MP4, MOV, MKV, AVI, WebM...) com eventos de vídeo e áudio ligados e formato do projeto definido pelo primeiro clipe
- [x] Decodificação de vídeo na prévia: thread de fundo, busca por quadro exato, qualidade Draft/Preview/Good/Best/Auto (feature `ffmpeg`)
- [x] Mixer multifaixa: soma todos os eventos de áudio ativos, com fade-in/out e `trim_in`
- [x] Medidores L/R reais no Barramento Master, zerados com a reprodução parada
- [x] Projetos `.neoveg` (ZIP com `project.json`, `media_pool.json`, `history.json`): abrir, salvar, salvar como
- [x] Relink de mídia ausente ao abrir: nome exato e depois nome aproximado (passos 1 e 2)
- [x] Log central em `logs/neoveg.log` e `logs/crash.log`

### A fazer (em ordem de prioridade)

- [ ] Importar imagens estáticas e sequências de imagens
- [ ] Cache de quadros decodificados e pré-busca durante a reprodução (hoje cada quadro é decodificado sob demanda)
- [ ] Composição de várias faixas de vídeo na prévia (hoje aparece só a faixa de vídeo mais alta sob o cursor)
- [ ] Reprodução de vídeo sincronizada ao relógio de áudio (hoje o relógio é o do mixer; quadros podem ser descartados)
- [ ] Autosave e recuperação (`projects/autosave/recovery_*.neoveg`)
- [ ] Janela de mídia ausente (passo 3: validar duração/canais com metadados) e relink manual
- [ ] Arrastar, aparar (trim), slip e redimensionar eventos com o mouse; snapping (`F8`), ripple, agrupar (`G`) e desagrupar (`U`)
- [ ] Marcadores (`M`), regiões e loop
- [ ] As 5 curvas de fade (linear, rápida, lenta, suave, acentuada) e crossfade automático entre eventos sobrepostos
- [ ] Volume, pan, mute e solo por faixa; barramentos e sends; plug-ins de áudio (VST3 via crate Rust)
- [ ] Efeitos de vídeo e transições na GPU (`wgpu`), composição e Pan/Crop do evento
- [ ] Geradores de mídia (títulos, cor sólida, padrão de teste) e janela de títulos
- [ ] Cabeçalho de evento (mídia gerada, Pan/Crop, FX, menu) e inspetor de evento
- [ ] Renderização/exportação (Make Movie) com FFmpeg
- [ ] Importação/exportação FCP7 XML, FCPXML e CMX3600 EDL
- [ ] Transcrição de fala para legendas (Whisper) e chaveamento de croma (CorridorKey)
- [ ] Scripting para o usuário em Rust (substituir Roslyn/Python)
- [ ] Preferências: aplicar o idioma (`pt-BR` padrão + 7 idiomas) em toda a interface
- [ ] Temas (Classic Foundry, Classic Silver, Modern Carbon Dark, Contemporary Electric Dark)
- [ ] Abas no rodapé em todos os painéis (hoje só o painel de Mídia; o `egui_dock` não posiciona a barra embaixo)
- [ ] Medidores de pico e cópia de logs no visualizador de logs da interface
- [ ] Empacotamento portátil (Windows, Linux, macOS x64/arm64) e CI com `cargo build/test/clippy/llvm-cov`
- [ ] Remover os diretórios legados (`desktop-host/`, `rust-core/`, `cpp-extensions/`, `python-backend/`) e atualizar `Dockerfile`, `.gitignore` e scripts de QA para o workspace Rust

## Créditos e licenciamento

O código do projeto é distribuído sob a licença [MIT](./LICENSE). Consulte [AUTHORS.md](./AUTHORS.md) e [third_party/README.md](./third_party/README.md) para atribuições e licenças das dependências. O FFmpeg usado nos builds de desenvolvimento é a variante **LGPL** compartilhada; preserve esses arquivos ao redistribuir pacotes.
