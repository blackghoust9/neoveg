---
tags: [neoveg, pesquisa, lacunas, roadmap]
---
# Lacunas funcionais

Cruzamento entre o inventário funcional de uma varredura local do produto de referência (menus, janelas, painéis, comandos) e o que o Neoveg já registra no roadmap e na pesquisa. Volta ao hub: [[Index]]. Catálogos por tema: [[segundo_cerebro]]. Regras de edição relacionadas: [[regras_de_timeline]]. Implementação: [[neoveg_engine]], [[neoveg_ui]], [[neoveg_video]], [[neoveg_audio]].

> Clean room (AGENTS.md §28): esta nota descreve **funções** com palavras próprias. Não há texto, captura, ícone ou nome de marca copiados. A varredura bruta fica fora do Git (`results_assets/vegasscan/`, ignorada); ela é só evidência de apoio, e o comportamento de cada recurso continua vindo dos manuais (§33).

## Método e limites
- Fontes comparadas: `README.md` (roadmap com caixas marcadas), `docs/research/nle_features_research.txt` e a tabela de comandos de `neoveg_common`.
- Foram checadas **74 funções** da varredura por termos nesses textos. Termo achado = "registrado"; não prova que está implementado.
- O status "implementado" vem das caixas `[x]` do roadmap e da flag `implemented()` da tabela de comandos, não de teste manual do aplicativo.
- A varredura foi estática (recursos e metadados dos arquivos instalados). Atalhos padrão, posições em pixels e comportamento de arrastar **não** foram obtidos; seguem pendentes de conferência nos manuais.

## Resultado em uma linha
70 das 74 funções já têm registro útil no roadmap ou na pesquisa; **4 não têm**: três sem nenhuma menção e uma citada só como item de menu.

## 1. Sem registro útil (decidir: adotar ou descartar)
| Função | Observação | Sugestão |
| :--- | :--- | :--- |
| Importar arquivo de projeto de outro editor (formato proprietário) | Só existem FCP7/FCPXML/EDL/OTIO; AAF consta apenas como item de menu na pesquisa | Registrar em `nle_features_research.txt`; avaliar AAF antes do formato proprietário |
| Envelope de vídeo "esmaecer para cor" por faixa | Os demais envelopes de vídeo (opacidade, velocidade, progresso de transição) estão previstos | Incluir na fase de envelopes de vídeo |
| Conexão com serviço online de clipes (nuvem) | Há apenas "Hub de mídia" como item genérico | Decisão de produto; provavelmente fora do escopo |
| Sobreposição de quadros de borda de edição no evento | Aparece só numa lista de itens de menu da pesquisa | Detalhar quando houver o modo de edição expandida |

## 2. Registradas, ainda não implementadas — por prioridade sugerida
**Alta (núcleo de edição e saída)**
- Escopos de ripple, origens de snap e quantização, modos de aparo (shuffle, stretch, split-trim, rolling): Fases 2 e 3 do plano detalhado no README.
- Envelopes e keyframes de vídeo, velocity, composição pai/filho, Pan/Crop e Track Motion com gizmo.
- Efeitos e transições de vídeo na GPU (incluindo OFX) e cadeias de efeitos por evento, faixa, mídia e projeto.
- Barramentos, sends e Mixing Console completo.
- Janela de Mídia do Projeto e Explorer; propriedades do projeto com espaço de cor; renderização com predefinições e pré-renderização.

**Média**
- Takes, modo de edição expandida, eventos de ajuste, proxies, correspondência de cor e OCIO.
- Hospedagem VST3 (CLAP já existe), gerenciador de plug-ins, scripting do usuário.
- Cola de atributos de evento, fechar lacunas, divisão inteligente, reparo de evento.
- Switches de vídeo (manter último quadro, mistura de quadros, fluxo óptico, redução de cintilação).
- Exportar/importar preferências; picos de áudio em cache e reconstrução manual.

**Baixa**
- Gravação de disco, saída para fita, exportação para disco de câmera profissional, extração de áudio de CD.
- Timecode MIDI, monitor externo, edição multicâmera.
- IA: texto para fala, reenquadramento automático, assistente.

## 3. Comandos da tabela ainda sem ação
Hoje 15 comandos em `neoveg_common::commands` têm atalho e rótulo mas a interface os ignora: novo projeto vazio, fechar projeto, próximo e anterior take, marcador de comando, renderizar para nova faixa, parar, reproduzir do início, alinhar à grade, a marcadores e a todos os eventos, quantizar para quadros e três escopos de ripple. Os quatro de snap/quantização e os de ripple dependem das Fases 2 e 3.

## 4. Pontos a conferir no próprio repositório
- O README ainda marca "aplicar o idioma em toda a interface" como pendente, mas `Assets/i18n` já tem oito idiomas. Conferir e atualizar a caixa.
- "Composição de várias faixas de vídeo" está concluída e "composição e Pan/Crop do evento" pendente; existe `composite.rs` em três crates. Conferir se a segunda linha trata só de Pan/Crop.
- Os quatro itens da seção 1 precisam de entrada em `docs/research/nle_features_research.txt` (AGENTS.md §9.1) antes de virarem tarefa.

## Próximos passos sugeridos
1. Decidir os quatro itens da seção 1.
2. Atacar as Fases 2 e 3 de edição, porque destravam sete dos 15 comandos parados (quatro de snap/quantização e três de ripple).
3. Abrir a frente de vídeo (envelopes, Pan/Crop, GPU), hoje a maior área pendente.
4. Rodar `python scripts/update_obsidian_docs.py` depois de qualquer mudança em comandos ou atalhos.
