#!/usr/bin/env python3
"""Mostra as alterações locais de um repositório Git sem exibir seu conteúdo.

Uso:
    python scripts/ver_mudancas.py
    python scripts/ver_mudancas.py C:\\caminho\\para\\outro\\repositorio
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run_git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="surrogateescape",
        check=True,
    )
    return result.stdout


def changed_files(repository: Path) -> list[tuple[str, str]]:
    output = run_git(
        repository, "status", "--porcelain=v1", "-z", "--untracked-files=all"
    )
    records = iter(output.split("\0"))
    changes = []

    for record in records:
        if not record:
            continue
        status = record[:2]
        path = record[3:]
        if "R" in status or "C" in status:
            original_path = next(records, "")
            path = f"{path} (origem: {original_path})"
        changes.append((status, path))

    return changes


def category(status: str) -> str:
    if status == "??":
        return "novo"
    if "U" in status:
        return "conflito"
    if "R" in status:
        return "renomeado"
    if "A" in status:
        return "adicionado"
    if "D" in status:
        return "removido"
    return "modificado"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Lista mudanças locais registradas pelo Git, sem mostrar o conteúdo."
    )
    parser.add_argument(
        "repository",
        nargs="?",
        type=Path,
        default=ROOT,
        help=f"caminho do repositório (padrão: {ROOT})",
    )
    args = parser.parse_args()
    repository = args.repository.expanduser().resolve()

    try:
        root = Path(run_git(repository, "rev-parse", "--show-toplevel").strip())
        branch = run_git(repository, "status", "--short", "--branch").splitlines()
        changes = changed_files(repository)
        diff_summary = run_git(repository, "diff", "--shortstat", "HEAD", "--").strip()
    except FileNotFoundError:
        print("Erro: Git não foi encontrado no PATH.", file=sys.stderr)
        return 2
    except subprocess.CalledProcessError as error:
        detail = error.stderr.strip() or "o caminho não é um repositório Git válido"
        print(f"Erro ao consultar o Git: {detail}", file=sys.stderr)
        return 2

    print(f"Repositório: {root}")
    print(branch[0] if branch else "Branch: não identificada")

    if not changes:
        print("\nNenhuma alteração local detectada.")
        return 0

    counts: dict[str, int] = {}
    for status, _ in changes:
        label = category(status)
        counts[label] = counts.get(label, 0) + 1

    print(f"\nArquivos alterados: {len(changes)}")
    print(", ".join(f"{count} {label}" for label, count in counts.items()))
    print("\nDetalhes:")
    for status, path in changes:
        print(f"  {status}  {path}")

    if diff_summary:
        print(f"\nResumo das diferenças rastreadas: {diff_summary}")
    if counts.get("novo"):
        print("Arquivos novos ainda não têm diff até serem adicionados ao Git.")
    print("\nAs colunas de status mostram separadamente alterações preparadas e não preparadas.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
