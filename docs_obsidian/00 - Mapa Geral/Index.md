---
tags: [neoveg, indice]
---
# Neoveg — Mapa Geral

Hub do vault. Abra o grafo (`Ctrl+G`) para ver como as partes se ligam; as cores seguem as pastas.

## 01 - Arquitetura Rust
A direção das dependências é `neoveg_common` → (`neoveg_audio`, `neoveg_video`, `neoveg_engine`) → `neoveg_ui`.
- [[neoveg_common]] — tipos de projeto, timecode SMPTE, curvas de fade, preferências, caminhos.
- [[neoveg_engine]] — timeline, comandos desfazíveis, regras de edição, arquivos `.neoveg`.
- [[neoveg_audio]] — decodificação, mixer, saída `cpal`, medidores.
- [[neoveg_video]] — quadros RGBA, geradores, `wgpu`, FFmpeg opcional.
- [[neoveg_ui]] — aplicativo `egui` / `eframe` com `egui_dock`.

## 02 - UI e Planejamento de Janelas
- [[especificacao_janelas]] — comportamento e redimensionamento de cada painel.
- [[inventario_icones]] — inventário clean-room e catálogo dos 99 ícones SVG autorais.
- `layout_janelas.canvas` — mapa visual do layout padrão do dock.

## 03 - Manual do Usuario
- [[guia_de_inicio]] → [[atalhos_teclado]] → [[subprojetos_e_abas]]

## 04 - Pesquisas e Benchmarks
- [[regras_de_timeline]] — regras extraídas dos manuais.
- [[desempenho_resolucoes]] — medições de desempenho.
- [[lacunas_funcionais]] — o que a pesquisa de referência prevê e o Neoveg ainda não registra ou não implementa.

## 05 - Referencia Funcional
- [[segundo_cerebro]] — hub do conhecimento sobre o editor de referência e como usar o vault.
- [[catalogo_efeitos_video]] · [[catalogo_transicoes_e_geradores]] · [[catalogo_efeitos_audio]] · [[estrutura_de_menus]]
- [[janelas_e_paineis]] · [[formatos_e_tecnologia]] · [[diferencas_entre_versoes]]

## ui_references
- [[fontes_de_referencia_de_ui]] — manuais, varredura e imagens extraídas cruzados por painel: [[medidas_da_interface]], [[painel_timeline_trilhas_eventos]], [[painel_preview_e_scopes]], [[painel_trimmer_midia_explorer]], [[painel_audio_master_e_mixer]], [[painel_efeitos_transicoes_geradores]], [[mapa_de_icones_por_funcao]], [[validacao_de_atalhos]].

## Fontes de verdade fora do vault
- `AGENTS.md` — padrões e regras (a seção 26 descreve este vault).
- `docs/research/nle_features_research.txt` — base de pesquisa completa.
- `README.md` — roadmap de recursos.

> As páginas de `03` e `04` têm blocos entre `<!-- auto:begin -->` e `<!-- auto:end -->` gerados por `scripts/update_obsidian_docs.py`. Não edite dentro deles.
