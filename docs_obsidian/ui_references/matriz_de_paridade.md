---
tags: [neoveg, referencia, ui, paridade, roteiro]
---
# Matriz de paridade funcional

Gerada por `scripts/parity_matrix.py`. Hub: [[fontes_de_referencia_de_ui]]. Voltar: [[Index]].

Cada comando da varredura da versão recente do editor de referência (872 comandos únicos) pertence a **uma** linha abaixo. As linhas descrevem a função em palavras próprias, sem copiar o texto dos menus (AGENTS.md §28 e §37). A coluna *Comandos* diz quantos comandos da referência a linha cobre; a coluna *No Neoveg* diz o que existe hoje.

**Estados:** `feito`, `parcial`, `previsto` (está no roteiro do README), `sem plano` (fora do escopo, com motivo) e `a classificar`.  
**Prioridade:** P1 define "igual" no uso diário, P2 é importante, P3 é baixa ou opcional.

## Resumo por área

| Área | Comandos | No escopo | Feito | Parcial | Previsto | Sem plano | Cobertura* |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Arquivo | 37 | 25 | 19 | 2 | 4 | 12 | 80 % |
| Editar | 100 | 100 | 18 | 56 | 26 | 0 | 46 % |
| Inserir | 32 | 30 | 16 | 5 | 9 | 2 | 62 % |
| Ferramentas | 56 | 38 | 6 | 9 | 23 | 18 | 28 % |
| Opções | 57 | 54 | 10 | 5 | 39 | 3 | 23 % |
| Exibir | 75 | 70 | 14 | 38 | 18 | 5 | 47 % |
| Ajuda | 14 | 7 | 0 | 7 | 0 | 7 | 50 % |
| Menus de contexto e auxiliares | 501 | 438 | 24 | 249 | 165 | 63 | 34 % |
| **Total** | **872** | **762** | | | | | **38 %** |

Por **linha da matriz** (cada linha é uma função, sem o peso do número de comandos): 35 feitas, 33 parciais, 43 previstas e 18 sem plano, de 129 linhas (111 no escopo).

\* Cobertura = (feito + metade do parcial) sobre os comandos no escopo, isto é, sem contar os de `sem plano`. É uma medida por **comando da referência**, não por esforço: um comando de menu pode esconder uma função enorme (por exemplo, o Pan/Crop).

## Onde está o trabalho (P1 e P2 abertos)

