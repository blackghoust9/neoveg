#!/usr/bin/env python3
"""Populates the Neoveg Obsidian vault (docs_obsidian/) from the project sources.

What it fills (everything between the `<!-- auto:begin -->` and `<!-- auto:end -->` markers; text
outside the markers is hand-written and never touched):

* `03 - Manual do Usuario/atalhos_teclado.md`   from `commands.rs` + `keymap.rs` (the real key map)
* `03 - Manual do Usuario/guia_de_inicio.md`    the list of reference manuals found in the PDF folder
* `03 - Manual do Usuario/subprojetos_e_abas.md` from section 24 of AGENTS.md
* `04 - Pesquisas e Benchmarks/regras_de_timeline.md` from docs/research/nle_features_research.txt
* `04 - Pesquisas e Benchmarks/desempenho_resolucoes.md` links to the files in the results folder

Only descriptions and page references are written, never text copied from the manuals.

Usage:
    python scripts/update_obsidian_docs.py [--vault DIR] [--pdfs DIR] [--results DIR] [--mirror DIR]

`--mirror` copies the finished vault (without `.obsidian/workspace*.json`) to a second place, for
example `D:\\ambiente_neoveg\\docs_obsidian`. The PDF folder defaults to the maintainer's Drive
mirror and `NEOVEG_PDF_DIR`; Python standard library only (`pypdf` is used for page counts when
installed).
"""
from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BEGIN = "<!-- auto:begin -->"
END = "<!-- auto:end -->"
DEFAULT_PDFS = r"G:\Meu Drive\vp-rev-eng\teste\teste2"
DEFAULT_MIRROR = r"D:\ambiente_neoveg\docs_obsidian"

CATEGORY_ORDER = ["File", "Edit", "Insert", "Cursor", "Transport", "Options", "Track", "View"]
CATEGORY_LABEL = {
    "File": "Arquivo",
    "Edit": "Editar",
    "Insert": "Inserir",
    "Cursor": "Cursor e seleção",
    "Transport": "Reprodução",
    "Options": "Opções",
    "Track": "Faixas",
    "View": "Exibir",
}

# Research entries summarised in "regras_de_timeline" (number -> topic).
RULE_ENTRIES = {
    2: "Snapping",
    6: "Fades e crossfades automáticos",
    9: "Variantes de snapping e quantização",
    10: "Envelopes de evento, switches, takes e grupos",
    14: "Automação, envelopes e keyframes",
    42: "Conferência com a ajuda oficial atual",
    43: "Conferência com os PDFs oficiais (v18)",
    44: "Estudo entre versões (JKL, preferências de edição)",
    45: "Manuais v8 e v14",
}


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def splice(path: Path, body: str, skeleton: str) -> bool:
    """Replaces the auto block of `path` (creating it from `skeleton` if missing). True if changed."""
    text = read(path) if path.exists() else skeleton
    block = f"{BEGIN}\n{body.rstrip()}\n{END}"
    if BEGIN in text and END in text:
        head, rest = text.split(BEGIN, 1)
        _, tail = rest.split(END, 1)
        new = head + block + tail
    else:
        new = text.rstrip() + "\n\n" + block + "\n"
    old = read(path) if path.exists() else None
    if old == new:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(new, encoding="utf-8", newline="\n")
    return True


def parse_commands(src: str) -> dict[str, dict]:
    pat = re.compile(r'(\w+)\s*=>\s*\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,\s*(\w+)\s*,\s*(true|false)\s*\)')
    return {
        m.group(1): {"id": m.group(2), "label": m.group(3), "cat": m.group(4), "done": m.group(5) == "true"}
        for m in pat.finditer(src)
    }


def parse_bindings(src: str) -> dict[str, list[str]]:
    start = src.find("const DEFAULT_BINDINGS")
    if start < 0:
        return {}
    block = src[start:]
    block = block[: block.find("];")]
    out: dict[str, list[str]] = {}
    for m in re.finditer(r"\(Command::(\w+),\s*&\[([^\]]*)\]\)", block):
        out[m.group(1)] = re.findall(r'"([^"]+)"', m.group(2))
    return out


def shortcuts_page() -> str:
    cmds = parse_commands(read(ROOT / "crates/neoveg_common/src/commands.rs"))
    binds = parse_bindings(read(ROOT / "crates/neoveg_common/src/keymap.rs"))
    lines = [
        f"Gerado de `commands.rs` e `keymap.rs` ({len(cmds)} comandos, {sum(1 for c in cmds if binds.get(c))} com atalho padrão).",
        "Itens marcados como *planejado* já têm atalho reservado, mas a função ainda não existe.",
        "",
    ]
    for cat in CATEGORY_ORDER:
        rows = [(n, c) for n, c in cmds.items() if c["cat"] == cat]
        if not rows:
            continue
        lines += [f"### {CATEGORY_LABEL[cat]}", "", "| Ação | Atalho | Id | Estado |", "| :--- | :--- | :--- | :--- |"]
        for name, c in rows:
            keys = " / ".join(f"`{k}`" for k in binds.get(name, [])) or "—"
            state = "implementado" if c["done"] else "*planejado*"
            lines.append(f"| {c['label']} | {keys} | `{c['id']}` | {state} |")
        lines.append("")
    return "\n".join(lines)


