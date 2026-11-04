---
tags: [neoveg, referencia, ui, trimmer, midia]
---
# Painel: Trimmer, mídia do projeto e explorador

Hub: [[fontes_de_referencia_de_ui]]. Código: [[neoveg_engine]] e [[neoveg_ui]].

## Fontes
- Varredura: seções do Trimmer, da mídia do projeto e do explorador.
- Manuais: versão 21, páginas 153 (mídia do projeto), 311 e 318 (Trimmer), 232 (explorador); versão 14, páginas 84 a 130 (mídia) e 158 a 162 (Trimmer).
- Diálogo de propriedades da mídia: 222 × 257 unidades, 34 controles (ver [[medidas_da_interface]]).

## Hierarquia de elementos
**Mídia do projeto**
1. Lista de mídias com vista em lista ou miniaturas.
2. **Pastas de mídia** (criar, renomear, apagar, mover, buscar).
3. Propriedades: nome da fita, timecode do arquivo ou personalizado, taxa de quadros substituta, ordem dos campos, proporção do pixel, canal alfa, cor de fundo.
4. Ações: remover não usadas, criar subclipe, criar proxy de vídeo e de áudio, recapturar o que está fora do ar.
5. Estado "fora do ar" por mídia e por fluxo.

**Trimmer**
1. Vídeo e áudio da origem, com marcas de início e fim.
2. Histórico de mídias abertas (anterior, próxima, ordenar, limpar).
3. Selecionar só vídeo, só áudio ou ambos; escolher fluxo e canal de origem.
4. Adicionar à linha do tempo: do cursor, até o cursor, sobrescrever e encaixar na seleção.
5. Marcadores e regiões da origem, subclipes, vídeo em monitor externo.

**Explorador**: navegação por arquivos, dispositivos e a vista unificada da versão recente.

## Estado no Neoveg
| Elemento | Estado |
| :--- | :--- |
| Lista de mídia e importação | `feito` |
| Trimmer (marcas, adicionar do cursor, até o cursor, encaixar) | `feito` |
| Histórico do Trimmer | `parcial` |
| Pastas de mídia, subclipes e proxies | `previsto` |
| Diálogo de propriedades da mídia (timecode, proporção do pixel, alfa) | `previsto`; entrou no roteiro do README |
| Explorador de arquivos | `previsto` |
| Relink de mídia fora do ar | `feito` (passos 1 e 2 do AGENTS.md §5) |

## Próximos passos
1. Diálogo de propriedades da mídia.
2. Pastas de mídia e subclipes.
