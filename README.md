# Neoveg

Editor não linear de vídeo e áudio multiplataforma, escrito em **Rust puro** (Cargo workspace com `egui`/`eframe`, `wgpu`, `cpal`, `symphonia`).

> **Em migração.** A branch `refactor/100-percent-rust` substitui a arquitetura anterior (C# / Avalonia, Rust FFI, C++ e Python) por um único workspace Rust. Os diretórios legados (`desktop-host/`, `desktop-host.Tests/`, `rust-core/`, `cpp-extensions/`, `python-backend/`) continuam no repositório apenas como referência até cada recurso ser portado e, em seguida, serão removidos.

Consulte [AGENTS.md](./AGENTS.md) para os protocolos de arquitetura (seção 22: *Pure Rust Architecture & Single-Toolchain Protocol*) e [REQUIREMENTS.md](./REQUIREMENTS.md) para o ambiente de desenvolvimento.

## Pré-requisito único: toolchain do Rust

A única dependência de compilação do projeto é a toolchain oficial do Rust (`rustup` / `cargo`, versão 1.80 ou superior). Não são necessários .NET, CMake, compilador C++ nem Python.

```bash
# Windows: winget install --id Rustlang.Rustup --exact
# Linux/macOS:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy rustfmt
```

No Linux, a janela e o áudio exigem as bibliotecas de sistema do desktop (ver [REQUIREMENTS.md](./REQUIREMENTS.md)).

## Compilar, testar e executar

```bash
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo run --release -p neoveg_ui          # abre o Neoveg com um projeto vazio
```

A decodificação de vídeo com FFmpeg é opcional e fica atrás da feature `ffmpeg` da crate `neoveg_video` (exige as bibliotecas de desenvolvimento do FFmpeg): `cargo build -p neoveg_video --features ffmpeg`.

## Estrutura do workspace

| Crate | Responsabilidade |
| --- | --- |
| `crates/neoveg_common` | Tipos de projeto, timecode SMPTE (drop-frame), log (`logs/neoveg.log`, `logs/crash.log`) e preferências. |
| `crates/neoveg_audio` | Decodificação (`symphonia`), saída de áudio (`cpal`), medidores de pico reais e waveform amostra a amostra. |
| `crates/neoveg_video` | Quadros RGBA, geradores procedurais (barras SMPTE), upload para GPU (`wgpu`) e decodificação opcional (`ffmpeg-next`). |
| `crates/neoveg_engine` | Timeline, faixas, eventos, divisão/movimento e comandos com Desfazer/Refazer. |
| `crates/neoveg_ui` | Aplicação `egui` com painéis acopláveis (`egui_dock`), abas no rodapé, slider de precisão, régua com grade de frames adaptativa e cabeçalhos de faixa. |

## Estado atual

- A janela principal abre com projeto vazio (0 faixas, 0 mídias), painéis Visualização, Linha do Tempo, Mídia do Projeto, Trimmer e Barramento Master.
- Já portados: waveform amostra a amostra, grade de frames adaptativa, timecode drop-frame, slider de precisão, medidores de pico alimentados só por amostras reais (zerados com a reprodução parada), divisão de eventos e Desfazer/Refazer.
- Ainda **não** portados: decodificação de vídeo na prévia, mixer multifaixa (a reprodução toca o primeiro evento de áudio sob o cursor), efeitos, transições, importação/exportação XML/EDL, relink de mídia ausente, projetos `.neoveg` e scripting.
- Esta migração foi escrita sem a toolchain do Rust disponível no ambiente do autor: rode `cargo test --workspace` e `cargo clippy --workspace -- -D warnings` antes de qualquer outra coisa.

## Créditos e licenciamento

O código do projeto é distribuído sob a licença [MIT](./LICENSE). Consulte [AUTHORS.md](./AUTHORS.md) e [third_party/README.md](./third_party/README.md) para atribuições e licenças das dependências. Preserve esses arquivos ao redistribuir os pacotes.