def pdf_inventory(pdf_dir: Path) -> list[tuple[str, int | None, float]]:
    folder = pdf_dir / "pdfs" if (pdf_dir / "pdfs").is_dir() else pdf_dir
    try:
        from pypdf import PdfReader  # type: ignore
    except ImportError:
        PdfReader = None
    items = []
    for p in sorted(folder.glob("*.pdf")) if folder.is_dir() else []:
        pages = None
        if PdfReader is not None and p.stat().st_size < 80_000_000:
            try:
                pages = len(PdfReader(str(p)).pages)
            except Exception:  # damaged or encrypted: the size is still useful
                pages = None
        items.append((p.name, pages, p.stat().st_size / 1_048_576))
    return items


def start_page(pdf_dir: Path) -> str:
    items = pdf_inventory(pdf_dir)
    if not items:
        return "Pasta de manuais não encontrada nesta máquina; execute o script onde o Drive está montado."
    lines = [
        "Manuais de referência lidos para a pesquisa (somente texto e descrições; as páginas ficam fora do repositório):",
        "",
        "| Arquivo | Páginas | MB |",
        "| :--- | ---: | ---: |",
    ]
    for name, pages, mb in items:
        lines.append(f"| {name} | {pages if pages is not None else '?'} | {mb:.1f} |")
    lines += ["", "Os fatos extraídos estão em [[regras_de_timeline]] e em `docs/research/nle_features_research.txt`."]
    return "\n".join(lines)


def subprojects_page() -> str:
    agents = read(ROOT / "AGENTS.md")
    m = re.search(r"^## 24\..*?(?=^---|\Z)", agents, re.S | re.M)
    if not m:
        return "Seção 24 do AGENTS.md não encontrada."
    keep = [ln for ln in m.group(0).splitlines()[1:] if ln.startswith("- **")]
    clean = [re.sub(r"`neoveg_[a-z_:A-Z]+`", lambda x: x.group(0), ln) for ln in keep]
    return "Resumo técnico (fonte: AGENTS.md, seção 24):\n\n" + "\n".join(clean)


def shorten(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    return text[:limit].rsplit(" ", 1)[0].rstrip(",;:") + " …"


def research_entries(src: str) -> dict[int, list[str]]:
    entries: dict[int, list[str]] = {}
    current = None
    for line in src.splitlines():
        m = re.match(r"^(\d+)\. Feature Name:", line)
        if m:
            current = int(m.group(1))
            entries[current] = [line]
        elif current is not None:
            entries[current].append(line)
    return entries


def rules_page() -> str:
    entries = research_entries(read(ROOT / "docs/research/nle_features_research.txt"))
    out = ["Resumo automático das entradas de pesquisa (texto completo em `docs/research/nle_features_research.txt`).", ""]
    for num, topic in RULE_ENTRIES.items():
        lines = entries.get(num)
        if not lines:
            continue
        out.append(f"### {num}. {topic}")
        bullets = [ln.strip() for ln in lines if ln.startswith("  - ")]
        out += [f"- {shorten(b[2:].strip(), 220)}" for b in bullets[:5]]
        src = next((ln.strip() for ln in lines if ln.startswith("Sources:")), "")
        if src:
            out.append(f"\n*{src[:200]}*")
        out.append("")
    return "\n".join(out)


def performance_page(results: Path) -> str:
    if not results.is_dir():
        return f"Pasta de resultados ainda não existe: `{results}`. Gere os dados de desempenho e rode o script de novo."
    files = sorted(p for p in results.rglob("*") if p.is_file())[:200]
    if not files:
        return f"A pasta `{results}` está vazia."
    lines = [f"Arquivos em `{results}`:", ""]
    for p in files:
        rel = p.relative_to(results).as_posix()
        lines.append(f"- [{rel}](<{p.as_uri()}>) ({p.stat().st_size / 1024:.0f} KB)")
    return "\n".join(lines)


def mirror(vault: Path, dest: Path) -> bool:
    """Copies the vault to `dest`; False (nothing written) when its drive is not present."""
    if dest.anchor and not Path(dest.anchor).exists():
        print(f"aviso: {dest.anchor} não existe nesta máquina; espelho em {dest} ignorado", file=sys.stderr)
        return False
    for src in vault.rglob("*"):
        rel = src.relative_to(vault)
        if src.is_dir() or rel.name in ("workspace.json", "workspace-mobile.json"):
            continue
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, target)
    return True


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--vault", default=str(ROOT / "docs_obsidian"))
    ap.add_argument("--pdfs", default=os.environ.get("NEOVEG_PDF_DIR", DEFAULT_PDFS))
    ap.add_argument("--results", default=str(Path(os.environ.get("NEOVEG_ENV_DIR", r"D:\ambiente_neoveg")) / "results"))
    ap.add_argument("--mirror", nargs="?", const=DEFAULT_MIRROR, default=None)
    args = ap.parse_args()

    vault = Path(args.vault)
    manual = vault / "03 - Manual do Usuario"
    research = vault / "04 - Pesquisas e Benchmarks"
    jobs = [
        (manual / "atalhos_teclado.md", shortcuts_page(), "# Atalhos de teclado\n"),
        (manual / "guia_de_inicio.md", start_page(Path(args.pdfs)), "# Guia de início\n"),
        (manual / "subprojetos_e_abas.md", subprojects_page(), "# Subprojetos e abas\n"),
        (research / "regras_de_timeline.md", rules_page(), "# Regras de timeline\n"),
        (research / "desempenho_resolucoes.md", performance_page(Path(args.results)), "# Desempenho por resolução\n"),
    ]
    changed = 0
    for path, body, skeleton in jobs:
        did = splice(path, body, skeleton)
        changed += did
        print(("atualizado " if did else "sem mudanças ") + str(path.relative_to(vault)))
    if args.mirror:
        if mirror(vault, Path(args.mirror)):
            print(f"espelhado em {args.mirror}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
