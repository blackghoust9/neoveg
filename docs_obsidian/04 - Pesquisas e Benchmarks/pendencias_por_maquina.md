---
tags: [neoveg, roadmap, hardware, pendencias]
---
# Pendências por máquina

O que dá e o que não dá para fazer **no PC de desenvolvimento atual**, com o motivo de cada caso. Cada item aponta a linha do `README.md` (seção *Roteiro*) onde ele está. Volta ao hub: [[Index]]. Itens parecidos e o catálogo geral: [[lacunas_funcionais]]. Medidas de desempenho: [[desempenho_resolucoes]].

> Clean room (AGENTS.md §28): a nota descreve **funções** com palavras próprias, sem nomes de marca de terceiros. Data da conferência: 30/10/2026.

## Hardware e ferramentas desta máquina

| Parte | O que há |
|---|---|
| Processador | Intel Core i5-7500T, 4 núcleos e 4 threads, 2,7 GHz (7ª geração) |
| Memória | 7,9 GB de RAM (o antivírus residente e o Claude Code já usam uns 900 MB) |
| Vídeo | Intel HD Graphics 630, gráfico integrado com 1 GB compartilhado, sem GPU dedicada |
| Disco | SSD SATA de 224 GB, com 38 GB livres (um `target/` de depuração passa de 3 GB) |
| Áudio | Realtek High Definition Audio (entrada e saída estéreo), sem interface MIDI |
| Sistema | Windows 10 Pro 22H2, 64 bits |
| Compilador | Rust 1.99.1 (MSVC), CMake 4.4.4, Python 3.14, Git, Docker Desktop |
| FFmpeg | só o programa de linha de comando 9.0.2 (build completo, sem cabeçalhos nem bibliotecas de desenvolvimento) |
| Faltam | LLVM 19.x (há só o LLVM-MinGW), FFmpeg 7.x de desenvolvimento, `vcpkg`, `pkg-config`, qualquer plug-in VST3 ou OFX, tela HDR, dispositivos de captura |

**Regra de trabalho nesta máquina:** só se acrescenta o que ainda não existe; rodam `cargo fmt`, `cargo check` e `cargo clippy`, um crate por vez com `-j 1`. Os testes ficam para outra máquina, porque o `cargo test` do `neoveg_ui` e do `neoveg_engine` inteiro estoura a memória e é parado pelo Claude Code. O perfil de desenvolvimento usa `debug = 0` por esse motivo.

## 1. Não dá para fazer (ou conferir) neste PC

| README | Item | Motivo |
|---|---|---|
| 599 | Superfícies de controle MIDI, joystick, controle remoto pela rede, MIDI timecode e clock | Não há interface MIDI, joystick nem segundo aparelho para conferir; só o áudio Realtek |
| 616 | Captura de tela e de câmeras, DVD, HDV e Hub de mídia | Sem dispositivo de captura, leitor de DVD nem câmera |
| 600 | Saída para disco óptico e envio direto a sites de vídeo | Sem gravador óptico; o envio exige contas e chaves de serviços online |
| 466, 329 | Decodificação AV1/HEVC/AVC por GPU, HEIC/HEIF | O gráfico integrado desta geração decodifica AVC e HEVC, mas **não AV1**; o caminho por GPU exige as bibliotecas do FFmpeg, ausentes aqui; HEIC pede um decodificador HEVC que o projeto não tem |
| 623, 624 | Modos reais de visualização da prévia (HDR) | Não há tela HDR para ver o resultado |
| 340 | Vídeo com `ffmpeg-next` (LLVM 19.x) e subir a versão da crate | Falta o LLVM 19.x e o FFmpeg 7.x de desenvolvimento; instalar e compilar isso cabe mal em 38 GB livres e 8 GB de RAM |
| 341, 318 | CI e pacote portátil nos três sistemas | Só há Windows; o Docker Desktop precisaria de uma máquina virtual de 4 GB ou mais e a compilação do workspace dentro dela estoura a memória |
| 452, 557, 621 | IA: edição por texto, ampliação (upscale), reenquadramento automático, máscara inteligente, rastreamento automático, Z-Depth | Modelos de segmentação e ampliação levam minutos por quadro em 4 núcleos sem GPU dedicada e passam da RAM; a transcrição por Whisper já existe e roda só com modelos pequenos |
| 393, 597, 615 | Formatos RAW de câmeras de cinema, MXF e formatos profissionais de câmera e de transmissão | Exigem kits de desenvolvimento proprietários e arquivos de amostra que não temos |
| 368, 381, 618, 595 (parte) | Host de plug-ins OFX, VST3 em processo isolado com gerenciador, efeitos de terceiros em eventos, compensação de atraso de plug-ins | O código pode ser escrito, mas não há nenhum plug-in instalado (nem a pasta VST3) para validar; precisa de plug-ins de teste |
| 327, 328, 332, 336, 590 | Itens "conferir em uso" | Pedem olho humano ou controle de tela, que não existe nesta sessão; só o dono do projeto confere |
| 367 | Medir a GPU com o benchmark de reprodução para escolher a primeira família de efeitos | O resultado só valeria para este gráfico integrado de 1 GB, que não representa as GPUs dos usuários |
| 343 | Falha intermitente de linkagem (`LNK1201`/`LNK1104`) | Só aparece depois de vários rebuilds seguidos, e cada rebuild completo aqui chega a ser parado por falta de memória; falta reproduzir (suspeita: antivírus residente) |
| 307, 318, 393 | Scripting do usuário, pacote portátil, quais formatos de câmera valem o custo | Estão "em decisão": não é limite de hardware, falta a escolha do dono do projeto |

