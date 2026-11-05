# Guia de semântica do VEGAS Pro para o Neoveg

Este guia diz **como o editor de referência (VEGAS Pro) se comporta** nas áreas que o Neoveg copia, e
em que ponto o Neoveg está igual, diferente ou ainda sem o recurso. Está escrito em palavras próprias a
partir dos manuais que o projeto guarda (`manuais/markdown/`: o manual v21, o v18 e a lista de atalhos
do v18); os números de página citados são os dos PDFs. Nenhuma captura de tela, texto de menu ou arquivo
do produto entra aqui (AGENTS.md §28). O estado do Neoveg foi conferido no código em 05/11/2026.

Legenda: **igual** (mesmo atalho e mesmo efeito), **diferente** (existe, mas muda tecla ou efeito),
**falta** (o Neoveg não tem), **a conferir** (a regra existe no código, mas não foi testada com o mouse).

## 0. Regra de integração (vale para todo trabalho novo)

> Toda funcionalidade que chegar de outro motor (FilmCraft, Whisper, OpenTimelineIO, VST3, OpenFX,
> CLAP, o que vier) é **envelopada na interface e na lógica do editor de referência**: entra pelos
> menus, painéis, atalhos e janelas que ele já usa para aquele tipo de coisa e **não muda** os
> atalhos, os layouts nem os menus que o Neoveg copia dele.

Na prática:

1. Antes de ligar um recurso, procure no manual onde o editor de referência o põe (menu, painel,
   atalho) e ponha no mesmo lugar. Um motor novo é um *meio* de fazer o que o menu já promete, não uma
   janela nova com regras próprias.
2. Uma tecla do manual nunca é tomada para outro comando. Um comando que o manual não liga a tecla
   nenhuma só ganha uma tecla que o manual não use (as teclas próprias do Neoveg ficam em
   `neoveg_common::keymap`, com o motivo no `AGENTS.md` §4.5).
3. O que o editor de referência não tem (gerenciador de plug-ins isolados, host OpenFX em processo
   próprio, Whisper) entra como **menu ou janela dentro do menu que corresponde** (por exemplo,
   *Ferramentas*), sem pôr atalho que o manual já use.
4. O teste `the_reference_default_bindings_are_kept` (`neoveg_common::keymap`) fixa as teclas desta
   seção: um atalho que mude por engano faz o teste falhar.

## 1. Edição de eventos

