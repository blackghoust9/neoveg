---
tags: [neoveg, referencia, ui, indice]
---
# Referências de interface: manuais, varredura e recursos extraídos

Hub das notas de `ui_references/`. Volta ao hub do cofre: [[Index]]. Resumo funcional mais amplo: [[segundo_cerebro]].

## As três fontes e o papel de cada uma
| Fonte | O que traz | O que **não** traz |
| :--- | :--- | :--- |
| `manuais/` (21 PDFs, versões 8 a 21) | comportamento: atalhos, ferramentas, estados ligado/desligado, fluxos | medidas em pixels |
| `VegasScan/` (varredura estática dos arquivos instalados) | hierarquia de menus e painéis, nomes de controles, tamanho de poucos diálogos em unidades de diálogo, tamanho das tiras de bitmaps | posição e tamanho dos painéis, espaçamento, cores, atalhos padrão (o próprio README da varredura diz que isso exige inspeção manual) |
| `results_assets/` (384 imagens extraídas, nomeadas pelo número do recurso) | **medidas** dos bitmaps: células de 16 px e as escalas 20, 32 e 64 px | qual botão usa qual imagem (o número do recurso não diz) |

As três pastas ficam fora do Git (`.gitignore`). Só estas notas entram no repositório, e só com descrições e medidas (AGENTS.md §28 e §37).

As páginas dos manuais citadas nas notas por painel são **candidatas**: foram achadas por busca de títulos no texto dos PDFs e devem ser conferidas ao abrir o manual. O que as notas afirmam sobre comportamento vem do texto dos manuais e da lista de itens da varredura; o que é inferência vem marcado como tal.

## Matriz de paridade
- [[matriz_de_paridade]] — os 872 comandos da referência agrupados por função, com o estado no Neoveg e a prioridade. Gerada por `scripts/parity_matrix.py`.

## Notas por painel
- [[medidas_da_interface]] — medidas reais encontradas e o que elas mudam (ou não) no Neoveg.
- [[painel_timeline_trilhas_eventos]]
- [[painel_preview_e_scopes]]
- [[painel_trimmer_midia_explorer]]
- [[painel_audio_master_e_mixer]]
- [[painel_efeitos_transicoes_geradores]]
- [[mapa_de_icones_por_funcao]] — função da referência, ícone original do Neoveg.
- [[validacao_de_atalhos]] — o que os manuais dizem dos atalhos de painel e do estado no Neoveg.

## O que não foi feito, e por quê
- **Nenhuma imagem de `results_assets/` foi copiada** para `crates/neoveg_ui/assets/icons/` nem adaptada. São os bitmaps de um produto comercial; levá-los para o repositório vai contra o AGENTS.md §28 (arte 100 % original, `.bmp` extraído de software comercial nunca entra no Git). Os ícones do Neoveg continuam sendo os 99 SVG autorais e os 27 BMP de Flat Color Icons (MIT).
- **O `theme.rs` não foi alterado.** Ele só define cores; não há métrica de layout nele. Dos números encontrados, o único que se aplica aos botões (célula de 16 px) já bate com o que o Neoveg desenha (ver [[medidas_da_interface]]). Espaçamentos e tamanhos de painel não existem na varredura, e inventá-los contraria o §33.