## 2. Dá só em parte (o código sim, a validação limitada)

| README | Item | O que limita |
|---|---|---|
| 429, 573 | Gravação de áudio: armar faixa, takes em laço, punch-in, monitoração | A entrada Realtek existe, mas latência e monitoração se medem com microfone e ouvido; o código dá |
| 595 | Surround 5.1 e Surround Panner | Não há saída 5.1: só dá para renderizar para arquivo e conferir pelo medidor |
| 596 | 3D estereoscópico (lado a lado, anaglífico, linha alternada) | Os modos se conferem por captura fora da tela; falta tela 3D |
| 464, 612 | Denoiser, controle de cintilação, preenchimento de barras pretas, estabilização | Em tempo real não cabe no i5-7500T; só em exportação, mais lenta |
| 450, 574 | Pré-renderização com régua verde/amarela/vermelha, proxies de vídeo, cache de picos em disco | Dá o código, mas os 38 GB livres acabam rápido com proxies de projetos grandes |
| 439, 459, 556, 594, 622 | Roda de cor, Color Match, curvas de cor, fluxos S-Log/ACES e OCIO | Matrizes, LUTs 3D e curvas dão em Rust; o OCIO oficial é C++ e a visualização HDR não se vê |
| 550 | Modos de composição na GPU com teste contra a CPU | O `wgpu` roda no gráfico integrado, mas a rodada de testes de GPU fica para outra máquina |
| 563, 619 | Detectar cenas, detecção de batidas e andamento, multicâmera por áudio | Dá com o `ffmpeg` de linha de comando e correlação em CPU; lento em material longo |

## 3. Dá para fazer neste PC (código, com `check` e `clippy`)

**Edição e linha do tempo**
- Caixas de início, fim e duração com valor relativo (476); arrastar com `Ctrl` copia e menu do botão direito (479, 562); `[` e `]` selecionam a borda (481); inserir evento vazio (484).
- Ripple por escopo, `TrimMode`, aparo adjacente, embaralhar e modo de edição expandida A/B (402, 487 a 492, 334, 456); evento em laço com quadros ilegíveis (493); flag de deslocamento de snap por evento (335, 499).
- Marcadores: mover e redimensionar etiquetas, marcadores de comando e de mídia (407, 521, 522); subclipes e copiar o quadro atual (529); importar estéreo como mono duplo (530).
- Faixas: grupos, duplicar, cor e altura (534); faixas pai e filho (410, 549); faixa e evento de ajuste (372); Fechar espaços, Reunir evento e Divisão inteligente (561, 377); L-cut, J-cut e vínculos de sincronização (460, 514); reparo de dessincronia (613).
- Seleção de canal em mídia multicanal (510), takes (512, 587), esticar com preservação de formantes (389), interruptores de vídeo que faltam (374, 508).

