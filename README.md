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

Fontes de referência (paridade funcional de NLE clássico; registrar achados em `docs/research/nle_features_research.txt`): manuais VEGAS Pro 10 (fundamentos e DSP de áudio), 11 (Titler, GPU, geradores), 12 (fluxos de edição expandidos), 13 (docking, correção de cor, pipelines de FX), 18 (UI escura moderna, rastreamento de movimento) e 21 (ferramentas de IA), além do inventário `results.md` (menus, diálogos, comandos e atalhos). Os PDFs ficam em `manuais/` (ignorada pelo Git por causa do tamanho); a lista abaixo já incorpora a leitura deles.

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

**Itens extras vindos dos manuais (VP10–VP21), por versão de origem**
- [ ] Modo de edição expandido e *rolling edit* pelo teclado numérico, com visão 2-up do ponto de edição (VP12)
- [ ] Proxies inteligentes: troca automática entre original e proxy conforme a qualidade da prévia; o render usa o original (VP12)
- [ ] Cadeias de efeitos de até 32 efeitos aplicáveis ao evento, faixa, mídia ou projeto; mascaramento de FX e formas (retângulo, elipse, Bézier) (VP12)
- [ ] Correspondência de cor entre clipes (*Color Match*) e fluxo S-Log/ACES com OpenColorIO por mídia e projeto (VP12)
- [ ] Edições em L-cut e J-cut, fades simultâneos de áudio e vídeo e sincronismo de eventos (*sync links*) (VP12/VP13)
- [ ] Edição multicâmera (sincronizar, criar evento multicâmera, alternar câmeras) e *takes* como versões alternativas do evento (VP13)
- [ ] Salvamento incremental (`Ctrl+Alt+S`), exportar/importar preferências e trocar arquivos de mídia por pasta (VP18)
- [ ] Painel de rastreamento de movimento e Black Bar Fill, Denoiser, Flicker Control (VP18)
- [ ] Eventos de ajuste (*adjustment events*): o efeito vale para os eventos de vídeo abaixo dele (VP21)
- [ ] Decodificação AV1/HEVC/AVC com aceleração de GPU e suporte a HEIC/HEIF (VP21)
- [ ] Opções para ocultar ou não criar arquivos de picos de áudio (`.sfk` no VEGAS; equivalente no Neoveg) (VP21)

### Plano de implementação detalhado (entradas 7 a 16 da pesquisa)

Cada tarefa cita a entrada de [`docs/research/nle_features_research.txt`](./docs/research/nle_features_research.txt) que descreve o comportamento esperado. Regras de edição ficam como funções puras em `neoveg_engine` com testes; a UI só traduz o ponteiro em chamadas (AGENTS.md 2.3). Todo campo novo de arquivo usa `#[serde(default)]` e tem teste com arquivo antigo.

**Fase 1 — Seleção e comandos de edição básicos (entrada 7)**
- [ ] `Selection` no engine: conjunto de ids de eventos + intervalo de tempo opcional; Shift = faixa contígua, `Ctrl` = alternar, `Ctrl+A` = tudo, "Selecionar eventos até o fim", seleção por retângulo
- [ ] Caixas de início/fim/duração da seleção que aceitam valor absoluto ou relativo (`+10`, `-5` na unidade da régua)
- [ ] Cortar, copiar, colar e excluir com seleção de tempo (divide eventos nas bordas, todas as faixas) e com eventos selecionados (só os selecionados)
- [ ] Colar por inserção (`Ctrl+Shift+V`, divide o evento sob o cursor e empurra os seguintes) e colar repetido (`Ctrl+B`, ponta a ponta ou a cada X)
- [ ] Arrastar com `Ctrl` copia; arrastar com o botão direito abre "Mover aqui / Copiar aqui / Embaralhar"
- [ ] Aparar à seleção (`Ctrl+T`), aparar início/fim ao cursor (`Alt+[` / `Alt+]`), `[` e `]` selecionam a borda do evento
- [ ] `S` divide sem seleção em todas as faixas sob o cursor e, com seleção de tempo, nas duas bordas
- [ ] Inserir evento vazio e inserir tempo (espaço em branco no cursor)

