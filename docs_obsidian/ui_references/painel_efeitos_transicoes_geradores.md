---
tags: [neoveg, referencia, ui, efeitos, transicoes, geradores]
---
# Painel: efeitos, transições, geradores, Pan/Crop e movimento de trilha

Hub: [[fontes_de_referencia_de_ui]]. Catálogos: [[catalogo_efeitos_video]] e [[catalogo_transicoes_e_geradores]]. Código: [[neoveg_video]].

## Fontes
- Varredura: seções de efeitos, transições, geradores, Pan/Crop e movimento de trilha.
- Manuais: versão 21, páginas 496 e 602 (transições), 628 (geradores), 1112 (Pan/Crop e movimento), 518 a 522 e 573 a 574 (efeitos de desfoque, nitidez e vinheta); versão 14, páginas 292 a 314 (Pan/Crop e movimento).

## Hierarquia de elementos
1. **Três bibliotecas em abas**: efeitos, transições e geradores, com miniaturas e pré-ajustes por item.
2. **Cadeia de efeitos** em quatro níveis (mídia, evento, faixa, projeto) e a janela do efeito com controles, quadros-chave e atalho de bypass.
3. **Pan/Crop do evento**: quadro de origem e de destino, trava de proporção, posição, orientação, rotação e quadros-chave.
4. **Movimento da trilha**: posição, escala e rotação 2D e 3D, quadros-chave, modo e nível de composição, pai e filho.
5. **Rastreador de movimento** (versão recente) em janela própria.
6. **Gradação de cor** em modo próprio.

## Estado no Neoveg
| Elemento | Estado |
| :--- | :--- |
| Cadeia de efeitos de vídeo (mídia, evento, faixa, projeto) | `feito`: 4 efeitos, GPU, prévia ao vivo ao arrastar |
| Biblioteca de efeitos com miniaturas | `previsto`; hoje é uma lista no botão "Adicionar efeito" |
| Transições (biblioteca, parâmetros, envelope de progresso) | `previsto` |
| Geradores (cor, padrão de teste, gradiente, título) | `feito`; tabuleiro e textura de ruído `previsto` |
| Pan/Crop do evento (caixa, rotação, quadros-chave, prévia ao lado) | `feito`, sem máscaras nem centro de rotação |
| Movimento da trilha (caixa, picture-in-picture, rotação 2D, quadros-chave) | `feito`, sem 3D, sombra, brilho nem movimento do pai |
| Composição e modos de mistura (17) | `feito` no compositor; painel de compositores `previsto` |
| Rastreamento de movimento | `previsto` |
| Correção de cor primária e LUT | `feito`; gradação por evento `previsto` |

## Próximos passos
1. Biblioteca com miniaturas e pré-ajustes (pede o registro de efeitos com categoria).
2. Máscaras do Pan/Crop, e sombra e brilho do movimento da trilha.
