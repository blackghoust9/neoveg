# Phosphor Icons

Origem dos ícones BMP de transporte e edição em `crates/neoveg_ui/assets/icons/*.bmp` (AGENTS.md seção 35).

- Repositório: https://github.com/phosphor-icons/core (site: https://phosphoricons.com, conjunto `ph` no Iconify)
- Licença: `MIT`, texto integral em `LICENSE-MIT`
- Arquivos usados: listados em `scripts/fetch_and_convert_icons.py` (`ICON_MAP`, prefixo `"ph"`)
- Alterações: cada definição vetorial foi obtida via API oficial do Iconify e convertida em BMP de 32 bits BGRA (`BITMAPV4HEADER`, 64x64, branco puro não-pré-multiplicado com canal alpha) por `scripts/fetch_and_convert_icons.py`.
- Sem uso de marcas do autor.
