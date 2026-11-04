# nnnoiseless (dependência)

| | |
| :--- | :--- |
| Projeto | **nnnoiseless**, porte em Rust puro do **RNNoise** (supressão de ruído por rede neural recorrente) |
| Repositório | <https://github.com/jneem/nnnoiseless> (versão `0.5.2`, crates.io) |
| Autores | Joe Neeman; o RNNoise original é de Jean-Marc Valin, Mozilla Corporation e Xiph.Org Foundation (veja [`AUTHORS`](AUTHORS) e [`COPYING`](COPYING)) |
| Licença | `BSD-3-Clause`. O texto, com os avisos de copyright, está em [`COPYING`](COPYING) e acompanha todos os pacotes |
| Uso no Neoveg | É uma **dependência do Cargo** de `neoveg_audio` (não foi copiado). O efeito **Redução de ruído por IA (RNNoise)** (`neoveg_audio::dsp::effects::rnnoise`) a envolve: monta quadros de 10 ms a 48 kHz, roda a rede e devolve o resultado, com compensação de latência e conversão de taxa para outros dispositivos |

A licença BSD-3-Clause exige reproduzir o aviso de copyright e a isenção de garantia em distribuições
binárias e proíbe usar o nome da Xiph.Org Foundation ou dos colaboradores para promover o produto
sem permissão.
