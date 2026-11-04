---
tags: [neoveg, referencia, audio, efeitos]
---
# Catálogo de efeitos de áudio

Hub: [[segundo_cerebro]]. Implementação: [[neoveg_audio]].

## O que o editor de referência traz
- Um pacote de **11 efeitos básicos** (versão recente): chorus e flanger, compressor, de-esser, gate, limitador, phaser, reverb, delay estéreo, tremolo e pan, estágio valvulado e tira vocal.
- Hospedagem de plug-ins: VST2 na versão antiga; VST2 e VST3 na recente, com servidores de varredura isolados em processos separados.
- Efeitos DirectX descobertos pelo registro do sistema na versão antiga.

## No Neoveg
- `feito`: 28 efeitos nativos por faixa e no master, até 8 por cadeia, com bypass e compensação de latência; auto-ducking; redução de ruído por IA (RNNoise); medidor de loudness BS.1770-4.
- `feito`: hospedagem CLAP. `previsto`: VST3 em processo isolado e gerenciador de plug-ins.
- Os 11 básicos têm equivalente no Neoveg. O diferencial do Neoveg é o conjunto de limpeza de áudio (DeNoise, DeHum, DeReverb, DeEsser).

## Próximos passos
1. VST3 em processo isolado, reaproveitando o modelo de latência do `BlockMixer`.
2. Predefinições de cada efeito e importação de presets.
