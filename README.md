# Neoveg

Editor não linear de vídeo e áudio multiplataforma, escrito em **Rust puro** (Cargo workspace com `egui`/`eframe`, `wgpu`, `cpal`, `symphonia`).

> **Em migração.** A branch `refactor/100-percent-rust` substitui a arquitetura anterior (C# / Avalonia, Rust FFI, C++ e Python) por um único workspace Rust. Os diretórios legados (`desktop-host/`, `desktop-host.Tests/`, `rust-core/`, `cpp-extensions/`, `python-backend/`) continuam no repositório apenas como referência até cada recurso ser portado e, em seguida, serão removidos.

Consulte [AGENTS.md](./AGENTS.md) para os padrões de arquitetura e codificação (Parte I) e [REQUIREMENTS.md](./REQUIREMENTS.md) para o detalhe de versões.

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

### Ambiente de referência (o que foi instalado na máquina de desenvolvimento)

Resumo do que o projeto usa hoje, para reproduzir o ambiente em outra máquina Windows:

- [x] Rust estável 1.99.1 (`rustup`), alvo `x86_64-pc-windows-msvc`, componentes `clippy`, `rustfmt`, `llvm-tools-preview`, `rust-src` e `rust-docs`
- [x] Visual Studio 2026 / Build Tools com *Desktop development with C++* (linker MSVC)
- [x] `cargo-llvm-cov` 0.9.1 (`cargo install cargo-llvm-cov --locked`)
- [x] FFmpeg 7.1.x compartilhado LGPL (`BtbN.FFmpeg.LGPL.Shared.7.1`) com `FFMPEG_DIR` apontando para a pasta extraída
- [x] LLVM / `libclang` 19.x com `LIBCLANG_PATH=C:\Program Files\LLVM\bin` (o LLVM 22+ quebra a `ffmpeg-sys-next`)
- [x] Git e GitHub Desktop (fluxo de branches descrito no AGENTS.md, seção 11)
- [ ] Nenhum dos itens legados (.NET SDK, CMake, Python) é necessário para o workspace Rust

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
- [x] Edição de eventos com o mouse: mover (inclusive entre faixas do mesmo tipo), aparar nas duas bordas (a fonte mantém o alinhamento e não passa do começo nem do fim da mídia), alinhamento a quadros, ao cursor e às bordas de outros eventos (`F8`; `Alt` ignora), rolagem automática nas bordas, `Esc` cancela e cada arrasto é um único Desfazer
- [x] As 5 curvas de fade (linear, rápida, lenta, suave, acentuada) com alças nos cantos do evento, menu do botão direito e Inspetor; crossfade automático em sobreposições parciais (ligável em Editar e na barra)
- [x] Importação de áudio (WAV, MP3, FLAC, OGG, AAC) em thread de fundo
- [x] Importação de vídeo (MP4, MOV, MKV, AVI, WebM...) com eventos de vídeo e áudio ligados e formato do projeto definido pelo primeiro clipe
- [x] Decodificação de vídeo na prévia: thread de fundo, busca por quadro exato, qualidade Draft/Preview/Good/Best/Auto (feature `ffmpeg`)
- [x] Mixer multifaixa: soma todos os eventos de áudio ativos, com fade-in/out e `trim_in`
- [x] Medidores L/R reais no Barramento Master, zerados com a reprodução parada
- [x] Projetos `.neoveg` (ZIP com `project.json`, `media_pool.json`, `history.json`): abrir, salvar, salvar como
- [x] Autosave (padrão 2 min, configurável em Editar > Preferências) e recuperação após falha: `recovery_*.neoveg` em `%LOCALAPPDATA%\Neoveg\autosave` (Linux `~/.local/share/neoveg/autosave`, macOS `~/Library/Application Support/neoveg/autosave`); aviso de alterações não salvas ao sair e `*` no título
- [x] Relink de mídia ausente ao abrir: nome exato e depois nome aproximado (passos 1 e 2)
- [x] Log central em `logs/neoveg.log` e `logs/crash.log`

### A fazer (em ordem de prioridade)

- [ ] Importar imagens estáticas e sequências de imagens
- [ ] Cache de quadros decodificados e pré-busca durante a reprodução (hoje cada quadro é decodificado sob demanda)
- [ ] Composição de várias faixas de vídeo na prévia (hoje aparece só a faixa de vídeo mais alta sob o cursor)
- [ ] Reprodução de vídeo sincronizada ao relógio de áudio (hoje o relógio é o do mixer; quadros podem ser descartados)
- [ ] Janela de mídia ausente (passo 3: validar duração/canais com metadados) e relink manual
- [ ] Slip, slide e stretch de eventos; ripple; agrupar (`G`) e desagrupar (`U`); seleção múltipla
- [ ] Aparar com o mouse em eventos de mídia gerada (hoje o limite de fonte só vale para mídia importada)
- [ ] Marcadores (`M`), regiões e loop
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

### A fazer a partir da pesquisa dos manuais e do `results.md`

Fontes de referência (paridade funcional de NLE clássico; registrar achados em `docs/research/nle_features_research.txt`): manuais VEGAS Pro 10 (fundamentos e DSP de áudio), 11 (Titler, GPU, geradores), 12 (fluxos de edição expandidos), 13 (docking, correção de cor, pipelines de FX), 18 (UI escura moderna, rastreamento de movimento) e 21 (ferramentas de IA), além do inventário `results.md` (menus, diálogos, comandos e atalhos). Os links dos manuais estão no histórico do pedido; o servidor do PDF do VP12 bloqueia download automático (HTTP 403) e os PDFs do VP10 e VP18 passam de 10 MB, então precisam ser baixados e lidos manualmente.

**Edição e timeline**
- [ ] Modos de edição (Ripple off/eventos/faixa/tudo) e *Auto Ripple*
- [ ] Slip, slide, stretch e *velocity* (envelope de velocidade) de eventos
- [ ] Seleção múltipla, agrupar (`G`) e desagrupar (`U`), travar faixa/evento
- [ ] Marcadores (`M`), regiões, marcadores de comando e loop; réguas adicionais
- [ ] Trimmer com marcação de entrada/saída e inserção na timeline (três/quatro pontos)
- [ ] Aninhar projetos (*nesting*) e faixas de composição/pai-filho
- [ ] Envelopes e keyframes (volume, pan, opacidade, FX) com interpolação e automação
- [ ] Personalizar teclado, barras de ferramentas e layouts de janela (salvar/restaurar)

**Áudio**
- [ ] Volume, pan, mute e solo por faixa; Mixing Console com barramentos, sends e inserts
- [ ] Efeitos de áudio nativos (EQ, compressor, reverb, delay) e hospedagem VST3
- [ ] Gravação de áudio, entradas de dispositivo e metrônomo

**Vídeo e composição**
- [ ] Composição multicamada na prévia e modos de composição (mistura)
- [ ] Track Motion, Pan/Crop de evento com gizmo `'F'` e keyframes
- [ ] Transições (categorias, presets, parâmetros) e efeitos de vídeo na GPU; OFX
- [ ] Correção/gradação de cor, LUTs (OCIO) e scopes de vídeo (histograma, parada RGB, vectorscope)
- [ ] Geradores e Titler/Texto com animação; legendas
- [ ] Rastreamento de movimento, máscaras e *Smart Mask*

**Projeto, mídia e saída**
- [ ] Projeto: propriedades (resolução, FPS, aspecto, espaço de cor), templates e perfis de mídia
- [ ] Janela de Mídia do Projeto (listas/miniaturas, filtros, busca, propriedades) e Explorer
- [ ] Renderização (*Make Movie*) com predefinições e pré-renderização/cache (régua verde/amarela/vermelha)
- [ ] Intercâmbio FCP7 XML, FCPXML e EDL
- [ ] IA: edição por texto, fala para texto, *upscale*, *auto reframe*
- [ ] Alertas e diálogos padronizados (mídia ausente, codec, disco, GPU, plug-in) conforme o catálogo do `results.md` §10

## Créditos e licenciamento

O código do projeto é distribuído sob a licença [MIT](./LICENSE). Consulte [AUTHORS.md](./AUTHORS.md) e [third_party/README.md](./third_party/README.md) para atribuições e licenças das dependências. O FFmpeg usado nos builds de desenvolvimento é a variante **LGPL** compartilhada; preserve esses arquivos ao redistribuir pacotes.