| Prioridade | Função | Área | Comandos | Estado |
| :--- | :--- | :--- | ---: | :--- |
| P1 | Envelope de velocidade do evento de vídeo | Inserir | 1 | parcial |
| P2 | Importar e exportar AAF | Arquivo | 3 | previsto |
| P2 | Colar atributos do evento, completo e seletivo | Editar | 2 | previsto |
| P2 | Divisão inteligente, curar evento dividido e fechar lacunas | Editar | 3 | previsto |
| P2 | Redimensionar eventos de ajuste ao projeto | Editar | 2 | previsto |
| P2 | Modo de edição expandida (entrar e sair) | Editar | 2 | previsto |
| P2 | Ferramentas de edição: normal, seleção, envelope, zoom, próxima e anterior | Editar | 5 | parcial |
| P2 | Interruptores de vídeo: último quadro, mistura de quadros, fluxo óptico, reamostragem, proporção | Editar | 9 | previsto |
| P2 | Takes do evento: renomear, escolher, próximo, anterior, apagar | Editar | 6 | previsto |
| P2 | Envelope do progresso de uma transição de vídeo | Inserir | 1 | parcial |
| P2 | Inserir faixa de ajuste | Inserir | 1 | previsto |
| P2 | Inserir barramento de áudio, de entrada e efeito atribuível | Inserir | 3 | previsto |
| P2 | Inserir fluxo ausente, evento vazio e evento de ajuste | Inserir | 3 | previsto |
| P2 | Inserir transição de áudio | Inserir | 1 | parcial |
| P2 | Efeitos de áudio do evento e da faixa, aplicar fora de tempo real | Ferramentas | 4 | parcial |
| P2 | Pan/Crop do evento: caixa de seleção, zoom, movimento, rotação e quadros-chave | Ferramentas | 1 | parcial |
| P2 | Movimento da trilha: posição, escala e rotação da camada com quadros-chave | Ferramentas | 1 | parcial |
| P2 | Modo de gradação de cor e rastreamento de movimento | Ferramentas | 2 | parcial |
| P2 | Renderizar para nova faixa | Ferramentas | 1 | previsto |
| P2 | Pré-renderização seletiva e prévia dinâmica em RAM | Ferramentas | 4 | previsto |
| P2 | Formato da régua (amostras, tempo, quadros, compassos, filme, SMPTE) | Opções | 18 | previsto |
| P2 | Layouts de janela: organizar, salvar, recarregar, padrão e os dez salvos | Exibir | 25 | parcial |
| P2 | Janela do explorador de arquivos | Exibir | 2 | previsto |
| P2 | Janela de transições | Exibir | 1 | parcial |
| P2 | Janelas de compositores e gerenciador de plug-ins | Exibir | 2 | previsto |
| P2 | Fader de prévia do mixer e trilhas de barramento | Exibir | 3 | previsto |
| P2 | Interruptores de exibição do evento: marcadores, take, ondas, botões, fades, bordas, cabeçalho, comprimento | Exibir | 9 | previsto |
| P2 | Mostrar envelopes de áudio e de vídeo | Exibir | 12 | parcial |
| P2 | Menu de contexto do evento (abrir no Trimmer, aninhar, propriedades, escopo de cena) | Menus de contexto e auxiliares | 33 | parcial |
| P2 | Menu de contexto da faixa (pasta, duplicar, apagar, cores de esmaecimento) | Menus de contexto e auxiliares | 35 | parcial |
| P2 | Menu de arrastar com o botão direito e do explorador | Menus de contexto e auxiliares | 16 | previsto |
| P2 | Menus do Trimmer (trilhas de áudio e vídeo, subclipe, captura de quadro) | Menus de contexto e auxiliares | 30 | parcial |
| P2 | Menus de envelope e de ponto-chave (forma, definir valor, copiar e colar) | Menus de contexto e auxiliares | 47 | parcial |
| P2 | Menus do console: pan surround, modelos de pan, envios antes ou depois do fader, entradas | Menus de contexto e auxiliares | 33 | previsto |
| P2 | Interruptores da barra de edição (agrupar, travar envelopes, crossfade, ferramentas) | Menus de contexto e auxiliares | 11 | parcial |
| P2 | Colunas e vistas da lista de mídia e do explorador (nome, tipo, taxas, tamanho, data, árvore, resumo) | Menus de contexto e auxiliares | 42 | previsto |
| P2 | Ações da mídia do projeto: proxy, trocar arquivo, remover, cenas, tempo, binário, propriedades do formato | Menus de contexto e auxiliares | 31 | previsto |
| P2 | Ações do explorador: abrir pasta, nova pasta, favoritos, tocar, copiar, mover, importar de câmera e CD | Menus de contexto e auxiliares | 19 | previsto |

## Detalhe por área

### Arquivo

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Novo projeto, abrir, tela de boas-vindas e sair | 5 | feito | `file.new`, `file.open`, tela de boas-vindas | - |
| Fechar o projeto atual | 1 | parcial | fecha pela aba; o comando `file.close` não tem ação | P3 |
| Salvar, salvar como, versão incremental, propriedades do projeto | 4 | feito | `file.save`, `file.save_as`, `file.save_incremental`, `file.properties` | - |
| Assistente de filme e renderizar como | 2 | feito | Arquivo > Renderizar como (MP4, MOV ProRes, WebM, WAV) | - |
| Renderizar áudio em tempo real incluindo entradas externas | 1 | sem plano | depende de gravação multicanal em tempo real | - |
| Importar e adicionar mídia | 2 | feito | Importar mídia (áudio, vídeo, imagem, sequência) | - |
| Importar mídia de outro projeto | 1 | previsto | sub-projetos existem; importar só a mídia não | P3 |
| Importar e exportar lista de edição (EDL) | 2 | feito | `neoveg_engine::edl` (CMX 3600) | - |
| Importar e exportar XML de edição e FCPXML | 4 | feito | `neoveg_engine::fcp7` e `fcpxml` | - |
| Importar e exportar AAF | 3 | previsto | roteiro: primeiro formato profissional a avaliar | P2 |
| Importar e exportar projeto de outro editor (formato fechado) | 2 | sem plano | formato proprietário; a ponte são XML, FCPXML, EDL, OTIO e AAF | - |
| Exportar o projeto no formato fechado do editor de referência | 1 | sem plano | o Neoveg usa `.neoveg` (ZIP aberto) | - |
| Importar legendas e closed caption | 1 | parcial | SRT e WebVTT feitos; closed caption de TV não | P3 |
| Importar Broadcast Wave e disco de câmera, exportar para disco profissional e console | 4 | sem plano | Broadcast Wave abre como WAV; mídia de disco de câmera e console ficam fora | - |
| Captura de vídeo e extração de faixas de CD de áudio | 2 | sem plano | captura e CD ficam fora do escopo atual | - |
| Preparar e gerenciar clipes de um serviço online do fabricante | 2 | sem plano | serviço fechado do fabricante | - |

