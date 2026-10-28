---
tags: [neoveg, referencia, janelas, paineis, layouts]
---
# Janelas, painéis e layouts do editor de referência

Hub: [[segundo_cerebro]]. Planejamento das janelas do Neoveg: [[especificacao_janelas]]. Código: [[neoveg_ui]].

## Painéis acopláveis (menu Exibir → Janela)
| Painel da referência | O que faz | Neoveg |
| :--- | :--- | :--- |
| Explorador (novo e antigo) | navega pelo disco e arrasta mídia | `feito` (Explorer) |
| Trimmer | prepara trechos da mídia antes de pôr na timeline | `feito` |
| Master Bus | fader e medidores do barramento principal | `feito` |
| Video Preview | prévia com qualidade e overlays | `feito` |
| Mídia do projeto | pool de mídias | `feito` |
| Notas do projeto | texto livre salvo no projeto | `feito` |
| Transições, Efeitos de vídeo, Geradores | bibliotecas em abas | `parcial`: abas existem, a GPU é `previsto` ([[catalogo_efeitos_video]]) |
| Compositores | modos de mistura entre faixas de vídeo | `previsto` |
| Gerenciador de plug-ins | habilita e oculta plug-ins | `previsto` (junto do VST3, [[catalogo_efeitos_audio]]) |
| Escopos de vídeo | forma de onda, vectorscópio, histograma, parada | `feito` |
| Panner surround | posição do som em canais múltiplos | `sem plano` |
| Detalhes de edição | tabela de eventos, marcadores e regiões | `feito` |
| Explorador de dispositivos e de disco profissional | câmeras e mídia removível | `sem plano` |
| Medidores de loudness | momentâneo, curto prazo e integrado | `feito` |
| Hub online | biblioteca de conteúdo na nuvem | `sem plano` |
| Extensões | painéis de terceiros | `feito` (gerenciador de extensões) |

## Mixing Console e barras
- Console de mixagem acoplável, com fader de prévia, régua de faixas de barramento de áudio e de vídeo. Neoveg: `parcial` (mixer com efeitos por faixa; barramentos e sends são `previsto`).
- Opções de exibição da timeline: botões do evento, cabeçalhos, comprimento do evento, tamanho dos fades, forma de onda, informação do take ativo, marcadores de mídia e quadros de borda de edição. Neoveg: `parcial`; veja [[regras_de_timeline]].
- Barra de status com taxa de reprodução, taxa de quadros, taxa de bits, tempo de gravação e aviso de proxies sendo gerados. Neoveg: `feito` (status com dimensões, bits e FPS).

## Layouts de janela
- A versão antiga entrega **3** layouts padrão e a recente **12**, um por tarefa: mixagem de áudio, correção de cor, edição expandida, janela de origem, e cinco "modos" (adicionar e organizar mídia, adicionar texto e títulos, adicionar elementos gráficos, adicionar transições, adicionar efeitos de vídeo) mais dois de fluxo específico.
- Comandos: organizar, salvar, salvar como, recarregar e padrão; dez posições numeradas de layout salvo (0 a 9) com atalho.
- Neoveg: `parcial`. Há layouts 1 a 9 e o padrão, só na sessão; `previsto`: persistir no disco, organizar, e incluir janelas destacadas (§32 do AGENTS.md).

## Diálogos de propriedades
- O **evento de áudio** tem um único diálogo com: nome do take ativo, trava, repetição, inversão de fase, normalização (automática e recalculada), ganho, método de esticar o tempo, mudança de altura (semitons), travar ao esticar e preservar formantes.
- O **diálogo de mídia** reúne: nome da fita, timecode do arquivo ou personalizado, taxa de quadros substituta, ordem dos campos, proporção do pixel, canal alfa e cor de fundo.
- Neoveg: `parcial`; interruptores de evento (mudo, trava, fase, loop, normalizar) e detalhes existem; esticar com formantes é `previsto`.

## Próximos passos
1. Persistir layouts nomeados e criar os layouts por tarefa (mixagem, cor, texto).
2. Painel de compositores junto do modelo de composição.
3. Diálogo de propriedades de mídia com timecode e proporção do pixel.
