---
tags: [neoveg, arquitetura, rust]
crate: neoveg_common
---
# neoveg_common

Base do workspace; não depende de nenhuma outra crate do Neoveg.

## Responsabilidades
- Tipos do projeto (`project`), mídia (`MediaSource::File` / `SubProject`) e timecode SMPTE (drop-frame `;`, non-drop `:`).
- `FadeCurve` com cinco curvas: Linear, Fast, Slow, Smooth, Sharp.
- Tabela de comandos (`commands`) e mapa de teclas (`keymap`) — ver [[atalhos_teclado]].
- Preferências (`preferences.json`), caminhos por usuário (`paths`), log (`logging`).

## Regras
- Formatos persistidos levam `format_version` e campos novos usam `#[serde(default)]`.
- Erros viram `Result`; nada de `panic` por causa de arquivo ruim.

## Ligações
Usada por [[neoveg_engine]], [[neoveg_audio]], [[neoveg_video]] e [[neoveg_ui]]. Voltar ao [[Index]].