### Editar

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Desfazer e refazer | 2 | feito | `edit.undo`, `edit.redo` (um gesto, um passo) | - |
| Desfazer tudo e limpar o histórico | 2 | previsto | o histórico existe; faltam os dois comandos | P3 |
| Recortar, copiar, colar, colar inserindo e colar repetido | 5 | feito | `edit.cut`, `edit.copy`, `edit.paste`, `edit.paste_insert`, `edit.paste_repeat` | - |
| Colar atributos do evento, completo e seletivo | 2 | previsto | roteiro (README) | P2 |
| Excluir eventos ou faixas | 1 | feito | `edit.delete` e `edit.ripple_delete` | - |
| Aparar à seleção, início ou fim ao cursor | 3 | feito | `edit.trim`, `edit.trim_start`, `edit.trim_end` | - |
| Dividir no cursor | 1 | feito | `edit.split` | - |
| Divisão inteligente, curar evento dividido e fechar lacunas | 3 | previsto | roteiro (README) | P2 |
| Redimensionar eventos de ajuste ao projeto | 2 | previsto | eventos de ajuste são `previsto` | P2 |
| Navegar: ir ao tempo, início e fim, quadro, marcador, ponto de edição, centralizar | 20 | parcial | a maioria feita (`cursor.*`); faltam passo por grade, pixel, evento selecionado e cursor para a reprodução | P3 |
| Escopo do ripple pós-edição | 4 | parcial | 3 dos 4 escopos (`edit.ripple.*`); sem faixas de barramento | P3 |
| Selecionar tudo, depois do cursor e bordas do evento | 4 | parcial | tudo e bordas (aparo por teclado) feitos; depois do cursor não | P3 |
| Seleção de tempo: laço, zoom, vista, projeto, início e fim, restaurar | 10 | parcial | entrada, saída e região de laço feitas; as demais não | P3 |
| Modo de edição expandida (entrar e sair) | 2 | previsto | roteiro (README) | P2 |
| Ferramentas de edição: normal, seleção, envelope, zoom, próxima e anterior | 5 | parcial | normal, slip, slide e embaralhar com ciclo (`D`); faltam esticar e dividir-aparar | P2 |
| Interruptores do evento: mudo, trava, fase, laço, normalizar | 5 | feito | `edit.switch.*` | - |
| Interruptores de vídeo: último quadro, mistura de quadros, fluxo óptico, reamostragem, proporção | 9 | previsto | roteiro (README) | P2 |
| Takes do evento: renomear, escolher, próximo, anterior, apagar | 6 | previsto | `edit.take.*` têm atalho, sem ação | P2 |
| Grupos: criar, remover, limpar, selecionar, recortar, copiar e apagar tudo | 7 | parcial | criar, remover, desagrupar e selecionar feitos (`edit.group`, `edit.ungroup`) | P3 |
| Escolha do fluxo de origem e dos canais (ambos, esquerdo, direito, combinar, trocar) | 6 | parcial | canais feitos (`ChannelMode`); fluxo de origem não | P3 |
| Menu de extensões | 1 | feito | gerenciador de extensões (Exibir) | - |

