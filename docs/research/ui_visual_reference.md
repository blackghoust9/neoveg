# Referência visual da interface (manuais v13 e v18) e mapeamento para o egui

Estudo das capturas de tela dos manuais do software de referência, feito para guiar `neoveg_ui`.
As imagens foram renderizadas por `scripts/extract_pdf_ui.py` (PyMuPDF, 150 dpi) para uma pasta
**fora do repositório** (`D:\ambiente_neoveg\pdf_ui_references` quando existe a unidade D:, senão
`%LOCALAPPDATA%\Neoveg\pdf_ui_references`), porque os manuais são protegidos por direitos autorais:
este documento guarda **descrições, proporções e cores aproximadas**, nunca as imagens.

**Método e limites.** O script ordena as páginas por palavras-chave de interface e por imagens grandes
embutidas e renderiza as melhores de cada manual (36 por PDF, 375 imagens no total; `--all` renderiza
tudo). As medidas abaixo vêm de capturas reduzidas dentro de páginas de 150 dpi: são **proporções
aproximadas (±10 %)**, boas para orientar o layout, não pixels exatos. As cores foram amostradas nas
imagens (cada canal arredondado para múltiplos de 8).

Páginas estudadas: v18 p. 22 (linha do tempo anotada) e p. 301 (prévia com texto); v13 p. 33 (Master Bus
e Video Preview), p. 229 (cabeçalhos de faixa e grupos), p. 294 (controlador de keyframes) e p. 345
(Mixing Console).

## 1. Hierarquia geral da janela

```
┌ barra de menus ───────────────────────────────────────────────────────────────┐
│ janelas acopladas (esquerda/centro/direita)        │ Master Bus / Video Preview │
│  Explorer | Trimmer | Project Media | ...   (abas) │  (abas no rodapé)          │
├────────────────────────────────────────────────────┴────────────────────────────┤
│ lista de faixas (cabeçalhos) │ régua de tempo                                   │
│                              │ eventos nas faixas                      │ rolagem│
│                              │                                         │ vertical
├──────────────────────────────┴─────────────────────────────────────────┴───────┤
│ barra de transporte | barra de edição | rolagem horizontal | caixas de duração | │
│ controle de zoom de tempo (+ / −) | controle de altura de faixa (+ / −)          │
└─────────────────────────────────────────────────────────────────────────────────┘
```

- A área de janelas acopladas fica **acima** da linha do tempo por padrão; cada janela tem barra de
  título com botões de maximizar e fechar, e as abas ficam **no rodapé** da janela acoplada
  ("Video Preview | Trimmer" na p. 301). Isso confirma o padrão `footer_tabs` do AGENTS.md 4.1.
- **Linha do tempo (v18 p. 22):** a régua de tempo fica em cima; a barra de transporte e a barra de edição
  ficam **abaixo** das faixas, à esquerda; a barra de rolagem horizontal ocupa o centro da mesma
  linha; à direita ficam as caixas de duração e o controle de zoom de tempo; no canto inferior
  direito, o controle de altura de faixa (`+` / `−`) e a ferramenta de zoom; a rolagem vertical fica à
  direita, e o duplo clique nela afasta o zoom até caberem todas as faixas.
- A faixa vermelha fina no alto da régua é a **régua de status de renderização** (já existe em Neoveg).

## 2. Cores

| Elemento | v13 (tema prata) | v18 (tema escuro) | Neoveg hoje |
| :--- | :--- | :--- | :--- |
| Fundo das janelas | `#888888` cinza médio, bordas `#707070`–`#989898` | `#404040` (barras de ferramentas da prévia) | `Classic Silver` / `Modern Carbon Dark` (`neoveg_common::theme`) |
| Fundo de faixa vazia e área de prévia | — | `#202020` | `timeline_bg` / `lane` |
| Linhas e separadores | `#787878` | `#303030` | `grid_minor` / `grid_major` |
| Forma de onda de áudio | — | `#A8A0C0` (miolo claro) com `#584080` (envelope) — **lilás/roxo** | `audio_clip` (azul `#265278` no Carbon) |
| Evento de vídeo | miniaturas | miniaturas claras (`#D8D8D8` em quadros sem imagem) | `video_clip` (roxo) |
| Botão de acento / seleção | azul `#8098B8` (faixa de canal selecionada) | azul | `accent` |
| Medidor (barra) | verde acinzentado `#708880` | — | verde `#2ECC71` |
| Marca de nível alto | vermelho | `#783040` (marcador/realce vinho) | vermelho `#E74C3C` |

