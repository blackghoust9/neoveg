---
tags: [neoveg, arquitetura, rust]
crate: neoveg_audio
---
# neoveg_audio

Áudio em Rust puro. Depende só de [[neoveg_common]].

## Responsabilidades
- Decodificação com `symphonia`; saída com `cpal`.
- Mixer multifaixa: soma os eventos ativos honrando `trim_in`, ganho, fades e crossfades.
- Medidores: pico L/R (`meter::PeakMeter`) e loudness EBU R128 (`dsp::loudness`).
- Efeitos por faixa e no master (`dsp::effects`, `fxmixer::BlockMixer`), auto-ducking e compensação de latência.
- Fonte de forma de onda com amostras reais (`waveform`).

## Regras
- O callback de áudio não aloca, não usa locks nem faz I/O: lê um plano de mixagem imutável e atômicos.
- Os medidores respondem somente ao pipeline real; parados, ficam zerados.

## Ligações
Usada por [[neoveg_ui]] (painéis Barramento Master e Loudness, ver [[especificacao_janelas]]). Voltar ao [[Index]].