### Inserir

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Envelopes de áudio: volume e pan | 2 | feito | `track.volume_envelope`, `track.pan_envelope` | - |
| Envelope de mudo de áudio | 1 | previsto | volume e pan feitos; mudo não | P3 |
| Auto-ducking: controlador e ouvinte | 3 | feito | auto-ducking por faixa (`neoveg_audio::duck`) | - |
| Envelopes de faixa de vídeo: mudo, nível de composição e esmaecer para cor | 3 | feito | Faixas > Envelopes de vídeo e `V` na faixa de vídeo; vale na prévia e na renderização | - |
| Envelope do progresso de uma transição de vídeo | 1 | parcial | o progresso segue a curva do fade (cinco curvas); falta o envelope livre com pontos | P2 |
| Envelope de velocidade do evento de vídeo | 1 | parcial | envelope de -100 % a 300 %, reverso, congelar quadro no cursor e prévia ao vivo feitos; falta desenhá-lo sobre o evento na linha do tempo, o áudio vinculado acompanhar e a velocidade nas exportações | P1 |
| Inserir faixa de áudio e de vídeo | 2 | feito | `insert.audio_track`, `insert.video_track` | - |
| Inserir faixa de ajuste | 1 | previsto | roteiro (eventos de ajuste) | P2 |
| Inserir barramento de áudio, de entrada e efeito atribuível | 3 | previsto | roteiro (barramentos e envios) | P2 |
| Inserir legendas de arquivo e de regiões | 2 | parcial | de arquivo (SRT, WebVTT) feito; de regiões não | P3 |
| Inserir fluxo ausente, evento vazio e evento de ajuste | 3 | previsto | roteiro (README) | P2 |
| Inserir texto, mídia gerada e apresentação de slides | 3 | feito | Faixas > Inserir mídia gerada, janela de slideshow | - |
| Inserir transição de áudio | 1 | parcial | crossfade automático e curvas de fade; sem janela de transições | P2 |
| Inserir tempo, marcador e região | 3 | feito | `insert.time`, `insert.marker`, `insert.region` | - |
| Inserir faixa e índice de CD de áudio | 2 | sem plano | CD de áudio fora do escopo | - |
| Inserir marcador de comando | 1 | previsto | `insert.command` tem atalho, sem ação | P3 |

### Ferramentas

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Abrir o evento em editor de áudio externo | 4 | sem plano | sem editor de áudio externo | - |
| Editar o projeto de origem de um evento aninhado | 1 | feito | duplo clique no sub-projeto abre a aba | - |
| Efeitos de áudio do evento e da faixa, aplicar fora de tempo real | 4 | parcial | faixa e master feitos (28 efeitos); efeitos por evento não | P2 |
| Efeitos de vídeo da mídia, do evento, da faixa e da saída | 4 | feito | cadeias de mídia, evento, faixa e projeto (4 efeitos) | - |
| Pan/Crop do evento: caixa de seleção, zoom, movimento, rotação e quadros-chave | 1 | parcial | janela com caixa e alças, campos numéricos, quadros-chave com suavidade e prévia ao lado; faltam máscaras, centro de rotação e o botão no cabeçalho do evento | P2 |
| Movimento da trilha: posição, escala e rotação da camada com quadros-chave | 1 | parcial | janela com caixa, picture-in-picture, rotação 2D, suavidade e prévia ao lado; faltam 3D, sombra, brilho e movimento do pai de um grupo | P2 |
| Modo de gradação de cor e rastreamento de movimento | 2 | parcial | correção de cor primária e LUT feitas; gradação por evento e rastreamento não | P2 |
| Ferramentas de depuração de efeitos, transições, geradores e compositores | 4 | sem plano | uso interno do fabricante | - |
| Renderizar para nova faixa | 1 | previsto | `tools.render_to_new_track` tem atalho, sem ação | P2 |
| Prévia em monitor externo | 1 | parcial | janela destacada em outro monitor; sem saída de dispositivo | P3 |
| Pré-renderização seletiva e prévia dinâmica em RAM | 4 | previsto | roteiro: barra verde, amarela e vermelha | P2 |
| Ferramentas de IA: fala para texto, texto para fala, reenquadramento, assistente | 4 | previsto | roteiro (fase de IA; Whisper) | P3 |
| Imprimir em fita, gravar disco e montar CD de áudio | 9 | sem plano | mídia física fora do escopo | - |
| Gerar registro de loudness | 1 | previsto | o medidor existe; o registro em arquivo não | P3 |
| Religar mídia de adaptador sem fio do fabricante | 1 | sem plano | hardware proprietário | - |
| Salvar avançado | 1 | previsto | sem plano de detalhe | P3 |
| Executar scripts e reexaminar a pasta de scripts | 2 | previsto | roteiro: scripting em Rust | P3 |
| Menu de extensões | 1 | feito | gerenciador de extensões | - |
| Multicâmera: sincronizar, criar, expandir e editar | 8 | previsto | roteiro (README) | P3 |
| Limpar mídia do projeto e reconstruir áudio aninhado | 2 | previsto | relink existe; a limpeza não | P3 |