**Fase 2 — Ripple, aparar e edição expandida (entrada 8)**
- [ ] `RippleScope` (faixas afetadas / + marcadores e regiões / tudo) e `ripple_after(edição, escopo)`; teclas `F`, `Ctrl+F`, `Ctrl+Shift+F`; modo Auto Ripple (`Ctrl+L`) com lista de escopos
- [ ] `TrimMode` {Normal, Slip, Slide, Stretch, Roll}; slip e slide por mouse e por teclado; resolver o conflito `Alt` (hoje ignora o snap) antes de adotar `Alt` = slip
- [ ] Aparar por teclado: selecionar borda, 1 quadro / 1 pixel por passo, modificadores de compressão, slip e slide de crossfade; roda do mouse com `Ctrl+Shift`
- [ ] Aparo adjacente (rolling) arrastando a borda entre dois eventos; cortes em J e L ignorando o agrupamento
- [ ] Embaralhar eventos (shuffle) por menu e por ferramenta
- [ ] Modo de edição expandida: visão A/B, quadros não usados dos dois lados, pré-visualização dividida, saída por `Esc`
- [ ] Evento em repetição (switch Loop) ao esticar além da mídia; quadros ilegíveis em vermelho

**Fase 3 — Snap e quantização (entrada 9)**
- [ ] `SnapSources` (grade, marcadores, todos os eventos, cursor, bordas da seleção) com atalhos `Ctrl+F8`, `Shift+F8`, `Ctrl+Shift+F8` além de `F8`
- [ ] `Shift` inverte o snap temporariamente (decidir a convivência com o `Alt` atual)
- [ ] Indicador de snap (linha na faixa ou na altura toda; tracejada quando a quantização arredonda)
- [ ] Flag de deslocamento de snap por evento (triângulo no canto inferior esquerdo)
- [ ] Preferência `quantize_audio_edits` (não quantizar edições só de áudio)

**Fase 4 — Envelopes, chaves e propriedades do evento (entrada 10)**
- [ ] Tipo genérico `Envelope` em `neoveg_common` (pontos tempo/valor/curva, com a curva `Hold` extra) avaliado no mixer e no compositor
- [ ] Envelopes de evento: volume (dB), fade, opacidade, velocidade (negativa = reverso, presets de -100 % a 300 %) e progresso de transição
- [ ] Switches por evento: mudo, travar, repetir, inverter fase, normalizar (com recalcular), manter proporção, reduzir cintilação, reamostragem Smart/Force/Disable
- [ ] Seleção não destrutiva de canal/stream de áudio (ambos, esquerdo, direito, combinar, trocar, canal x)
- [ ] Esticar no tempo e mudar o tom (teclas `=` e `-`), taxa de reprodução e subamostragem de vídeo
- [ ] Takes por evento (adicionar como take, `T` / `Shift+T`, escolher/apagar ativo, "slip em todos os takes")
- [ ] Grupos (`G`, `U`, `Ctrl+Shift+U`, selecionar grupo) e vínculos de sincronização de um sentido; reparo de dessincronia A/V (mover ou deslizar)
- [ ] Reverter evento (cria subclipe revertido)

**Fase 5 — Marcadores, regiões e laço (entrada 11)**
- [ ] `markers: Vec<Marker>` no projeto (`#[serde(default)]`), barra de marcadores acima da régua, `M` com nome inline, teclas `0`–`9` saltam para marcadores
- [ ] Regiões (`R`), mover/redimensionar etiquetas, apagar tudo / apagar na seleção, ferramenta de marcador com seleção múltipla
- [ ] Marcadores de comando (comando, parâmetro, comentário, modelos) e marcadores de mídia mostrados nos eventos (snap opcional)
- [ ] Região de laço (`Shift+Q`), reprodução em laço (`Q`) e "renderizar só a região de laço"

