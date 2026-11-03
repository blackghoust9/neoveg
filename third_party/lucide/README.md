# Lucide Icons

Origem de parte dos ícones BMP da interface em `crates/neoveg_ui/assets/icons/*.bmp` (AGENTS.md seção 35).

- Repositório: https://github.com/lucide-icons/lucide (site: https://lucide.dev, conjunto `lucide` no Iconify)
- Licença: `ISC` (com partes de Feather sob `MIT`), texto integral em `LICENSE`
- Arquivos usados: listados em `scripts/fetch_and_convert_icons.py` (`ICON_MAP`, prefixo `"lucide"`)
- Alterações: cada definição vetorial foi obtida via API oficial do Iconify e convertida em BMP de 32 bits BGRA (`BITMAPV4HEADER`, 64x64, branco puro não-pré-multiplicado com canal alpha) por `scripts/fetch_and_convert_icons.py`.
- Sem uso de marcas do autor.
