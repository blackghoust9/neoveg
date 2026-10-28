---
tags: [neoveg, segundo-cerebro, indice]
---
# Segundo cérebro do Neoveg

Esta pasta guarda o que se sabe sobre o **editor de referência** em palavras próprias e liga cada função ao que o Neoveg já faz. Volta ao hub: [[Index]].

## Origem e regras
- Fonte: uma varredura estática local de duas versões do editor de referência (menus, diálogos, plug-ins e tabelas de texto lidos dos arquivos instalados). A pasta bruta `VegasScan/` fica **fora do Git** (`.gitignore`); só estas notas entram no repositório.
- Decisão do mantenedor (2026-10-28): resumir o conteúdo em notas versionadas. As notas descrevem **funções**, sem copiar textos de menu, strings, capturas, ícones nem nomes de marca (AGENTS.md §28). O comportamento exato de cada recurso continua vindo dos manuais (§33).
- A varredura não executou o programa: atalhos padrão, medidas em pixels e interação de arrastar não constam aqui.

## Mapa das notas
- [[catalogo_efeitos_video]] — efeitos de vídeo agrupados por finalidade, com o estado no Neoveg.
- [[catalogo_transicoes_e_geradores]] — transições e geradores de mídia.
- [[catalogo_efeitos_audio]] — efeitos de áudio e hospedagem de plug-ins.
- [[estrutura_de_menus]] — como o editor de referência organiza menus e importação/exportação.
- [[janelas_e_paineis]] — painéis acopláveis, layouts e diálogos de propriedades.
- [[formatos_e_tecnologia]] — formatos de arquivo, aceleração, IA e arquitetura.
- [[diferencas_entre_versoes]] — o que mudou entre a versão antiga e a recente.
- Cruzamento com o roadmap: [[lacunas_funcionais]].

## Como usar o vault no dia a dia
1. **Antes de planejar um recurso**, procure a função nestas notas e em [[lacunas_funcionais]] e veja o estado no Neoveg.
2. **Ao implementar**, atualize a nota (estado `feito`, link para o módulo, por exemplo [[neoveg_video]]) no mesmo commit.
3. **Ao descobrir algo novo**, registre em `docs/research/nle_features_research.txt` (§9.1) e depois resuma aqui com um link `[[...]]`.
4. Cada nota termina com os próximos passos; o grafo (`Ctrl+G`) mostra as ligações.

Estados usados nas notas: `feito` (no código e nos testes), `parcial`, `previsto` (no roadmap), `sem plano`.