**Fase 6 — Trimmer (entrada 12)**
- [ ] `TrimmerState` (mídia, entrada, saída, cursor) com histórico; `I` e `O` marcam entrada e saída
- [ ] Edição em três pontos (adicionar a partir de / até o cursor, `A` / `Shift+A`) e em quatro pontos (ajustar para preencher)
- [ ] Subclipes (com opção de reverso), abrir mídia-mãe, copiar/salvar quadro atual
- [ ] Modo sobrescrever linha do tempo e importar estéreo como mono duplo

**Fase 7 — Faixas, mixer e barramentos (entrada 13)**
- [ ] Volume, pan (leis Add Channels / Balance 0, -3, -6 dB / Constant Power), mudo, solo (exclusivo com `Ctrl`) e inverter fase por faixa; operar em todas as faixas selecionadas
- [ ] Grupos de faixas (recolher, nomear, mudo/solo de grupo), duplicar faixa, cor e altura de faixa, minimizar/maximizar
- [ ] Faixa de áudio: ajuste (trim) somado ao envelope; botão de automação e modos Off/Read/Touch/Latch
- [ ] Barramentos (até 26 + Master), roteamento sem laços, sends pré/pós-fader, "vincular ao pan da faixa"
- [ ] Cadeias de FX atribuíveis (envio/retorno) e FX de barramento
- [ ] Medidores de loudness (M, S, I, LRA, pico verdadeiro; EBU R128 e ATSC A/85) em painel próprio e registro `_loud.txt`

**Fase 8 — Automação e keyframes (entrada 14)**
- [ ] Edição de envelopes: pontos, `Ctrl` fino, `Alt` só tempo, pincel livre, afinar pontos, inverter, tipo de curva por segmento
- [ ] Gravação de automação (Touch e Latch) e "travar envelopes aos eventos"
- [ ] Editor de keyframes (anterior/próximo, adicionar/remover, suavidade) para Track Motion, Pan/Crop e FX

**Fase 9 — Movimento, máscaras e composição (entrada 15)**
- [ ] Editor de Track Motion / Pan/Crop (retângulo com alças, rotação, painel numérico, menu de contexto, sombra e brilho)
- [ ] Máscaras (âncoras, positivo/negativo, pena, formas retângulo/elipse) e máscara de FX
- [ ] `parent: Option<TrackId>`, `composite_mode` e `level` por faixa; árvore pai/filho no engine
- [ ] Modos de composição na GPU (`wgpu`) com teste contra referência na CPU: Source Alpha, Add, Subtract, Multiply, Screen, Overlay, Hard Light, Dodge, Burn, Darken, Lighten, Difference, Difference Squared, Cut; modo personalizado (Displacement/Height/Bump)

**Fase 10 — Fluxos de trabalho (entrada 16)**
- [ ] Salvamento incremental (`Ctrl+Alt+S`), exportar/importar preferências, trocar mídia por pasta (reusar o relink)
- [ ] Proxies inteligentes (troca automática conforme a qualidade da prévia; render usa o original)
- [ ] Cadeias de até 32 efeitos por evento, faixa, mídia e projeto, com marcadores Pan/Crop e Composite
- [ ] Color Match, LUTs/OpenColorIO, scopes (vectorscope, forma de onda, histograma, parada RGB)
- [ ] Projetos aninhados e eventos de ajuste; painel de rastreamento de movimento; ferramentas de IA (transcrição, edição por texto, máscara, upscale)