### Opções

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Quantizar para quadros e mostrar bordas não quantizadas | 2 | parcial | quantizar feito; mostrar bordas não | P3 |
| Alinhar à grade, a marcadores e a todos os eventos | 3 | feito | `options.snap_*` | - |
| Espaçamento da grade (marcas da régua, tempo, compassos, notas) | 14 | previsto | a grade é automática; escolher o passo não | P3 |
| Formato da régua (amostras, tempo, quadros, compassos, filme, SMPTE) | 18 | previsto | roteiro (fase 12); hoje SMPTE com e sem drop | P2 |
| Fade rápido nas edições de áudio | 1 | previsto | roteiro (preferência `quick_fade_ms`) | P3 |
| Ripple automático e escopo do ripple | 5 | feito | `options.auto_ripple`, `edit.ripple.*` | - |
| Deslizar todos os takes | 1 | previsto | depende dos takes | P3 |
| Metrônomo | 1 | feito | Ferramentas > Metrônomo | - |
| Ignorar todos os efeitos de áudio e silenciar todo áudio ou vídeo | 3 | previsto | sem plano de detalhe | P3 |
| Timecode e relógio MIDI | 3 | sem plano | MIDI fora do escopo | - |
| Personalizar a barra de ferramentas e a da linha do tempo | 2 | previsto | sem plano de detalhe | P3 |
| Personalizar o teclado | 1 | feito | Editar > Personalizar teclado | - |
| Preferências e exportar ou importar preferências | 3 | parcial | preferências feitas; exportar e importar não | P3 |

### Exibir

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Barra de ferramentas e barra de status | 2 | feito | barra e status | - |
| Layouts de janela: organizar, salvar, recarregar, padrão e os dez salvos | 25 | parcial | layouts 1 a 9 e padrão na sessão; sem disco nem organizar | P2 |
| Foco na linha do tempo | 1 | feito | `view.focus_timeline` | - |
| Janelas: Trimmer, master, visualização, mídia, notas, escopos, detalhes de edição, loudness | 8 | feito | `view.*` e janelas do cofre | - |
| Janelas de efeitos de vídeo e de geradores | 2 | feito | aba Efeitos de Vídeo; diálogo de geradores | - |
| Janela do explorador de arquivos | 2 | previsto | roteiro (Explorer) | P2 |
| Janela de transições | 1 | parcial | aba Transições (`Ctrl+Alt+6`) com 8 transições, entrada e saída, parâmetros e duração; faltam miniaturas, pré-ajustes e favoritos | P2 |
| Janelas de compositores e gerenciador de plug-ins | 2 | previsto | roteiro (README) | P2 |
| Janelas de pan surround, explorador de dispositivos, disco profissional e hub online | 5 | sem plano | hardware e serviço do fabricante | - |
| Fader de prévia do mixer e trilhas de barramento | 3 | previsto | roteiro (barramentos) | P2 |
| Interruptores de exibição do evento: marcadores, take, ondas, botões, fades, bordas, cabeçalho, comprimento | 9 | previsto | roteiro: atalhos de visualização | P2 |
| Mostrar envelopes de áudio e de vídeo | 12 | parcial | áudio e, no vídeo, nível, esmaecer e mudo feitos; velocidade, progresso, desfoque de movimento e supersampling não | P2 |
| Minimizar todas as trilhas e reconstruir picos de áudio | 2 | previsto | roteiro: cache de picos | P3 |
| Menu de extensões | 1 | feito | gerenciador de extensões | - |

### Ajuda

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Conteúdo, atalhos, tutoriais, manual e comunidade | 7 | parcial | atalhos e créditos; sem manual embutido | P3 |
| Atualizações, ativação, hardware e memória, rotinas de depuração | 7 | sem plano | ativação e depuração do fabricante; atualização fica com o pacote | - |

### Menus de contexto e auxiliares