**Automação e áudio**
- O tipo `Envelope` genérico e os envelopes que faltam (fade, opacidade, progresso de transição): 361, 366, 503, 504, 411.
- Editor gráfico dos envelopes, pincel livre e afinar pontos (542); gravação de automação Touch e Latch (543, 535); editor de keyframes de Track Motion, Pan/Crop e efeitos (544).
- Cadeias de efeitos de áudio atribuíveis, predefinições dos efeitos e importação de presets (537, 382); efeitos padrão ao importar áudio (602).

**Vídeo e cor**
- Máscaras de forma (retângulo, elipse, Bézier) e máscara de efeito (548, 458, 621 em parte); cadeias de até 32 efeitos (458, 555).
- Curvas de cor (364, 622), textura de ruído (365), categorias, predefinições e miniaturas de transições (434).
- Painel de compositores e modos de mistura na GPU (387, 432, 550); proxies inteligentes (457, 554).

**Projeto, mídia e interface**
- Propriedades do projeto e da mídia (388, 448, 568, 569, 614); formatos de régua (571); modelos de projeto (601); janela de Mídia do Projeto com filtros, bins e smart bins (449, 575, 617).
- Salvamento incremental, exportar e importar preferências, trocar mídia por pasta (378, 463, 553); arquivar projetos (611); layouts nomeados, janelas destacadas e mapas de teclado nomeados (386, 412, 572, 581).
- Atalhos que faltam: projeto e janelas, visualização, cursor, edição, teclado numérico, altura e takes, reprodução, faixas (582 a 589); tabela única de ações ligada aos menus de contexto e tooltips (560, 562, 564).
- Alertas e diálogos padronizados (453); linha de comando `neoveg --open` (598); sub-projeto com visão da linha do tempo pai (620); `.ico` incluído no `cargo build` (339).
- Legendas: MCC, SMI e RT, canais CC2 a CC4 e `[BR]`, sincronizar com `Ctrl+K`, legendas a partir de regiões ou de `.txt` e `.csv` (593); estilo por trecho e parágrafos com largura fixa nos títulos (444).
- Formatos de especificação pública: PSD em camadas e Broadcast Wave (615); storyboards (610).
- Documentação: registrar os quatro itens da análise de lacunas (346), rever o menu Faixas (394).

## 4. Itens que parecem já feitos (conferir e marcar)

A conferência por busca no código mostra que estas caixas estão atrasadas em relação ao que existe:

| README | O que há no código |
|---|---|
| 405 | Travar evento existe (interruptor e cadeado); falta só conferir a trava de faixa |
| 415, 536 | Barramentos (até 26), sends e cadeia de efeitos do barramento já estão no roteiro como feitos |
| 465, 372, 557 | `adjustment.rs`: evento de ajuste já existe; falta o redimensionamento ao projeto |
| 515, 612 | Reverter evento: a velocidade negativa já reproduz ao contrário; falta o subclipe revertido |
| 563 | Congelar quadro no cursor existe (`velocity::freeze_command`) |
| 539 | O registro `_loud.txt` e a medição offline existem em `loudness_report` e no render |
| 556 | Os escopos (histograma, forma de onda e vectorscope) e os LUTs `.cube` existem; faltam Color Match, parada RGB e OCIO |
| 573, 429 | `neoveg_audio::record` já grava; falta punch-in, takes em laço e monitoração |
| 411, 504 | O volume por evento e a velocidade estão prontos; faltam fade, opacidade e progresso de transição |
| 487, 489, 491 | O texto diz "engine pronto" ou "feito", mas a caixa ficou aberta: separar o que falta (ligar menus e ferramentas) |

Conferir a lista item a item continua pendente; esta tabela só cobre o que foi encontrado por busca.

**Marcadas em 30/10/2026** (conferidas no código): eventos de ajuste, barramentos com sends pré/pós-fader e laço recusado, registro `_loud.txt` com medição offline, exportar e importar preferências, duplicar faixa e Fechar espaços (as duas últimas e a das preferências foram implementadas neste dia).