Observação: o roxo da forma de onda em v18 é a convenção do software de referência; o Neoveg usa
azul para áudio e roxo para vídeo, escolha própria que pode virar tema "Classic" se se quiser paridade.

## 3. Cabeçalho de faixa (v13 p. 229)

- Altura do cabeçalho padrão ≈ **84 px** em duas linhas: (1) caixa numérica colorida com o número da
  faixa (≈ 28 × 16 px, cor da faixa, número em branco) + campo de nome (≈ 110 px) + fileira de botões
  quadrados de ≈ 14–16 px; (2) rótulo **"Level: 100.0 %"** + controle deslizante horizontal (≈ 100 px)
  com cursor largo em duas barras.
- Fileira de botões de faixa, da esquerda para a direita: ícone de composição, movimento da faixa
  (Pan/Crop), FX de faixa, engrenagem (automação, com seta de menu), **proibido** (mudo) e **exclamação**
  (solo/atenção); botões planos com ícone monocromático, sem texto.
- À direita da área de cabeçalho há uma miniatura do primeiro quadro (≈ 56 × 40 px) em faixas de vídeo.
- **Grupo de faixas:** cabeçalho de grupo de ≈ 20 px com triângulo de recolher/expandir à esquerda,
  nome do grupo em campo escuro e dois botões à direita; as faixas do grupo ficam recuadas por uma
  **"grouping lane"** (barra vertical de ≈ 12 px com o ícone de arrastar).
