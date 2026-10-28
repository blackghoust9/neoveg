---
tags: [neoveg, referencia, ui, preview, scopes]
---
# Painel: visualização de vídeo e escopos

Hub: [[fontes_de_referencia_de_ui]]. Código: [[neoveg_video]] e [[neoveg_ui]].

## Fontes
- Varredura: seção da janela de visualização e das janelas de análise (cerca de 150 textos).
- Manuais: versão 21, páginas 205, 422, 794 e 813 (visualização), 798 a 803 (escopos); versão 14, páginas 209 e 588 a 631.

## Hierarquia de elementos
1. **Barra da visualização**: dispositivo de saída, qualidade (melhor, boa, rascunho e automática), sobreposições (áreas seguras, grade), vista dividida, monitor externo, tela cheia, captura de quadro.
2. **Quadro** com proporção de pixel e rotação de saída do projeto.
3. **Escopos de vídeo**: forma de onda, vectorscópio, histograma e parada RGB; opção de atualizar enquanto toca.
4. **Medidores de loudness** em janela própria.
5. **Pré-visualização em RAM** da seleção ("renderizar para a memória").

## Estados validados nos manuais
- Cada sobreposição é um interruptor independente e vem desligada.
- A qualidade escolhe a resolução e o desfoque: a automática acompanha a janela.
- Vista dividida compara com e sem efeito.

## Estado no Neoveg
| Elemento | Estado |
| :--- | :--- |
| Qualidade (Rascunho, Prévia, Boa, Melhor, Auto) com amostragem de pixel visível | `feito` |
| Sobreposições (HUD SMPTE, áreas seguras, grade) desligadas por padrão | `feito` |
| Janela destacada em outro monitor | `feito` (AGENTS.md §32) |
| Captura de quadro, cópia e tela cheia | especificados no AGENTS.md §4.2; estado exato **a confirmar** |
| Escopos (forma de onda, vectorscópio, histograma, parada RGB) | `feito` |
| Vista dividida com e sem efeito | `previsto`; a cadeia de efeitos já existe, falta o botão |
| Monitor externo dedicado | `parcial`: janela destacada em outro monitor, sem saída de dispositivo |
| Pré-visualização em RAM | `previsto` (barra de renderização verde, amarela e vermelha) |
| Prévia ao vivo de um controle arrastado | `feito` (efeitos de vídeo) |

## Próximos passos
1. Botão de vista dividida usando a cadeia de efeitos (com e sem).
2. Pré-visualização em RAM da seleção.
