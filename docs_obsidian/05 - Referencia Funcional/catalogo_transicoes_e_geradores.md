---
tags: [neoveg, referencia, transicoes, geradores]
---
# Catálogo de transições e geradores

Hub: [[segundo_cerebro]]. Implementação: [[neoveg_video]], [[neoveg_engine]].

## Transições (cerca de 25 na versão antiga, 27 na recente)
- **Persianas e portas:** persianas 3D, porta de celeiro, persiana veneziana.
- **Varreduras:** linear, relógio, gradiente, estrela, íris.
- **Deslizar e empurrar:** deslizar, empurrar, dividir, comprimir, trocar.
- **Página:** virar, enrolar, descascar.
- **3D e movimento:** cascata 3D, entrada e saída 3D, embaralhar 3D, portais, espiral, zoom, fluxo de deformação.
- **Luz e mistura:** dissolver, flash, efeito cruzado.
- **Extensível:** uma transição que carrega efeitos GL de terceiros (versão recente).
- Neoveg: `parcial`. Há dezesseis transições (dissolver, cortina linear, cortina de relógio, íris, porta de celeiro, persianas, deslizar, crescer, espremer, espiral, flash, dividir, empurrar, rolar página, descolar página e cortina de gradiente; as três do meio enxergam o clipe anterior) na aba Transições, na entrada, na saída e na sobreposição, com a duração e a curva do fade. O **modelo no engine está feito**: a transição é guardada por evento (`transition_in` na entrada e na sobreposição, `transition_out` na saída; o eixo A/B é a camada que sai e a que entra), com a duração e a curva do fade e o envelope livre de progresso (`transition_progress`). `previsto`: as demais (3D, zoom, estrela), miniaturas, pré-ajustes e uma duração própria da transição, separada do fade.

## Geradores de mídia (6 em ambas as versões)
| Gerador | Neoveg |
| :--- | :--- |
| Títulos e texto | feito: contorno, sombra, animação e âncora vertical |
| Cor sólida | feito |
| Padrão de teste | feito |
| Gradiente de cor | feito: formas e ângulo |
| Tabuleiro (xadrez) | feito, entre os padrões de teste |
| Textura de ruído | feito: cinza ou colorido, grão, semente e modo animado |

Os geradores do Neoveg ficam em `neoveg_video::generators` e abrem por Faixas → Inserir mídia gerada.

## Próximos passos
- Uma duração própria da transição, separada do fade do evento.
- Miniaturas e pré-ajustes das transições, e as transições 3D.
