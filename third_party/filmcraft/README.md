# FilmCraft (código adaptado)

| | |
| :--- | :--- |
| Projeto | **FilmCraft**, editor não linear em Rust puro |
| Repositório | <https://github.com/storytold/filmcraft> (commit `8fda2f2`, 2026-10-25) |
| Autores | ArtCraft Team and the FilmCraft contributors |
| Licença | `MIT OR Apache-2.0`. O Neoveg usa o código **sob a licença MIT**; o texto está em [`LICENSE-MIT`](LICENSE-MIT) e acompanha todos os pacotes |
| Crate de origem | `crates/audio-dsp` (`filmcraft-audio-dsp`) |

## O que foi copiado

| Arquivo no Neoveg | Origem | Função |
| :--- | :--- | :--- |
| `crates/neoveg_audio/src/dsp/loudness.rs` | `audio-dsp/src/loudness.rs` | Medidor ITU-R BS.1770-4 / EBU R128: filtro K, loudness momentâneo (400 ms), de curto prazo (3 s) e integrado (porta dupla), faixa de loudness (EBU Tech 3342), pico de amostra e pico verdadeiro |
| `crates/neoveg_audio/src/dsp/biquad.rs` | `audio-dsp/src/biquad.rs` | Filtros biquad (receitas RBJ) usados pelo filtro K |
| `crates/neoveg_audio/src/dsp/oversample.rs` | `audio-dsp/src/oversample.rs` | Sobreamostragem polifásica do pico verdadeiro |
| `crates/neoveg_audio/src/dsp/effect.rs` | trecho de `audio-dsp/src/lib.rs` | Trait `AudioEffect`, descrição de parâmetros (`ParamSpec`, `Unit`) e funções auxiliares |
| `crates/neoveg_audio/src/dsp/smooth.rs` | `audio-dsp/src/smooth.rs` | Suavização de parâmetros por amostra |
| `crates/neoveg_audio/src/dsp/effects/eq.rs` | `audio-dsp/src/effects/eq.rs` | Equalizador paramétrico e de 3 bandas |
| `crates/neoveg_audio/src/dsp/effects/dynamics.rs` | `audio-dsp/src/effects/dynamics.rs` | Compressor, expansor/gate e limitador de pico verdadeiro |
| `crates/neoveg_audio/src/dsp/effects/time.rs` | `audio-dsp/src/effects/time.rs` | Delay e reverb |
| `crates/neoveg_audio/src/dsp/effects/filters.rs` | `audio-dsp/src/effects/filters.rs` | Equalizadores gráficos de 10, 20 e 30 bandas, paramétrico completo, notch, filtros clássicos (Butterworth, Chebyshev, Bessel, Elíptico) e filtro FFT |
| `crates/neoveg_audio/src/dsp/effects/restoration.rs` | `audio-dsp/src/effects/restoration.rs` | DeHum (pente de notches em 50/60 Hz) e DeNoise (porta espectral por STFT) |
| `crates/neoveg_audio/src/dsp/effects/stft.rs` | `audio-dsp/src/effects/stft.rs` | Processamento por STFT usado pelo DeNoise e pelo filtro FFT |
| `crates/neoveg_audio/src/dsp/design.rs` | `audio-dsp/src/design.rs` | Projeto de filtros clássicos em seções biquad |
| `crates/neoveg_audio/src/dsp/fft.rs` | `audio-dsp/src/fft.rs` | FFT |
| `crates/neoveg_audio/src/dsp/ducking.rs` | `audio-dsp/src/ducking.rs` | Análise do auto-ducking: regiões de atividade e curva de ganho (os keyframes de descida e subida) |
| `crates/neoveg_audio/src/dsp/effects/modulation.rs` | `audio-dsp/src/effects/modulation.rs` | Chorus/flanger, flanger, phaser, delay analógico e delay de várias batidas |
| `crates/neoveg_audio/src/dsp/effects/pitch.rs` | `audio-dsp/src/effects/pitch.rs` | Mudança de tom (pitch shifter por STFT) |
| `crates/neoveg_audio/src/dsp/effects/declick.rs` | `audio-dsp/src/effects/declick.rs` | Removedor automático de cliques |
| `crates/neoveg_audio/src/dsp/effects/essential.rs` | `audio-dsp/src/effects/essential.rs` | DeEsser, DeReverb, realce de fala e largura estéreo |
| `crates/neoveg_audio/src/dsp/effects/multiband.rs` | `audio-dsp/src/effects/multiband.rs` | Compressor multibanda, rack de dinâmica e compressor valvulado |
| `crates/neoveg_audio/src/dsp/effects/channel.rs` | `audio-dsp/src/effects/channel.rs` | Amplificar, volume por canal, balanço e inverter fase |
| `crates/neoveg_audio/src/dsp/effects/util.rs` | `audio-dsp/src/effects/util.rs` | Linhas de atraso, filtros de um polo, LFOs e envelopes usados pelos efeitos |
| `crates/neoveg_engine/src/captions.rs` | `captions/src/srt.rs`, `captions/src/vtt.rs` e funções de relógio e de decodificação de `captions/src/lib.rs` | Leitura e escrita de legendas SubRip (`.srt`) e WebVTT (`.vtt`) tolerantes a arquivos reais |
| `crates/neoveg_audio/src/dsp/effects/mod.rs` | trecho de `audio-dsp/src/effects/mod.rs` | Registro dos efeitos (só os 7 que o Neoveg oferece) e seus testes de consistência |

