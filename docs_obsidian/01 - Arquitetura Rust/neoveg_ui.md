---
tags: [neoveg, arquitetura, rust]
crate: neoveg_ui
---
# neoveg_ui

Aplicativo `egui` / `eframe` (binário `neoveg`) com painéis acopláveis via `egui_dock`. Depende de todas as outras crates.

## Responsabilidades
- Janela principal, menus, barra de abas de projetos ([[subprojetos_e_abas]]).
- Painéis do dock: ver [[especificacao_janelas]] e o mapa em `layout_janelas.canvas`.
- Ícones planos desenhados com formas do `egui` (`icons.rs`) e SVGs em `assets/icons/`.
- Despacho de atalhos por comando (`shortcuts.rs`), nunca por tecla fixa — ver [[atalhos_teclado]].
- Tela de abertura (`splash.rs`) que acompanha as etapas reais de inicialização.

## Regras
- A thread da UI nunca bloqueia: decodificação, formas de onda e autosave rodam em workers.
- A lógica de estado (arrastar, hit-test, montar clipes) fica testável sem GUI; regras puras ficam em [[neoveg_engine]].
- Botões de toolbar mostram só ícones; nomes e atalhos aparecem em tooltips.

## Ligações
[[neoveg_engine]], [[neoveg_audio]], [[neoveg_video]], [[neoveg_common]]. Voltar ao [[Index]].