| Função | Comandos | Estado | No Neoveg | Prio. |
| :--- | ---: | :--- | :--- | :--- |
| Menu de contexto do evento (abrir no Trimmer, aninhar, propriedades, escopo de cena) | 33 | parcial | criar sub-projeto, abrir no Trimmer e propriedades; faltam cena e quadro congelado | P2 |
| Menu de contexto da faixa (pasta, duplicar, apagar, cores de esmaecimento) | 35 | parcial | inserir, apagar e mudar a ordem; faltam pasta e duplicar | P2 |
| Menu de arrastar com o botão direito e do explorador | 16 | previsto | roteiro (menus de contexto) | P2 |
| Menus do Trimmer (trilhas de áudio e vídeo, subclipe, captura de quadro) | 30 | parcial | marcas e adicionar feitos; subclipe e captura de quadro não | P2 |
| Qualidade e zoom da visualização | 31 | parcial | qualidade feita; zoom em passos fixos não | P3 |
| Opções dos escopos: histograma, vetorscópio, forma de onda, parada, ângulo | 25 | parcial | escopos feitos; opções de canal e ângulo não | P3 |
| Opções do medidor de loudness e do pico verdadeiro | 35 | parcial | metas, LUFS e pico verdadeiro feitos; escalas e retenção não | P3 |
| Menus de envelope e de ponto-chave (forma, definir valor, copiar e colar) | 47 | parcial | formas de ponto feitas; copiar e colar quadros-chave não | P2 |
| Entradas, saídas, canais e sintetizadores MIDI | 56 | sem plano | MIDI fora do escopo | - |
| Menus do console: pan surround, modelos de pan, envios antes ou depois do fader, entradas | 33 | previsto | roteiro (console e barramentos) | P2 |
| Menus de marcador, região e seleção (ir para, renomear, apagar) | 11 | feito | menu de marcadores e regiões da régua | - |
| Modelos de script | 3 | previsto | depende do scripting | P3 |
| Interruptores da barra de edição (agrupar, travar envelopes, crossfade, ferramentas) | 11 | parcial | agrupar, crossfade, snap, ripple, slip, slide e embaralhar; faltam travar envelopes, esticar e dividir-aparar | P2 |
| Menu da cadeia de efeitos: bypass, mover e remover efeito | 9 | feito | botões da aba de efeitos (bypass, subir, descer, remover) | - |
| Colunas e vistas da lista de mídia e do explorador (nome, tipo, taxas, tamanho, data, árvore, resumo) | 42 | previsto | roteiro: janela de mídia com lista, miniaturas, filtros e busca | P2 |
| Ações da mídia do projeto: proxy, trocar arquivo, remover, cenas, tempo, binário, propriedades do formato | 31 | previsto | roteiro: pastas de mídia, subclipes e proxies | P2 |
| Ações do explorador: abrir pasta, nova pasta, favoritos, tocar, copiar, mover, importar de câmera e CD | 19 | previsto | roteiro: explorador de arquivos | P2 |
| Cor do texto e do fundo de eventos e faixas | 4 | previsto | sem plano de detalhe | P3 |
| Sobreposições da visualização: grade e áreas seguras | 2 | feito | sobreposições desligadas por padrão (AGENTS.md 4.2) | - |
| Ver canais de cor da visualização (vermelho, verde, azul, alfa, cinza) | 7 | previsto | sem plano de detalhe | P3 |
| Escolher o canal de closed caption exibido | 4 | sem plano | legendas SRT e WebVTT cobrem o uso; closed caption de TV não | - |
| Vista dividida da visualização e metades selecionáveis | 3 | previsto | a cadeia de efeitos já existe; falta o botão | P3 |
| Itens genéricos repetidos (selecionar tudo, área de transferência) | 2 | feito | `edit.select_all` e a área de transferência de eventos | - |
| Áreas de correção de olho vermelho | 1 | sem plano | fora do escopo de edição | - |
| Grupos de faixas: agrupar, recolher, expandir, mudo e solo do grupo | 7 | previsto | grupos de eventos existem; de faixas não | P3 |
| Sincronizar o storyboard com a linha do tempo | 2 | sem plano | storyboard fora do escopo | - |
| Formato de tempo e posição da barra de status | 2 | parcial | formato SMPTE na barra | P3 |

## Como manter

1. Ao concluir uma função, mude o estado da linha em `scripts/parity_matrix.py` e rode o script (sem a varredura ele usa `docs/research/parity_matrix.json`).
2. Linhas `a classificar` são itens soltos de menus auxiliares: analisar com a varredura e os manuais e criar uma linha própria.
3. Esta matriz mede **função e fluxo de trabalho**. A aparência continua original (AGENTS.md §28).
