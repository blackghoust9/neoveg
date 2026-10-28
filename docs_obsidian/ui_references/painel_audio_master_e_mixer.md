---
tags: [neoveg, referencia, ui, audio, master, mixer]
---
# Painel: barramento master e console de mixagem

Hub: [[fontes_de_referencia_de_ui]]. Código: [[neoveg_audio]].

## Fontes
- Varredura: seções do console de mixagem (cerca de 450 textos) e do barramento master (cerca de 215).
- Manuais: versão 21, páginas 869 a 917 (console) e 463, 865 a 901 (master); versão 14, páginas 349, 350, 368 e 372.

## Hierarquia de elementos
**Console de mixagem**
1. Uma tira por trilha de áudio: fader de entrada e de saída, pan (inclusive surround), mudo e solo, medidores de entrada e de saída.
2. **Inserções** antes e depois do fader, com automação de cada efeito.
3. **Envios** para barramentos e auxiliares, antes ou depois do volume, com nível e pan.
4. Barramentos de entrada, de auxiliar, de sintetizador e de saída, que se inserem e se removem.
5. Modos de automação e ajuste de travar canais do fader.

**Master**
1. Fader do master, medidores (inclusive downmix e saída mono ou estéreo) e o botão de atenuar a saída.
2. Medidores de loudness e registro de loudness.
3. Barramento de vídeo (efeitos de saída) e o barramento de prévia.

## Estado no Neoveg
| Elemento | Estado |
| :--- | :--- |
| Barramento master com medidores L e R reais, fader e leituras em dBFS | `feito` |
| Loudness (momentâneo, curto prazo, integrado, LRA, pico verdadeiro) | `feito` |
| Tira da trilha (volume, pan, mudo, solo, fase) | `feito`, dentro da aba de efeitos de áudio |
| Inserções (cadeia de até 8 efeitos por trilha e no master) | `feito`, só antes do fader (a referência também tem inserções depois do fader) |
| Envios, auxiliares, barramentos e janela de console completa | `previsto` |
| Pan surround | `sem plano` |
| Atenuar saída (dim) | `previsto` |

## Próximos passos
1. Janela de console com todas as tiras lado a lado.
2. Envios e auxiliares, que pedem um modelo de barramento no engine.