## O que foi adaptado (reescrito)

| Arquivo no Neoveg | Origem | Função |
| :--- | :--- | :--- |
| `crates/neoveg_video/src/vfx.rs` e `crates/neoveg_video/src/vfx_gpu.rs` | `render/src/gpufx.rs` (`FxOp::eval` e `FxOp::apply`) e `gpu/src/fx.wgsl` | Fórmulas dos efeitos de cor Equilíbrio de cor, Níveis, Correção de gama, Posterizar, Extrair e Matiz, reescritas para o buffer de trabalho do Neoveg (cor codificada e pré-multiplicada, parâmetros numéricos do registro `ParamSpec`) e conferidas por teste entre a CPU e a GPU |

Cada arquivo copiado mantém, no topo, o aviso de origem e de copyright.

## Alterações feitas pelo Neoveg

- Caminhos de módulo adaptados para `neoveg_audio::dsp` (`crate::...` virou `super::...`).
- Formatação com `rustfmt`; o código segue com o estilo original.
- Em `dsp/mod.rs`, as funções auxiliares (`flush` e utilitários de teste) foram reproduzidas a partir
  de `audio-dsp/src/lib.rs`, e alguns lints do clippy foram silenciados só nesse módulo.
- Em `effects/mod.rs` só foram registrados os 28 efeitos usados (os de `audio-dsp` que dependem de convolução, ambisonics, guitarra e do modelo de projeto ficaram de fora); os nomes dos efeitos, dos parâmetros e das opções de escolha ficaram em pt-BR (mantendo os identificadores internos dos parâmetros) e foram acrescentados `has_latency` e `Category::label`.
- Em `captions.rs` o tempo passou de ticks para segundos (`f64`), as cadeias `let` da edição 2024 foram reescritas para a 2021, e o modelo de projeto do FilmCraft (faixas de legenda, ticks) foi trocado pelo `Caption` e pelos comandos do Neoveg, que são código próprio.
- Os testes unitários de todos os arquivos copiados foram mantidos. Os testes de oráculo contra o `ffmpeg`
  (`audio-dsp/tests/loudness_oracle.rs`) **não** foram trazidos.

## Como o Neoveg usa

- **Loudness:** o callback de áudio (`neoveg_audio::playback`) alimenta o medidor com as amostras estéreo que vão de fato para a placa de som e publica as leituras por atômicos (`neoveg_audio::readout`). O painel **Loudness** da interface mostra essas leituras.
- **Auto-ducking:** `neoveg_audio::duck` mede o nível das faixas de controle a cada 10 ms, acha os trechos com som com as funções do FilmCraft (`dsp::ducking`) e monta a curva de ganho da faixa abaixada; a análise roda ao iniciar a reprodução. A medição do nível é código do Neoveg.
- **Efeitos:** a cadeia de cada faixa e a do master ficam no projeto (`neoveg_engine::effects`); `neoveg_audio::fxmixer` cria os efeitos fora do callback e roda as cadeias em blocos dentro dele. O painel **Efeitos de Áudio** edita a cadeia da faixa selecionada ou do master.
- Nada disso aloca memória durante a reprodução.

## Créditos

Os créditos aparecem em **Ajuda → Créditos e licenças…**, em [`AUTHORS.md`](../../AUTHORS.md) e na
tabela de [`third_party/README.md`](../README.md).
