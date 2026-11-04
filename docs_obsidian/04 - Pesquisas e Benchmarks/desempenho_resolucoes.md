---
tags: [neoveg, benchmark, desempenho]
---
# Desempenho por resolução

Dados de desempenho gerados em `D:\ambiente_neoveg\results\`. A qualidade do preview define a resolução decodificada: Draft = 1/4, Preview = 1/2, Good e Best = cheia, Auto = ajusta à janela (ver [[especificacao_janelas]]).

## Como medir
- Qualidade do build: `python build_portable.py qa` (relatório em `coverage/qa/qa_report.json`).
- Os benchmarks Criterion (`benches/`) e perfis Tracy ainda não existem — ver `AGENTS.md`, seção 3.

## CPU contra GPU (04/11, Ryzen 5 5600GT, Radeon integrada)
Crossfade de duas camadas e cadeia de três efeitos (desfoque, brilho e contraste, vinheta), mediana do Criterion, com envio e leitura do quadro incluídos: o crossfade não ganha na GPU (0,8 a 1,0 vez), a cadeia de efeitos vai de 0,9 vez em 480p a 2,4 vezes em 4K, e em 8K a placa recusa o quadro. A tabela completa está no README. O custo parece estar no envio, na leitura e na conversão entre 8 bits e ponto flutuante, não nos efeitos; vale a pena na GPU o que é caro e em cadeia. Ver [[especificacao_janelas]] para a qualidade da prévia.

## Arquivos de resultados

<!-- auto:begin -->
Pasta de resultados ainda não existe: `D:\ambiente_neoveg\results`. Gere os dados de desempenho e rode o script de novo.
<!-- auto:end -->