**Fase 11 — Menus, comandos e preferências (entradas 26 a 29)**
- [ ] Tabela única de ações (`Command`: id pontuado, rótulo i18n, atalho, predicado de habilitação) usada pelo menu principal, menus de contexto, tooltips e pela janela "Personalizar teclado"; overrides em `preferences.json` (`keymap`)
- [ ] Comandos baratos que cabem no motor atual: Fechar espaços (Close Gaps), Reunir evento (Event Heal), Divisão inteligente (Smart Split), Selecionar tudo após o cursor, Zoom na seleção, Desfazer tudo e Limpar histórico, Colar atributos (inteiro e seletivo)
- [ ] Menus de contexto de evento, faixa, superfície de vídeo e arrastar com o botão direito (Mover aqui / Copiar aqui / Embaralhar; Adicionar em sequência / em faixas / como takes)
- [ ] Detectar cenas e dividir (filtro de cena do FFmpeg), Congelar quadro no cursor (envelope de velocidade), detecção de batidas e de andamento
- [ ] Novas chaves de preferência (todas com `#[serde(default)]`): `quantize_audio_edits`, `quick_fade_ms`, `still_image_seconds`, `cut_to_overlap`, `envelope_fade`, `preserve_pitch_on_stretch`, `interface_theme`, `thumbnails_mode`, `import_stereo_as_dual_mono`, `create_fx_undos`, suavidade padrão de keyframes, áreas seguras e divisões da grade
- [ ] Faixas e eventos especiais: faixa de pasta, faixa e evento de ajuste, aninhar/abrir linha do tempo aninhada, auto-ducking (controlador/ouvinte), inserir legendas de arquivo `.txt`/`.csv` ou de regiões

**Fase 12 — Propriedades, régua e espaço de trabalho (entradas 30 a 35)**
- [ ] Propriedades do projeto: formato de pixel, ordem de campos, taxa de amostragem e profundidade de bits, início da régua, BPM e compasso, pastas de gravação e pré-renderização, espaço de cor (ACES/OCIO) — tudo com valor padrão para abrir projetos antigos
- [ ] Propriedades de mídia: tipo de canal alfa (indefinido, nenhum, direto, pré-multiplicado, pré-multiplicado sujo), espaço e faixa de cor, rotação, timecode do arquivo ou personalizado, edição de várias mídias com "(valores diferentes)"
- [ ] Janela de mídia ausente com as quatro opções (pesquisar, nova localização, ignorar, ignorar todas) e janela de erro de arquivo de mídia
- [ ] Formatos de régua (`RulerFormat`): amostras, tempo, segundos, tempo e quadros, quadros absolutos, compassos e tempos, pés e quadros 16/35 mm, SMPTE 23,976 / 24 / 25 / 29,97 sem e com drop-frame / 30, tempo de CD; espaçamento da grade (minutos a quadros, notas musicais) e deslocamento da régua
- [ ] Layouts de janela nomeados (salvar, carregar, 10 atalhos, organizar) além de painéis flutuantes; janela Edit Details (tabela sobre eventos, marcadores, regiões e comandos, com colunas ordenáveis e predefinições); histórico de desfazer com lista e rótulo por comando
- [ ] Gravação de áudio (armar faixa, takes em laço, punch-in com pré/pós-roll, monitoração Off/On/Auto, metrônomo) com entrada `cpal`
- [ ] Cache de picos por mídia em disco (equivalente aos arquivos `.sfk`), com reconstrução manual e opção para desligar; proxies de vídeo (`Draft`/`Preview` usam o proxy, `Good`/`Best` o original; automático acima de 4K)
- [ ] Biblioteca de mídia com tags hierárquicas, paleta de tags e tags automáticas a partir do caminho da pasta

## Créditos e licenciamento

O código do projeto é distribuído sob a licença [MIT](./LICENSE). Consulte [AUTHORS.md](./AUTHORS.md) e [third_party/README.md](./third_party/README.md) para atribuições e licenças das dependências. O FFmpeg usado nos builds de desenvolvimento é a variante **LGPL** compartilhada; preserve esses arquivos ao redistribuir pacotes.
