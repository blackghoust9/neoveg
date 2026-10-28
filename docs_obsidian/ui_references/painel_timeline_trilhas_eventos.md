---
tags: [neoveg, referencia, ui, timeline]
---
# Painel: linha do tempo, trilhas e eventos

Hub: [[fontes_de_referencia_de_ui]]. Regras de edição já registradas: [[regras_de_timeline]]. Código: [[neoveg_ui]].

## Fontes
- Varredura: seções de linha do tempo, trilhas e eventos, que somam cerca de 1 500 itens de menu e 2 300 textos de interface (versão recente).
- Manuais: versão 21, páginas 68 a 79 (visão geral da linha do tempo e dos eventos), 372 (edição) e 388 a 395 (cabeçalho de trilha); versão 14, páginas 245 a 275 (trilhas) e 21 a 25 (visão geral).

## Hierarquia de elementos (da varredura)
1. **Régua** com formato de tempo selecionável, marcadores e regiões.
2. **Lista de trilhas** à esquerda, com um cabeçalho por trilha.
   - Botões do cabeçalho de vídeo e áudio: mudo, solo, armar gravação, efeitos da trilha, movimento da trilha, modo e nível de composição, aninhamento e automação.
   - Trilhas de barramento (áudio e vídeo) que se mostram ou ocultam.
3. **Área de eventos**, com forma de onda e quadros dentro do evento.
   - Cabeçalho do evento com botões: mídia gerada, Pan/Crop, efeitos e menu.
   - Alças de fade e de aparo nas bordas.
   - Cinco curvas de fade com nome: linear, rápida, lenta, suave e acentuada, mais "segurar".
   - Envelopes de volume, pan, velocidade e progresso de transição; ponto de keyframe.
4. **Barra de ferramentas de edição**: ferramenta normal, de seleção e de envelope; alinhamento (snap); quantizar para quadros; ripple automático; reprodução em laço.

## Estados e comportamentos validados nos manuais
- Mostrar e ocultar são interruptores de exibição: forma de onda, botões do evento, cabeçalho do evento, comprimento do fade, marcadores de mídia, informação do take ativo e bordas de edição (manual v18, lista de atalhos, páginas 3 e 4).
- O foco alterna entre a lista de trilhas e a linha do tempo.
- O zoom tem passos pequenos, grandes, até a seleção e até um quadro por miniatura.

## Estado no Neoveg
| Elemento | Estado |
| :--- | :--- |
| Régua, marcadores, regiões, snap, quantizar, ripple | `feito` (ver [[atalhos_teclado]]) |
| Cabeçalho de trilha (mudo, solo, fase) e pontinho piscante da trilha selecionada | `feito` |
| Cabeçalho do evento (gerada, Pan/Crop, efeitos, menu) | `parcial`: os botões existem, Pan/Crop e Track Motion são `previsto` |
| Curvas de fade (5) e crossfade automático | `feito` |
| Envelopes de volume e pan e de parâmetros de efeito | `feito` |
| Envelopes da faixa de vídeo (nível, esmaecer para cor, mudo) | `feito`, na prévia e na renderização |
| Envelope de velocidade do evento de vídeo (−100 % a 300 %, reverso, congelar no cursor) | `parcial`: falta desenhá-lo sobre o evento; o progresso de transição segue a curva do fade |
| Trilhas de barramento visíveis na linha do tempo | `sem plano` ainda |
| Alternar foco entre lista de trilhas e linha do tempo (`Tab`) | `sem plano` ainda |
| Altura das trilhas por atalho e minimizar | `sem plano` ainda |

## Próximos passos
1. Interruptores de exibição do evento (forma de onda, botões, cabeçalho) como comandos, com os atalhos da referência quando o §33 permitir.
2. Altura de trilha e minimizar, com `Ctrl+Shift+Cima` e `Ctrl+Shift+Baixo` (manual v18).
