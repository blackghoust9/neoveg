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

Cada arquivo mantém, no topo, o aviso de origem e de copyright.

## Alterações feitas pelo Neoveg

- Caminhos de módulo adaptados para `neoveg_audio::dsp` (`crate::...` virou `super::...`).
- Formatação com `rustfmt`; o código segue com o estilo original.
- Em `dsp/mod.rs`, as funções auxiliares (`flush` e utilitários de teste) foram reproduzidas a partir
  de `audio-dsp/src/lib.rs`, e alguns lints do clippy foram silenciados só nesse módulo.
- Os testes unitários dos três arquivos foram mantidos. Os testes de oráculo contra o `ffmpeg`
  (`audio-dsp/tests/loudness_oracle.rs`) **não** foram trazidos.

## Como o Neoveg usa

O callback de áudio (`neoveg_audio::playback`) alimenta o medidor com as amostras estéreo que vão de
fato para a placa de som e publica as leituras por atômicos (`neoveg_audio::readout`). O painel
**Loudness** da interface mostra essas leituras. O medidor não aloca memória durante a reprodução.

## Créditos

Os créditos aparecem em **Ajuda → Créditos e licenças…**, em [`AUTHORS.md`](../../AUTHORS.md) e na
tabela de [`third_party/README.md`](../README.md).