- Mapeamento: Neoveg tem `HEADER_WIDTH = 150` e `TRACK_HEIGHT = 64`; a referência é mais larga e alta
  (≈ 200 × 84). Sugestão: manter 64 px como altura mínima, oferecer "altura padrão / mínima / grande"
  (atalhos `` ` `` e `Ctrl+Shift+↑/↓`) e acrescentar a segunda linha (nível e botões M/S/Ø, que já
  existem) quando a altura passar de 64 px.

## 4. Master Bus (v13 p. 33)

- Janela estreita de ≈ **180 px de largura × 190 px de altura** com título, linha de informação
  "48,000 Hz, 16-bit" (esquerda) e quatro botões de ícone (direita).
- Duas tiras lado a lado: **Preview** (cinza) e **Master** (com caixa de nome e botões de FX/automação
  abaixo). Cada tira = **fader vertical + par de medidores estéreo**.
- **Fader:** trilho de ≈ 2 px, comprimento ≈ 120 px; cursor retangular largo (≈ 24 × 10 px) com
  duas barras horizontais em relevo ("mola dupla"); a tira Master tem um cadeado embaixo para ligar
  os canais.
- **Medidores:** duas colunas de ≈ 10 px de largura por ≈ 120 px de altura (proporção ≈ 12 : 1) com
  **escala central** de rótulos `6, 12, 18, 24, 30, 36, 42, 48, 54` em dB negativos (passo de 6 dB,
  rótulos de 8–9 px, linhas de marcação tracejadas), "−Inf." acima de cada coluna e a leitura
  numérica de pico (`0.0`) abaixo.
- Mapeamento: o painel Master do Neoveg usa passos 0/−6/−12/−24/−48/−∞ e leituras `L`/`R` embaixo
  (AGENTS.md 4.4). A referência sugere (a) escala uniforme de 6 dB, (b) leitura de pico acima e
  abaixo de cada barra, (c) fader ao lado das barras e não abaixo. Largura mínima útil: ≈ 150 px.

## 5. Video Preview (v13 p. 33; v18 p. 301)

- Barra superior de ≈ 22 px: ícones à esquerda (configurações, copiar, FX), rótulo "Preview (Auto)"
  com seta (seletor de qualidade), grade/overlays, copiar e salvar quadro.
- Quadro de vídeo centralizado sobre fundo `#202020`; menu de contexto da prévia: fundo padrão /
  preto / branco, simular proporção do dispositivo, escalar para a janela, ajustar tamanho e
  qualidade, mostrar barra de ferramentas / de status / de transporte.
- **Transporte** (≈ 16 px) logo abaixo do quadro, centralizado: gravar, laço, reproduzir do início,
  reproduzir, pausar, parar, quadro anterior / seguinte, ir ao início / fim.
- **Barra de status** de duas colunas em fonte de ≈ 9 px: esquerda `Project: 1920x1080x32; 29,970i` e
  `Preview: 480x270x32; 29,970p` (em vermelho quando difere do projeto); direita `Frame: 0` e
  `Display: 597x336x32`. Corresponde à faixa de status de AGENTS.md 4.2.
- Alças de texto (p. 301): caixa vermelha fina com 4 alças quadradas de ≈ 6 px nos cantos e um
  quadradinho vazio central (ponto de âncora): referência para o gizmo `F`.

## 6. Mixing Console (v13 p. 345)

- Estrutura: **lista de canais** à esquerda (≈ 100 px, caixas de seleção + nome colorido), abaixo dela
  os botões de visualização ("Show All", "Audio Tracks", "Audio Busses", "Input Busses",
  "Assignable FX", "Master Bus", "Preview Bus", I/O, VU Meters, Meters, Faders), **barra de
  ferramentas** no alto (Inserir faixa de áudio / FX atribuível / barramento / barramento de entrada) e as
  **tiras de canal** lado a lado.
- Cada tira (≈ 53 px de largura em modo normal, ≈ 40 estreita, ≈ 70 larga) de cima para baixo:
  cabeçalho colorido com número e tipo ("Audio"), **slots de FX de inserção** (campos de ≈ 14 px de
  altura com o nome do efeito), **envios** (rótulo "Bus A", fader pequeno horizontal, "Pre/Post" e
  valor em dB), campo de **entrada/saída**, modo de automação ("Touch"), **pan** (barra horizontal
  com "Center"), **fader + medidor** (≈ 130 px de altura útil) com botões de mudo/solo/gravar ao
  lado, e o **nome** da faixa no rodapé; tira do mestre à direita.
- Mapeamento: um painel "Mixer" do Neoveg pode reutilizar `mixer_panel.rs` (já tem Mudo/Solo/Fase,
  volume e pan) em tiras verticais com `PrecisionSlider` vertical, medidor por faixa e escala de 6 dB.

## 7. Controlador de keyframes (v13 p. 294)

- Faixa de ≈ 100 px de altura sob a prévia/efeitos: rótulos de parâmetro à esquerda (≈ 120 px) e uma
  linha por parâmetro com **losangos** de ≈ 9 px (cinza = interpolação linear, verde, azul, vermelho = tipos de curva;
  vermelho preenchido = keyframe selecionado) e cursor vertical vermelho.
- Barra de botões: inserir / excluir keyframe, primeiro / anterior / próximo / último, campo de tempo
  `00:00:08;28`, abas **Lanes | Curves**.
- Mapeamento: já há envelopes de faixa (`neoveg_engine::envelope`); para keyframes de efeitos
  reutilizar o desenho de losango do `envelope_canvas.rs`.

## 8. Regras de estilo extraídas

1. Botões **planos e quadrados**, ícone monocromático sobre fundo transparente, realce só no `hover`;
   o texto aparece em dica (confirma AGENTS.md 4.1).
2. Tipografia pequena: rótulos de 9–11 px nas janelas, 11–12 px nos campos; números de tempo em
   fonte monoespaçada.
3. Controles **densos**: espaçamento de 2–4 px, alturas de linha de 14–22 px.
4. Cada faixa tem **cor própria** (caixa numérica e tira do mixer), usada também no evento.
5. Medidores e faders sempre com **escala de 6 dB** e leitura numérica de pico.
6. Janelas com **maximizar** e **fechar** no título; abas embaixo; divisórias arrastáveis.

## 9. Ações sugeridas para `neoveg_ui`

| Prioridade | Ação | Onde |
| :--- | :--- | :--- |
| Alta | Escala de 6 dB e leitura de pico nos medidores do Master | `master_bus_content` |
| Alta | Segunda linha no cabeçalho de faixa (nível + botões) acima de 64 px, altura de faixa ajustável (`` ` ``, `Ctrl+Shift+↑/↓`) | `timeline_ui`, comandos `view.track_height_*` |
| Média | Barra de status de duas colunas na prévia (Project / Preview, Frame / Display) | `preview_ui` |
| Média | Rolagem horizontal e controle de zoom de tempo na linha do rodapé, controle de altura de faixa no canto | `timeline_ui` |
| Média | Mixer em tiras verticais (fader, medidor, pan, mudo/solo) | novo painel sobre `mixer_panel.rs` |
| Baixa | Grupo de faixas com cabeçalho recolhível | `neoveg_engine` + `timeline_ui` |
| Baixa | Tema "Classic" com forma de onda lilás `#A8A0C0` / `#584080` | `neoveg_common::theme` |
