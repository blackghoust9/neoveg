---
tags: [neoveg, referencia, versoes, roadmap]
---
# Diferenças entre a versão antiga e a recente

Hub: [[segundo_cerebro]]. A comparação mostra para onde o editor de referência evoluiu em oito anos e ajuda a priorizar o [[lacunas_funcionais]].

## Números gerais
| Medida | Antiga | Recente |
| :--- | :--- | :--- |
| Arquivos instalados | 793 | 2149 |
| Itens do menu principal | 419 | 441 |
| Diálogos | 139 | 118 |
| Módulos gerenciados | 102 | 216 |
| Layouts padrão | 3 | 12 |

Leitura: o programa ficou maior pelo lado de IA e serviços online, enquanto a interface clássica quase não mudou.

## O que a versão recente **tirou**
- Captura de vídeo, foto de scanner, publicação e envio online, exportação para console portátil, gravação de CD de vídeo e de multimídia.
- Música gerada, texto como mídia (no formato antigo), editor de áudio externo, painéis de gerenciador de mídia e de tarefas.
- Seleção de qualidade de prévia no menu Exibir (passou para o painel da prévia).
- Disposição de multicâmera por data e por timecode (trocada pela sincronização automática).
- O diálogo de ponte com estação de áudio externa.

## O que a versão recente **acrescentou**
- **Edição:** fechar lacunas, reparo de evento, divisão inteligente, selecionar tudo depois do cursor, colar atributos seletivos, eventos e faixas de ajuste (com redimensionamento ao projeto), inserir faixa de fluxo ausente.
- **Interruptores do evento:** normalização automática, mistura de quadros, manter o último quadro, fluxo óptico, repetir evento, aparar para incluir todos os quadros.
- **Áudio:** controlador e ouvinte de auto-ducking, loudness em janela própria.
- **Legendas:** inserir de arquivo e de regiões.
- **Projeto e preferências:** salvar incremental, salvar avançado, tela de boas-vindas, exportar e importar preferências, quantizar para quadros com exibição de bordas não quantizadas, tempo do projeto na régua.
- **Vídeo:** gradação de cor, rastreamento de movimento, limpar a prévia em RAM; vários predefinidos de 360° e de formato vertical.
- **IA:** assistente, reenquadramento automático, fala para texto e texto para fala.
- **Janelas:** notas do projeto, comprimento e cabeçalho de evento, hub online.
- **Menus de contexto:** cada pop-up de evento de áudio e de vídeo ganhou de 9 a 12 itens.

## O que o Neoveg já tem dessa lista
`feito`: salvar incremental, tela de boas-vindas, quantizar para quadros, notas do projeto, auto-ducking, loudness, legendas de arquivo, ruler de tempo. `previsto`: fechar lacunas, divisão inteligente, colar atributos seletivos, eventos de ajuste, interruptores de vídeo (fluxo óptico, mistura de quadros), exportar e importar preferências, gradação de cor, rastreamento. `sem plano`: hub online, captura, publicação.

## Próximos passos
1. Eventos e faixas de ajuste: abrem a porta para efeitos de grupo sem copiar a cadeia por evento.
2. Interruptores de vídeo (manter último quadro, mistura de quadros) pelo custo baixo.
3. Exportar e importar preferências (o `preferences.json` já é serializável).