| Regra do editor de referência | Neoveg |
| :--- | :--- |
| **Crossfade automático.** Quando um evento é arrastado ou colocado de modo que sobreponha parcialmente outro da mesma faixa, nasce um crossfade na sobreposição (manual v21: *Automatic crossfades*). O botão fica na barra de edição; atalho `Ctrl+Shift+X`. | **igual** (`neoveg_engine::crossfade`, ligado por padrão; `Ctrl+Shift+X`). |
| **Alças de fade** nos cantos superior esquerdo e direito do evento; arrastar muda a duração; o formato da curva vem do menu da alça (v21 p.242). | **igual** (alças desenhadas no corte e na ponta, `FadeCurve` por lado, menu de contexto). |
| **Alça de ganho no centro do topo**: opacidade do evento de vídeo, volume do evento de áudio (v21 p.242). | **igual** (`event_level_drag`; o duplo clique restaura). |
| **Dividir sob o cursor** com `S`: os eventos selecionados, ou todos os que o cursor cruza quando nada está selecionado; com seleção de tempo, nas duas bordas. `Alt+S` é a divisão inteligente. | **igual**. |
| **Aparar.** `Ctrl+T` apara o evento à seleção de tempo; `Alt+[` e `Alt+]` aparam início e fim até o cursor. | **igual**. |
| **Aparo por teclado**: `[` e `]` ou `7` e `9` do teclado numérico escolhem a borda; `1`/`3` movem um quadro, `4`/`6` um pixel; `Ctrl` estica, `Alt` desliza a mídia, `Shift` aparo L/J, `Ctrl+Alt` desliza o corte; `5` sai. | **igual** nas teclas da fileira de números e `[` `]`; as do teclado numérico em si: **a conferir**. |
| **Mouse nas bordas e no corpo.** Arrastar a borda apara; `Ctrl`+borda estica (compressão do tempo); `Ctrl+Alt`+borda apara o evento vizinho junto (*trim adjacent*); `Alt`+arrastar dentro do evento desliza a mídia (*slip*); `Shift` ignora o agrupamento e inverte o alinhamento (liga se estava desligado, desliga se estava ligado). | **igual** em esticar, vizinho e agrupamento (`DragMode::Stretch*`, `Roll*`, `drag_ignores_group`); **diferente** no `Shift`: inverte o alinhamento só ao mover o evento (`effective_snap`), não nos aparos, e o `Alt` desliga o alinhamento; **a conferir** o `Alt`+arrastar sem a ferramenta Slip. |
| **Interruptores por evento**: *Loop*, *Normalize*, *Lock*, *Invert phase* (também *Mute*, *Reverse*, *Maintain aspect*, *Resample*, *Reduce interlace flicker*). O manual não dá atalho a eles. | **igual** nos efeitos; teclas próprias `Ctrl+Alt+O/N/L/I/U` (nenhuma delas é usada pelo manual). |
| **Ferramentas de edição.** `D` próxima, `Shift+D` anterior, `Ctrl+D` normal (Normal, Slip, Slide). | **igual**. |
| **Agrupar.** `G` agrupa, `U` tira do grupo, `Ctrl+U` dissolve, `Shift+G` seleciona o grupo, `Ctrl+Shift+U` ignora o agrupamento. | **igual**. |
| **Ripple.** `F`, `Ctrl+F`, `Ctrl+Shift+F` escolhem o alcance; `Ctrl+L` liga o ripple automático. | **igual**. |
| **Colar.** `Ctrl+V` cola; `Ctrl+Shift+V` cola inserindo; `Ctrl+B` repete a colagem. | **igual**. |
| **Renomear o evento selecionado:** `F2`. | **falta** (o nome se edita no cabeçalho do evento). |
| **Mover eventos com o teclado numérico** (`1`/`3` um quadro, `4`/`6` um pixel, `8`/`2` uma faixa acima ou abaixo). | **falta** fora do modo de aparo por teclado. |
| **Transformar corte em transição** e o contrário (`/`, `*`, `-` do teclado numérico, com `Ctrl`). | **falta**. |
| **Abrir o sub-projeto** (`Alt+N`) e o projeto pai (`Alt+P`). | **falta** (o Neoveg cria o sub-projeto com `Alt+C`, que também é do manual). |

## 2. Navegação, régua e seleção

