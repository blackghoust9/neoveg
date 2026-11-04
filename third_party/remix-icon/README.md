# Remix Icon

Origem de ícones BMP de mídia, áudio e vídeo em `crates/neoveg_ui/assets/icons/*.bmp` (AGENTS.md seção 35).

- Repositório: https://github.com/Remix-Design/RemixIcon (site: https://remixicon.com, conjunto `ri` no Iconify)
- Licença: `Apache-2.0`, texto integral em `LICENSE-APACHE`
- Arquivos usados: listados em `scripts/fetch_and_convert_icons.py` (`ICON_MAP`, prefixo `"ri"`)
- Alterações: cada definição vetorial foi obtida via API oficial do Iconify e convertida em BMP de 32 bits BGRA (`BITMAPV4HEADER`, 64x64, branco puro não-pré-multiplicado com canal alpha) por `scripts/fetch_and_convert_icons.py`.
- Sem uso de marcas do autor.
