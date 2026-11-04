#!/usr/bin/env python3
"""Gera site-neoveg/historico.html a partir do git log e do roteiro do README.md.

Uso (na raiz do repositório): python scripts/gerar_site_historico.py
Rode de novo antes de cada publicação no Cloudflare Pages.
"""
import html, re, subprocess, sys
from collections import OrderedDict
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
SITE = RAIZ / "site-neoveg"
TIPOS = OrderedDict([("feat", "Novidade"), ("fix", "Correção"), ("perf", "Desempenho")])


def git_log():
    out = subprocess.run(
        ["git", "log", "--date=short", "--format=%ad\x1f%h\x1f%s"],
        cwd=RAIZ, capture_output=True, text=True, encoding="utf-8", check=True,
    ).stdout
    dias = OrderedDict()
    total = 0
    for linha in out.splitlines():
        data, h, assunto = linha.split("\x1f", 2)
        total += 1
        m = re.match(r"(feat|fix|perf)(?:\(([^)]*)\))?!?:\s*(.+)", assunto)
        if not m:
            continue
        dias.setdefault(data, []).append((m.group(1), m.group(2) or "", m.group(3), h))
    return dias, total


def titulo_item(txt):
    txt = re.sub(r"`", "", txt)
    txt = re.sub(r"\*\*", "", txt)
    cortes = [txt.find(c) for c in (": ", " (", ". ") if txt.find(c) > 20]
    if cortes:
        txt = txt[: min(cortes)]
    return txt[:170].strip().rstrip(",;")


def roteiro():
    feito, falta = [], []
    for linha in (RAIZ / "README.md").read_text(encoding="utf-8").splitlines():
        if linha.startswith("- [x] "):
            feito.append(titulo_item(linha[6:]))
        elif linha.startswith("- [ ] "):
            falta.append(titulo_item(linha[6:]))
    return list(OrderedDict.fromkeys(feito)), list(OrderedDict.fromkeys(falta))


def e(s):
    return html.escape(s, quote=True)


def main():
    dias, total = git_log()
    feito, falta = roteiro()
    n = {k: sum(1 for v in dias.values() for t in v if t[0] == k) for k in TIPOS}
    blocos = []
    for data, itens in dias.items():
        lis = "".join(
            f'<li><span class="tag {t}">{TIPOS[t]}</span>'
            f'{"<b>" + e(esc) + "</b> " if esc else ""}{e(txt)}</li>'
            for t, esc, txt, _ in itens
        )
        y, m, d = data.split("-")
        blocos.append(f'<details class="dia"><summary>{d}/{m}/{y} <small>{len(itens)} mudanças</small></summary><ul>{lis}</ul></details>')
    li_feito = "".join(f"<li>{e(x)}</li>" for x in feito)
    li_falta = "".join(f"<li>{e(x)}</li>" for x in falta)
    pagina = f"""<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Neoveg — O que já foi feito</title>
<meta name="description" content="Histórico completo do Neoveg: roteiro do que está pronto e todas as novidades, correções e ganhos de desempenho do repositório.">
<link rel="icon" type="image/svg+xml" href="favicon.svg">
<link rel="stylesheet" href="style.css">
</head>
<body>
<header class="top"><div class="wrap">
  <a class="logo" href="index.html">NEO<span>VEG</span></a>
  <nav><a href="index.html#recursos">Recursos</a><a href="index.html#telas">Telas</a><a href="#roteiro">Roteiro</a><a href="#historico">Histórico</a></nav>
</div></header>
<main>
<div class="wrap" style="padding-top:48px">
  <h1 style="font-size:2.4rem;margin:0 0 8px">O que já foi feito</h1>
  <p class="sub">Gerado a partir do repositório no GitHub: {total} commits, sendo {n['feat']} novidades, {n['fix']} correções e {n['perf']} melhorias de desempenho. {len(feito)} itens do roteiro estão prontos.</p>
</div>
<section id="roteiro" class="alt"><div class="wrap">
  <h2>Roteiro: pronto</h2>
  <p class="sub">Itens marcados como feitos no README do projeto.</p>
  <ul class="chk">{li_feito}</ul>
  <h2 style="margin-top:48px">Roteiro: ainda falta</h2>
  <ul class="chk todo">{li_falta}</ul>
</div></section>
<section id="historico"><div class="wrap">
  <h2>Histórico de mudanças</h2>
  <p class="sub">Do mais novo ao mais antigo. Clique em um dia para abrir.</p>
  {''.join(blocos)}
</div></section>
</main>
<footer><div class="wrap">Neoveg · Licença MIT · <a href="https://github.com/blackghoust9/neoveg/commits">Ver no GitHub</a></div></footer>
</body>
</html>
"""
    (SITE / "historico.html").write_text(pagina, encoding="utf-8", newline="\n")
    print(f"historico.html: {total} commits, {len(feito)} feitos, {len(falta)} pendentes")


if __name__ == "__main__":
    sys.exit(main())