| Regra | Neoveg |
| :--- | :--- |
| **Régua e tempo em SMPTE** (com quadros descartados para 29,97 e 59,94); o cursor mostra o quadro em que está. | **igual** (`neoveg_common::timecode`; o traço de amostragem do quadro está na playhead). |
| **Alinhamento (snap)**: `F8` liga e desliga; `Ctrl+F8` à grade, `Shift+F8` a marcadores, `Ctrl+Shift+F8` a todos os eventos; `Alt+F8` quantiza as edições aos quadros (*Quantize to Frames*). `Shift` ao arrastar inverte o alinhamento. | **igual**. |
| **Pontos de entrada e saída**: `I` e `O` marcam o início e o fim da seleção de tempo no cursor. | **igual**. |
| **Marcadores e regiões**: `M` insere marcador, `R` região, `C` marcador de comando; as teclas `0` a `9` vão ao marcador ou região de mesmo número; `Ctrl+←` e `Ctrl+→` vão ao marcador ou borda de região anterior ou seguinte. | **igual**. |
| **Reprodução e varredura (scrub)**: `L` avança (de novo, acelera: 1x, 2x, 4x…), `J` volta, `K` pausa; `Espaço` toca ou pausa; `Q` repete em laço; `Shift+Q` seleciona a região de laço. | **igual**. |
| **Mover o cursor**: `Home` e `End` ao início e ao fim da seleção ou da vista; `Ctrl+Home`/`Ctrl+End` e `W` ao início e ao fim do projeto; `\` centraliza a vista; `Ctrl+Alt+←/→` vão às bordas de evento e de fade; `Backspace` restaura as últimas cinco seleções de tempo. | **igual**. |
| **Setas.** No manual, `←` e `→` movem **um pixel** (um quadro com a quantização ligada) e `Alt+←/→` um quadro. | **diferente, aceito**: o Neoveg move um quadro com `←` e `→`, e `Alt+←/→` também (AGENTS.md §4.5). |
| **Mover em marcas da grade**: `Page Up` e `Page Down`. | **falta** (as teclas servem à rolagem de faixas com `Ctrl+Alt`). |
| **Rolagem fina**: `F3` e `F9` (jog) e `Alt+↓` (cursor de edição para o de reprodução). | **falta**. |
| **Zoom**: `↑` e `↓` em passos pequenos, `Ctrl+↑/↓` em passos grandes, `Alt+↑` até um quadro por miniatura. | **igual**. |
| **Altura das faixas e da forma de onda**: `` ` `` minimiza ou restaura, ``Ctrl+` `` volta à altura padrão, `Ctrl+Shift+↑/↓` mudam todas, `Shift+↑/↓` a forma de onda. | **falta**. |
| **Esconder janelas**: `F11`, `Ctrl+F11`, `Shift+F11` (área de janelas, linha do tempo, lista de faixas); `Tab` alterna o foco entre a lista de faixas e a linha do tempo. | **igual**. |
| **Ciclo entre janelas**: `F6` e `Ctrl+Tab`, `Shift+F6` e `Ctrl+Shift+Tab`. | **falta**. |
| **Seleção**: `Ctrl+A`, `Ctrl+Shift+A`; `Shift`+clique seleciona um intervalo, `Ctrl`+clique soma. | **igual**. |

## 3. Mixagem e envelopes

| Regra | Neoveg |
| :--- | :--- |
| **Envelopes de faixa**: `V` insere ou mostra o de volume (nível, em vídeo), `P` o de pan, `Shift+V` e `Shift+P` os removem; `Ctrl+Shift+E` mostra ou esconde os envelopes; `E` e `Shift+E` percorrem os envelopes de efeito. | **igual**, menos `E` e `Shift+E` (**falta**). |
| **Envelope de evento**: volume, pan e, no vídeo, velocidade (*Velocity*); `Ctrl+Alt+Shift+K` trava os envelopes aos eventos (tecla própria). | **igual**. |
| **Mudo e solo**: `Z` e `X`; com `Shift`, exclusivo (as outras faixas saem do grupo). | **igual**. `Ctrl+Alt+Z/X` (grupo de faixas): **falta** (não há grupos de faixas). |
| **Barramentos**: até **26** barramentos de áudio mais o Master (v21), cada um com cadeia de efeitos, envios e roteamento; o *Mixing Console* (`Ctrl+Alt+5`) mostra faixas, barramentos e efeitos atribuíveis. | **igual** no console, nos envios, nos inserts e no limite (`MAX_BUSES = 26`); os barramentos **não** aparecem como faixas da linha do tempo (`B` e `Ctrl+Shift+B` do manual: **falta**). |
| **Efeito de faixa e de evento.** Faixas e eventos têm cadeias próprias; a cadeia de vídeo tem até **32** efeitos (v21 p.6469); a de projeto é a última antes da correção de cor. | **igual** (`neoveg_engine::video_effects`, 32 por cadeia, quatro cadeias: mídia, evento, faixa e projeto). |
| **Ordem do sinal de vídeo**: decodificar, chave de croma, efeitos de mídia, de evento, opacidade e crossfade, efeitos de faixa, composição, efeitos de projeto, correção e LUT. | **igual** (AGENTS.md §4.3). |

## 4. Movimento e efeitos

| Regra | Neoveg |
| :--- | :--- |
| **Pan/Crop do evento**: uma caixa de seleção sobre a fonte (centro, tamanho e rotação), animável por quadros-chave; abre numa janela própria pelo botão do cabeçalho do evento; tecla `F` no gizmo. | **igual** (`neoveg_engine::pan_crop`, `pan_crop_window`). |
| **Track Motion** (2D) e **3D Track Motion**: posição, escala, rotação e ponto de origem da camada da faixa; atalhos da janela: `A` trava a proporção, `X`, `Y`, `Z` impedem o movimento, `Shift+X/Y/Z` impedem a escala, `1` a `6` mudam o layout 3D. | **igual** no 2D; o 3D e os atalhos da janela: **falta**. |
| **Efeitos de vídeo**: lista por categoria na janela *Video FX* (`Alt+8`), arraste ou duplo clique aplica; predefinições por efeito; as chaves de parâmetros sobre o tempo. | **igual** nas listas e predefinições; as curvas de cor e os 36 efeitos estão no README. |
| **Transições**: janela *Transitions* (`Alt+7`), sobre o fade de entrada ou saída e sobre a sobreposição. | **igual** (`transition_in`, `transition_out`, `transition_progress`). |

## 5. Interface

| Regra | Neoveg |
| :--- | :--- |
| **Layout compacto e denso**: cabeçalhos de faixa com botões pequenos (mudo, solo, armar, efeitos, movimento), eventos com cabeçalho próprio (nome e botões de *Generated Media*, *Pan/Crop*, *FX* e menu), tema escuro. Tokens do tema escuro compacto pedidos para o Neoveg: fundo `#181818` e destaque `#00A2ED`. | **igual** como tema à parte: o tema `Compact Dark` (`neoveg_common::theme`, Editar > Preferências > Temas) usa esses dois tokens; o padrão continua `Modern Carbon Dark`. |
| **Docking com abas no rodapé** do grupo de janelas, abas planas e compactas, menu de contexto do grupo (fechar, desencaixar, mover para, maximizar), layouts numerados `Alt+D` e depois `0` a `9`. | **igual** (`footer_tabs`, `dock_footer`, `layouts`). |
| **Menus**: Arquivo, Editar, Exibir, Inserir, Ferramentas, Opções, Ajuda, com as janelas do manual (`Alt+0` a `Alt+9`, `Ctrl+Alt+1` a `Ctrl+Alt+8`). | **diferente**: o Neoveg usa *Arquivo, Editar, Faixas, Ferramentas, Exibir e Ajuda*, e liga só parte das janelas (ver abaixo). |
| **Janelas e as teclas do manual.** `Alt+0` foco na linha do tempo, `Alt+2` Trimmer, `Alt+3` Master Bus, `Alt+4` Video Preview, `Alt+5` Project Media, `Alt+7` Transitions, `Alt+8` Video FX, `Ctrl+Alt+2` Video Scopes, `Ctrl+Alt+5` Mixing Console, `Ctrl+Alt+7` Loudness. | **igual**. |
| Sem tecla no Neoveg: `Alt+1` Explorer, `Alt+6` Project Notes, `Alt+9` Media Generators, `Ctrl+Alt+1` Plug-In Manager, `Ctrl+Alt+3` Surround Panner, `Ctrl+Alt+4` Edit Details, `Ctrl+Alt+6` Device Explorer, `Ctrl+Alt+8` Dashboard. | **falta** a tecla (as janelas de notas, detalhes, geradores e gerenciador de plug-ins existem; só estão no menu). |

## 6. Como conferir

* **Atalhos:** o teste `the_reference_default_bindings_are_kept` guarda as teclas desta seção, e a
  lista de teclas do manual v18 (`manuais/markdown/VEGAS_Pro_18_EN_Keyboard_shortcuts.md`) é a fonte.
  Para refazer o cruzamento completo, extraia a tabela do PDF com `pdftotext -layout` (a conversão
  para Markdown separa a coluna dos comandos da das teclas; no `-layout` as duas ficam na mesma
  linha) e compare com `neoveg_common::keymap`. Em 05/11/2026 o cruzamento deu 136 teclas do manual
  com comando no Neoveg (todas com o mesmo efeito, fora a divergência das setas) e as demais sem
  ligação, listadas nas seções acima como **falta**.
* **Comportamento do mouse:** os arrastes da linha do tempo se conferem na janela real; o código está
  em `neoveg_ui::app` (`hit_event`, `DragMode`, `effective_snap`).
* Toda divergência nova entra aqui, com o motivo, antes de ir para o código.
