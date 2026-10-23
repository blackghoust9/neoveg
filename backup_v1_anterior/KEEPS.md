# KEEPS.md — Dossiê Enciclopédico do Neoveg & Arquitetura do OpenNeoveg 21

> **Referência Histórica, Técnica, Comunitária e Arquitetural Completa**
> Baseado na documentação oficial do Neoveg, no artigo e fontes da Wikipédia ([en.wikipedia.org/wiki/Neoveg_Pro](https://en.wikipedia.org/wiki/Neoveg_Pro)), nos arquivos históricos da *Sonic Foundry*, *Classic NLE Creative Software*, *Modern NLE / Neoveg Creative Software* e *Contemporary NLE*, bem como nos debates técnicos de comunidades (*Reddit r/NeovegPro*, *Official Neoveg Forum*, *Creative COW*, *JetDV* e *Tools4Neoveg*).

---

## 1. Visão Geral e Origem Histórica (Wikipédia & Fontes Primárias)

O **Neoveg** (estilizado originalmente como *Neoveg* e comercializado entre 2003 e 2016 como **Classic NLE** / **Classic NLE**) é uma suíte profissional de edição não-linear (*Non-Linear Editing* — NLE) para o sistema operacional Microsoft Windows.

Diferente de outros editores de vídeo que nasceram focados apenas em corte de fita ou película (como o Avid Media Composer ou o Adobe Premiere clássico), o Neoveg nasceu em **1999** como uma **estação de trabalho de áudio digital (DAW) multicanal de 24-bit/96kHz** chamada *Neoveg Audio* / *Neoveg Beta*, desenvolvida pela empresa norte-americana **Sonic Foundry** (sediada em Madison, Wisconsin). Essa origem explica por que até hoje a manipulação de eventos na timeline, os *crossfades* automáticos em tempo real, os envelopes de automação e o roteamento de áudio do Neoveg são considerados entre os mais rápidos e fluidos da indústria.

### As Quatro Eras Corporativas do Neoveg

| Era | Período | Proprietária / Desenvolvedora | Marcos Principais |
| :--- | :--- | :--- | :--- |
| **1ª Era** | **1999 – 2003** | **Sonic Foundry** | Beta público em junho de 1999; lançamento comercial do *Neoveg 1.0* (áudio multicanal), seguido de *Neoveg Video 2.0* (2000) e *Neoveg Video 3.0* (2001) e *Neoveg 4.0* (fevereiro de 2003). |
| **2ª Era** | **Maio 2003 – Maio 2016** | **Classic NLE Creative Software** | Em maio de 2003, a Classic NLE adquiriu os ativos de software da Sonic Foundry (*Neoveg*, *Sound Forge*, *ACID*) por US$ 18 milhões. Consolidou a marca mundial **Classic NLE** (versões 5.0 a 13.0). |
| **3ª Era** | **Maio 2016 – Março 2026** | **Modern NLE Software GmbH** (*Neoveg Creative Software*) | Em 24 de maio de 2016, a Classic NLE vendeu a maior parte de seu catálogo criativo para a alemã Modern NLE. Lançou o **Neoveg 14 a 22**, introduzindo NVENC/QSV, HDR, ACES, VST3, *Nested Timelines*, *Smart Masking AI*, *Z-Depth* e *Mocha Neoveg*. |
| **4ª Era** | **Março 2026 – Presente** | **Contemporary NLE** | No final de março de 2026, a **Contemporary NLE** adquiriu o Neoveg, Sound Forge e ACID Pro da Modern NLE, integrando profundamente a suíte com os pacotes de efeitos visuais *Sapphire*, *Continuum* e *Mocha Pro*. |

---

## 2. Revisão Cronológica Detalhada de Todas as Versões (1999 – 2026)

### 2.1. Era Sonic Foundry (1999–2003)
* **Neoveg 1.0 (1999)**: Focado exclusivamente em áudio multicanal (até 24-bit/96kHz), mixagem em tempo real via CPU (sem exigir placas DSP dedicadas como Pro Tools TDM), suporte a plugins DirectX e sincronização SMPTE/MIDI.
* **Neoveg Video 2.0 (2000)**: Adicionou faixas de vídeo mantendo a filosofia de "timeline livre de faixas rígidas", *crossfades* automáticos ao sobrepor dois eventos, *Event Pan/Crop* e efeitos DirectX/Video FX.
* **Neoveg Video 3.0 (dezembro de 2001)**: Captura e impressão DV (IEEE 1394 FireWire), *Time Stretch* contínuo, suporte a Windows XP (tema clássico cinza/azul lunar) e gerador de texto/créditos.
* **Neoveg 4.0 (fevereiro de 2003)**: Abandonou o sufixo "Video", introduziu o **DVD Architect 1.0**, mixagem nativa em **Surround 5.1**, suporte a drivers **ASIO**, correção de cor primária/secundária de 3 rodas (*3-Wheel Color Corrector*) e a **API de Scripting em JScript e VBScript (.NET)**.

### 2.2. Era Classic NLE Creative Software (2003–2016)
* **Classic NLE 5.0 (2004)**:
  * Composição **3D Track Motion** (perspectiva 3D de faixas com interseção de planos).
  * Máscaras vetoriais **Bézier** dentro da janela *Event Pan/Crop*.
  * Suporte a superfícies de controle de hardware (*Mackie Control Universal*, *Frontier Design TranzPort*).
  * Sub-buses de áudio e envelopes de transição personalizáveis.
* **Classic NLE 6.0 (abril de 2005)**:
  * Suporte nativo a **HDV (1080i / 720p)** e SDI com placas Blackmagic DeckLink / AJA.
  * Suporte a plugins de áudio **VST** (Steinberg Virtual Studio Technology).
  * Importação/exportação **AAF** (*Advanced Authoring Format*) e gerenciamento de projetos aninhados (`.veg` como clipe dentro de outro `.veg`).
* **Classic NLE 7.0 (setembro de 2006)**:
  * Tema clássico prateado da Classic NLE (*Classic NLE Silver*).
  * Suporte nativo a **Classic NLE XDCAM** (disco óptico profissional) e **AVCHD**.
  * Melhoria na pré-visualização em tela cheia em monitor secundário (*Windows Secondary Display*).
* **Classic NLE 8.0 (setembro de 2007)**:
  * Introdução do sufixo **"Pro"** no nome oficial.
  * Pipeline de processamento de vídeo em **32-bit Floating Point** (evitando *banding* em gradientes e permitindo níveis lineares de luz).
  * **Edição Multicâmera (Multicam)** nativa na timeline.
  * **ProType Titler** (gerador de títulos baseado em curvas spline e animação por caractere).
* **Classic NLE 9.0 (maio de 2009)**:
  * Interface escura (*Dark UI Palette*) introduzida pela primeira vez como padrão.
  * Resolução de projeto até **4K (4096×4096)**.
  * Suporte nativo a arquivos RAW de cinema **RED ONE (`.r3d`)** e Classic NLE XDCAM EX.
  * Novos efeitos de iluminação óptica: *Glint*, *Rays*, *Defocus*, *Soft Contrast*, *Starburst* e *Fill Light*.
* **Classic NLE 10.0 (outubro de 2010)**:
  * Arquitetura de plugins **OpenFX (OFX)** — padrão aberto da *The Foundry*, substituindo a antiga API DirectX Video FX e permitindo keyframes em quase todos os parâmetros.
  * **Aceleração de GPU** para codificação AVC (NVIDIA CUDA e AMD ATI Stream).
  * Edição e renderização em **3D Estereoscópico** (Side-by-Side, Anaglifo, Dual Stream MVC).
  * **Audio Event FX** (aplicação de cadeias de efeitos de áudio diretamente em um clipe individual, além da faixa e do Master Bus).
  * **Agrupamento de Faixas (Track Grouping)** inicial para organizar grandes projetos.
* **Classic NLE 11.0 (outubro de 2011)**:
  * Motor de processamento de vídeo acelerado por GPU via **OpenCL** (para placas AMD e NVIDIA) em dezenas de efeitos e transições nativas.
  * Novo gerador **Titles & Text** simplificado e animado.
  * **Sync Links** (vínculo unidirecional de sincronia entre eventos na timeline).
* **Classic NLE 12.0 (setembro de 2012)**:
  * Exclusivo para sistemas **Windows 64-bit**.
  * **Expanded Edit Mode** (edição dinâmica A/B roll com visualização dupla no monitor de preview).
  * Mascaramento de retângulo/oval para efeitos (*FX Masking* — permitindo borrar rostos ou placas rapidamente).
  * Gerenciamento de espaços de cor **S-Log** da Classic NLE.
  * Importação/exportação direta de timelines para **DaVinci Resolve (`.xml`)**, **Adobe Premiere Pro / Final Cut Pro 7**, **Final Cut Pro X (`.fcpxml`)** e **Avid Pro Tools (`.aaf`)**.
* **Classic NLE 13.0 (abril de 2014)**:
  * Última versão lançada pela Classic NLE antes da venda para a Modern NLE.
  * Nova barra de ferramentas inferior no Preview e cabeçalho compacto com abas *Edit / View / Tool*.
  * **Project Archiving** (arquivamento completo do projeto `.veg` copiando apenas as mídias utilizadas ou recortadas).
  * Medidores de **Loudness EBU R128 / CALM Act** (LUFS / LKFS / True Peak).

### 2.3. Era Modern NLE / Neoveg Creative Software (2016–2026)
* **Neoveg 14 (setembro de 2016)**:
  * Suporte nativo a **Apple ProRes** e **HEVC / H.265**.
  * Ferramenta *Smart Upscale* e *Smart Zoom* para converter HD em 4K.
  * Limite de *Velocity Envelope* ampliado para **1.000% (10x)** — combinável com *Playback Rate* (4x) para atingir **40x** de velocidade.
* **Neoveg 15 (agosto de 2017)**:
  * Interface moderna em **4 tons de cinza/escuro** (Dark, Medium, Light, White) com ícones redesenhados e cabeçalhos de faixa personalizáveis (*Hamburger Menu System*).
  * Codificação e decodificação acelerada por hardware **NVIDIA NVENC** e **Intel Quick Sync Video (QSV)**.
  * Plugins OFX interativos **Picture-in-Picture** e **Crop** com manipulação direta de *gizmo* na janela de Video Preview.
  * Plugin **LUT Filter** OFX nativo para carregar tabelas `.cube`.
* **Neoveg 16 (agosto de 2018)**:
  * **Rastreamento de Movimento (Motion Tracking)** integrado ao *Bézier Masking* (permitindo prender textos, efeitos ou borrões a objetos em movimento).
  * Suporte completo a **HDR10** e **HLG (Hybrid Log-Gamma)** com escopos de vídeo calibrados em *Nits (ST.2084 PQ)*.
  * **360° Video Editing** e estabilizador de vídeo reescrito.
* **Neoveg 17 (agosto de 2019)**:
  * **Nested Timelines** (Timelines Aninhadas sem precisar abrir uma segunda instância do programa).
  * Painel dedicado unificado de **Color Grading** com rodas de cor Lift/Gamma/Gain, curvas RGB e *Vectorscope* integrado.
  * **Optical Flow Slow Motion** e *Warp Flow Transition*.
  * Detecção automática de cortes de cena (*Scene Detection*) em vídeos longos.
* **Neoveg 18 (agosto de 2020)**:
  * Integração do pipeline **ACES 1.2** (*Academy Color Encoding System*).
  * Plugins de Inteligência Artificial: *Neoveg Style Transfer* e *Neoveg Colorization*.
  * Detecção automática de tremulação (*Flicker Control*) e redução de ruído de vídeo (*Denoise*).
* **Neoveg 19 (agosto de 2021)**:
  * **Adjustment Tracks** (Faixas de Ajuste que aplicam efeitos e correção de cor a todas as faixas abaixo).
  * Ferramentas de nuvem: *Text-to-Speech* neural e transcrição *Speech-to-Text* para legendas automáticas.
* **Neoveg 20 (agosto de 2022)**:
  * Suporte oficial a plugins de áudio **VST3 de 64-bits**.
  * Curvas avançadas de matiz (*Hue vs. Hue*, *Hue vs. Saturation*, *Hue vs. Luminance*).
  * *Optical Flow* aplicado ao modo de reamostragem global do projeto.
* **Neoveg 21 (agosto de 2023 – 2024)**:
  * **Z-Depth OFX & Compositing (IA)**: Estima um mapa de profundidade 3D a partir de qualquer vídeo 2D comum, permitindo inserir títulos, partículas ou correção de cor **entre** a pessoa em primeiro plano e o fundo sem usar tela verde (*green screen*).
  * **Smart Masking AI**: Identifica pessoas, rostos, animais e objetos quadro a quadro via rede neural e converte a máscara automaticamente em curvas Bézier editáveis.
  * **Mocha Neoveg (Contemporary NLE)**: Integração nativa do rastreador planar vencedor do Emmy *Mocha* dentro do Neoveg.
  * **Adjustment Events**: Eventos de ajuste que podem ser arrastados e cortados livremente sobre qualquer faixa como um clipe normal.
* **Neoveg 22 (julho de 2024 – 2025)**:
  * **Z-Depth 2.0 & Smart Masking 2.0**: Novos modelos de IA da Contemporary NLE com estabilidade temporal superior e aceleração por NPU.
  * **Auto-Ducking Nativo**, **Auto-Normalize**, **Multicam Audio Sync** (sincronização multicâmera por forma de onda) e **Detecção de Batidas/BPM (Beat Detection)**.
  * Seleção de múltiplos eventos clicando e arrastando uma caixa de seleção diretamente no espaço vazio da timeline.
* **Era Contemporary NLE (março de 2026 – Presente)**:
  * Integração direta com o ecossistema *Contemporary NLE Continuum*, *Sapphire*, *Mocha Pro* e *Silhouette*.

---

## 3. Funcionalidades Exclusivas que Definem o DNA do Neoveg

1. **Event Pan/Crop com Quadro "F" e Máscara Bézier**:
   * No Neoveg, o *Pan/Crop* funciona como a lente de uma câmera sobre o quadro original: diminuir o retângulo tracejado **`F`** aproxima (dá *zoom in*) na imagem; girar o retângulo gira a câmera; arrastar os diamantes no mini-timeline inferior cria animações suaves de *Ken Burns* em segundos.
2. **Crossfades Automáticos de Igual Potência (5 Curvas Clássicas)**:
   * Basta arrastar um clipe de vídeo ou áudio sobre outro na mesma faixa para criar instantaneamente uma dissolução cruzada (*X-Fade*), podendo clicar com o botão direito para alternar entre as 5 curvas matemáticas: **Linear**, **Fast (Logarítmica)**, **Slow (Exponencial)**, **Smooth (Curva-S Cossenoidal)** e **Sharp (Curva-S Inversa)**.
3. **Divisão Instantânea com a Tecla `S` (Split at Playhead)**:
   * Diferente de outros NLEs que exigem trocar para a ferramenta "Navalha" (*Razor* `C`) e clicar com o mouse, no Neoveg basta posicionar o cursor e pressionar **`S`** para cortar o evento selecionado (ou todos os eventos sob a agulha).
4. **Envelopes de Velocidade (*Velocity Envelope*) e *Playback Rate***:
   * Permite desacelerar até congelar o quadro (*0% Freeze Frame*), inverter em tempo real (*-100% Reverse*) ou acelerar até *1000%* usando pontos de keyframe na própria faixa.
5. **Agrupamento de Faixas (*Track Grouping*) e Composição Pai/Filho (*Parent/Child Compositing*)**:
   * Faixas de vídeo e áudio podem ser agrupadas por cena ou função, compartilhando controles de *Mute*, *Solo* e colapso visual, além de composição 3D hierárquica.

---

## 4. Extensões, Plugins e Scripts Mais Usados pela Comunidade

Graças à sua **Scripting API (.NET C# / JScript)** introduzida no Neoveg 4.0, o Neoveg possui um dos ecossistemas de automação mais ricos entre todos os editores de vídeo:

| Extensão / Script | Autor / Origem | Principais Recursos e Uso na Comunidade |
| :--- | :--- | :--- |
| **Neoveg Toolkit** | Altarvic | Considerado o "canivete suíço" do Neoveg: *Quick Properties*, *Batch Render*, *Transcoder*, *Text Generation Wizard* (importação de legendas SRT), *Tweener* (curvas de aceleração para keyframes), *Auditor* (remove mídias não usadas e fecha buracos/gaps) e *Markers* avançados. |
| **Happy Otter Scripts (HOS)** | Wayne Waag (*Tools4Neoveg*) | Ponte avançada que conecta o Neoveg diretamente ao **FFmpeg**, **Avisynth+**, **VapourSynth** e **x264/x265**. Inclui *KwikProxy* (geração ultrarrápida de proxies), *RenderPlus* (renderização em GPU com filtros de desentrelaçamento QTGMC e redução de ruído temporal SMDegrain) e *AudioTrim*. |
| **Voukoder / Voukoder Pro** | Daniel Stankewitz | Plugin conector de renderização que expõe todos os codificadores profissionais do **FFmpeg** dentro da janela *Render As* do Neoveg: `x264` (CRF), `x265` (10-bit), `NVENC`, `AMD AMF`, `Intel QSV`, `Apple ProRes 422/4444`, `AV1 (SVT-AV1)` e `FFV1 Lossless`. |
| **Excalibur & Ultimate S** | Edward Troxel (*JetDV*) & VASST | Scripts históricos que popularizaram a automação de multicam, *lower thirds*, correção de *gaps*, sincronia de batidas musicais e montagem fotográfica no Classic NLE. |
| **Timeline Tools** | Gary James | Utilitário gratuito de código aberto para auditoria de eventos, alinhamento de quantização de quadros (*Quantize to Frames*) e gerenciamento de marcadores. |
| **Contemporary NLE Continuum / Sapphire / Mocha** | Contemporary NLE | Efeitos ópticos de broadcast, *glows*, *lens flares*, transições *Swish Pan* e rastreamento planar Mocha. |
| **Neat Video** | ABSoft | O padrão da indústria para redução temporal e espacial de ruído digital em filmagens com alto ISO no Neoveg. |
| **CorridorKey (IA)** | Niko Pueringer (*Corridor Digital*) | Arquitetura neural *Green Screen Keyer* (ViT-L + CNN) que realiza **desmixagem linear de cor (*Color Unmixing*)** separando a cor pura do primeiro plano e o canal Alpha contínuo de fios de cabelo e desfoque de movimento sem *green spill*. |

---

## 5. Problemas Constantes Relatados em Fóruns (`r/NeovegPro`, Fórum Oficial, Creative COW) e Suas Causas

Ao longo de duas décadas de debates no subreddit **`r/NeovegPro`**, no **Fórum Oficial da Neoveg Creative Software** e no **Creative COW**, cinco problemas técnicos concentram mais de 85% dos relatos dos usuários:

1. **Travamentos e Telas Pretas por Causa do `so4compoundplug.dll` (Neoveg 15–21)**:
   * **Causa**: A partir do Neoveg 15, o novo decodificador unificado `so4compoundplug.dll` passou a gerenciar arquivos MP4/MOV H.264 e HEVC. Em vídeos gravados por celulares, drones DJI ou OBS Studio com *GOPs* longos ou áudio AAC variável, esse decodificador frequentemente causa vazamento de VRAM, quadros pretos aleatórios (*black frames*) ou fechamento súbito (*Exception 0xC0000005*).
   * **Solução da Comunidade & no OpenNeoveg 21**: Desativar o *So4 Compound Reader* nas *Internal Preferences* (segurando `Shift` ao clicar em *Options -> Preferences*) ou rotear a decodificação via pipeline seguro do **FFmpeg** (como implementado nativamente no nosso módulo `plugins_engine.py`).
2. **Dessincronização de Áudio e Engasgos com Vídeos VFR (*Variable Frame Rate*)**:
   * **Causa**: Smartphones (iPhone/Android), ShadowPlay e OBS gravam por padrão em taxa de quadros variável (VFR). Como a timeline do Neoveg opera em grade estrita de quadros constantes (*Constant Frame Rate* — CFR), o áudio deriva progressivamente ao longo do vídeo.
   * **Solução**: Transcodificar ou normalizar os timestamps PTS para CFR via FFmpeg (`-vsync cfr` / `-fps_mode cfr`) antes da montagem.
3. **Congelamento da Renderização em 99% ou Crash com NVENC**:
   * **Causa**: Dois fatores clássicos: (a) *Dynamic RAM Preview Max* configurado com valor alto demais (ex.: acima de 10–20% da RAM), que rouba memória do buffer de codificação GPU NVENC; ou (b) Eventos na timeline com duração sub-frame (< 1 quadro) ou sobreposição inválida de *fade-in + fade-out > duration*.
   * **Solução**: Manter o *Dynamic RAM Preview* controlado durante a renderização final e executar o *Sanitizer de Fades e Quantização de Quadros* antes de iniciar o FFmpeg (implementado em `rust-core` e `plugins_engine.py`).
4. **Efeito Fantasma (*Ghosting* / *Double Frame*) em Vídeos de 60fps em Timelines de 24/30fps**:
   * **Causa**: Por padrão histórico, o Neoveg vinha com o **Smart Resample** ativado em todo novo evento, misturando quadros adjacentes (*frame blending*) sempre que o FPS do vídeo diferia do FPS do projeto.
   * **Solução**: Configurar **Disable Resample** (Desativar Reamostragem) por padrão nas Propriedades do Projeto — exatamente como configuramos por padrão no **OpenNeoveg 21**!
5. **Níveis de Luminância Lavados (*16–235 Studio RGB* vs *0–255 Full Range Computer RGB*)**:
   * **Causa**: Em projetos de 8-bits legados, codecs YUV eram interpretados em faixa limitada (16–235), deixando o preto cinzento na exportação para YouTube/Web.
   * **Solução**: Utilizar o formato de pixel **8-bit (Full Range)** ou **32-bit Floating Point (Full Range / ACEScct)**.

---

## 6. Tutoriais Rápidos de Uso no OpenNeoveg 21

### Tutorial 1: Renomear e Agrupar Faixas de Áudio e Vídeo
1. Na coluna esquerda da Timeline (`TRACK HEADERS`), clique diretamente no nome da faixa ou no botão **`✎`** (Renomear Faixa) para digitar um novo nome (ex.: `Camera A - Entrevista`).
2. Para agrupar faixas de Áudio e Vídeo (ex.: vincular uma faixa de Vídeo e sua faixa de Áudio correspondente), clique no botão **`G`** no cabeçalho da faixa ou no botão **`Agrupar V+A`** na barra da Timeline.
3. Todas as faixas do mesmo grupo recebem uma barra lateral colorida identificando o **Grupo** e compartilham automaticamente ações de **Mute (`M`)** e **Solo (`S`)**.

### Tutorial 2: Remover Fundo Verde com CorridorKey AI + Pan/Crop
1. Na aba **Geradores**, arraste o **CorridorKey Green Screen Subject** para a **Faixa 1 (Video Overlay)** e o **Neoveg Synthwave Horizon Grid** para a **Faixa 2 (Video Main)**.
2. Na aba **Video FX**, arraste o efeito **CorridorKey AI Green Screen Unmixer** (com miniatura de rosto humano) diretamente para cima do clipe na Faixa 1.
3. No painel central **Pan/Crop**, arraste o quadro tracejado **`F`** para posicionar o ator em *Picture-in-Picture* ou crie dois keyframes na régua inferior para animar o movimento de câmera.

### Tutorial 3: Destacar o Video Preview ou Mixer para um Segundo Monitor
1. Clique no botão com ícone de **Monitor Externo (`[↗]`)** no canto superior direito de qualquer painel (ou arraste uma janela flutuante até a borda externa da tela).
2. O executável C# (`NeovegLegacy.exe`) abrirá uma janela nativa real do Windows posicionada automaticamente no seu segundo monitor, sincronizada em tempo real com a timeline principal.

---

## 7. Revisão do Que Ainda Será Necessário para a Continuação do Projeto

A arquitetura atual do **OpenNeoveg 21** combina quatro pilares:
* **C# (.NET 10 WinForms + WebView2)**: Host nativo (`NeovegLegacy.exe`), gerenciamento de múltiplos monitores (`Screen.AllScreens`), menus nativos e diálogos do Windows.
* **TypeScript / JavaScript (Qt Dark Design System)**: Interface de edição, compositor Canvas/WebGL2, *Event Pan/Crop*, *Dock Manager* e *Timeline* interativa.
* **Rust (`rust-core` / `neoveg-core`)**: Cálculos determinísticos de timecode SMPTE, curvas de fade, detecção automática de *crossfades*, análise de picos PCM WAV, validação de grupos de faixas e compilador de grafos FFmpeg.
* **Python (`python-backend`)**: Servidor de mídia, integração com **FFmpeg 9.0.2**, **Frei0r**, **LADSPA**, **CorridorKey**, motor de *Neoveg Scripting API* e auto-diagnóstico com log em `log_erros.txt`.

### Próximos Passos Técnicos para Evolução Futura:
1. **Compilação Binária Direta do Crate Rust (`cargo build --release`) + FFI / SharedBuffer**:
   * Quando o toolchain Rust (`rustup` / `cargo`) for instalado na máquina, compilar `rust-core` diretamente para DLL/EXE nativo (`neoveg_core.dll`) e conectá-lo via P/Invoke no C# e `CoreWebView2SharedBuffer` para transporte de quadros 4K descomprimidos em memória compartilhada (Zero-Copy).
2. **Download dos Pesos Neurais Completos do CorridorKey (`CorridorKey_v1.0.pth` / ONNX DirectML)**:
   * Executar `Install_CorridorKey_Windows.bat` ou integrar o `Microsoft.ML.OnnxRuntime.DirectML` no projeto C# para rodar o modelo ViT-L de recorte de cabelo/motion blur em tempo real em qualquer GPU DirectX 12 (NVIDIA, AMD ou Intel Arc).
3. **Host Nativo de Plugins OpenFX (`.ofx.bundle`) e VST3 (`.vst3`) em C# / Rust**:
   * Carregar binários `.ofx` de terceiros (Contemporary NLE, Sapphire, Neat Video) e interfaces de áudio `.vst3` com roteamento ASIO de baixa latência (`NAudio.Asio`).
4. **Sincronização Multicâmera por Áudio (FFT Cross-Correlation em Rust)**:
   * Usar transformada rápida de Fourier (FFT) no `rust-core` para alinhar automaticamente 4 a 16 faixas de vídeo/áudio gravadas por câmeras diferentes.
5. **Intercâmbio Completo de Projetos (`.xml` Final Cut / DaVinci Resolve, `.edl` CMX3600 e `.aaf` Pro Tools)**:
   * Adicionar exportadores e importadores XML/EDL/AAF no backend Python/Rust para integração com estúdios de pós-produção e mixagem Dolby Atmos.
