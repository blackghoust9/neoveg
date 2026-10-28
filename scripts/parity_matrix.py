#!/usr/bin/env python3
"""Matriz de paridade funcional do Neoveg com o editor de referência (biblioteca padrão apenas).

Cada comando da varredura local (`VegasScan/22_Functions.md`, seção da versão recente, 872 comandos
únicos) é atribuído a **uma** linha desta matriz. As linhas são escritas à mão, em palavras próprias, e
dizem o estado da função no Neoveg. O script só conta quantos comandos da referência cada linha cobre e
confere se algum ficou sem linha; o texto dos menus da varredura nunca é gravado na saída (AGENTS.md
§28 e §37).

    python scripts/parity_matrix.py                 # lê VegasScan/, grava a matriz e o JSON
    python scripts/parity_matrix.py --scan OUTRA/22_Functions.md
    python scripts/parity_matrix.py --check         # só confere cobertura (sai com 1 se sobrar comando)

Saídas: `docs_obsidian/ui_references/matriz_de_paridade.md` e `docs/research/parity_matrix.json`.
Sem a varredura (pasta ignorada pelo Git) o script lê os números do JSON já versionado e só regera o
Markdown, então a matriz pode ser atualizada por quem não tem a varredura.

Estados: feito, parcial, previsto (está no roteiro do README), sem_plano (fora do escopo, com motivo),
a_classificar (item solto de menu auxiliar que ainda não foi analisado).
Prioridade: P1 define "igual" no uso diário, P2 importante, P3 baixa ou opcional, "-" fora do escopo.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from collections import OrderedDict
from pathlib import Path
from typing import Dict, List, Tuple

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SCAN = ROOT / "VegasScan" / "22_Functions.md"
OUT_MD = ROOT / "docs_obsidian" / "ui_references" / "matriz_de_paridade.md"
OUT_JSON = ROOT / "docs" / "research" / "parity_matrix.json"

# id, área, função (em palavras próprias), padrão sobre o caminho normalizado "Menu > Item",
# estado, o que o Neoveg tem, prioridade
Row = Tuple[str, str, str, str, str, str, str]

A_ARQ, A_EDIT, A_INS, A_FER, A_OPC, A_EXI, A_AJU, A_AUX = (
    "Arquivo", "Editar", "Inserir", "Ferramentas", "Opções", "Exibir", "Ajuda", "Menus de contexto e auxiliares",
)

ROWS: List[Row] = [
    # ---------------------------------------------------------------- Arquivo
    ("arq.novo", A_ARQ, "Novo projeto, abrir, tela de boas-vindas e sair", r"^File > (New|Open|Welcome|Exit)", "feito", "`file.new`, `file.open`, tela de boas-vindas", "-"),
    ("arq.fechar", A_ARQ, "Fechar o projeto atual", r"^File > Close$", "parcial", "fecha pela aba; o comando `file.close` não tem ação", "P3"),
    ("arq.salvar", A_ARQ, "Salvar, salvar como, versão incremental, propriedades do projeto", r"^File > (Save|Incremental|Properties)", "feito", "`file.save`, `file.save_as`, `file.save_incremental`, `file.properties`", "-"),
    ("arq.renderizar", A_ARQ, "Assistente de filme e renderizar como", r"^File > (Make Movie|Render As)", "feito", "Arquivo > Renderizar como (MP4, MOV ProRes, WebM, WAV)", "-"),
    ("arq.render_tempo_real", A_ARQ, "Renderizar áudio em tempo real incluindo entradas externas", r"^File > Real-Time Render", "sem_plano", "depende de gravação multicanal em tempo real", "-"),
    ("arq.importar_midia", A_ARQ, "Importar e adicionar mídia", r"^File > (Import > Media\.\.\.|Add Media)", "feito", "Importar mídia (áudio, vídeo, imagem, sequência)", "-"),
    ("arq.importar_projeto", A_ARQ, "Importar mídia de outro projeto", r"^File > Import > Media from Project", "previsto", "sub-projetos existem; importar só a mídia não", "P3"),
    ("arq.edl", A_ARQ, "Importar e exportar lista de edição (EDL)", r"^File > (Import|Export) > EDL", "feito", "`neoveg_engine::edl` (CMX 3600)", "-"),
    ("arq.xml", A_ARQ, "Importar e exportar XML de edição e FCPXML", r"^File > (Import|Export) > Final Cut", "feito", "`neoveg_engine::fcp7` e `fcpxml`", "-"),
    ("arq.aaf", A_ARQ, "Importar e exportar AAF", r"^File > (Import|Export) > .*AAF", "previsto", "roteiro: primeiro formato profissional a avaliar", "P2"),
    ("arq.prproj", A_ARQ, "Importar e exportar projeto de outro editor (formato fechado)", r"^File > (Import|Export) > .*prproj", "sem_plano", "formato proprietário; a ponte são XML, FCPXML, EDL, OTIO e AAF", "-"),
    ("arq.arquivo_proprio", A_ARQ, "Exportar o projeto no formato fechado do editor de referência", r"^File > Export > .*Project Archive", "sem_plano", "o Neoveg usa `.neoveg` (ZIP aberto)", "-"),
    ("arq.legendas", A_ARQ, "Importar legendas e closed caption", r"^File > Import > Closed Captioning", "parcial", "SRT e WebVTT feitos; closed caption de TV não", "P3"),
    ("arq.formatos_disco", A_ARQ, "Importar Broadcast Wave e disco de câmera, exportar para disco profissional e console", r"^File > (Import > (Broadcast|DVD)|Import DVD|Export > (XDCAM|PSP))", "sem_plano", "Broadcast Wave abre como WAV; mídia de disco de câmera e console ficam fora", "-"),
    ("arq.captura", A_ARQ, "Captura de vídeo e extração de faixas de CD de áudio", r"^File > (.*Capture|Extract Audio)", "sem_plano", "captura e CD ficam fora do escopo atual", "-"),
    ("arq.servico_online", A_ARQ, "Preparar e gerenciar clipes de um serviço online do fabricante", r"^File > (Prepare|Manage)", "sem_plano", "serviço fechado do fabricante", "-"),
    # ---------------------------------------------------------------- Editar
    ("edit.desfazer", A_EDIT, "Desfazer e refazer", r"^Edit > (Undo|Redo)$", "feito", "`edit.undo`, `edit.redo` (um gesto, um passo)", "-"),
    ("edit.historico", A_EDIT, "Desfazer tudo e limpar o histórico", r"^Edit > (Undo All|Clear Edit)", "previsto", "o histórico existe; faltam os dois comandos", "P3"),
    ("edit.area_transf", A_EDIT, "Recortar, copiar, colar, colar inserindo e colar repetido", r"^Edit > (Cut|Copy|Paste|Paste Repeat\.\.\.|Paste Insert)$", "feito", "`edit.cut`, `edit.copy`, `edit.paste`, `edit.paste_insert`, `edit.paste_repeat`", "-"),
    ("edit.colar_atributos", A_EDIT, "Colar atributos do evento, completo e seletivo", r"^Edit > (Selectively )?Paste Event Attributes", "previsto", "roteiro (README)", "P2"),
    ("edit.excluir", A_EDIT, "Excluir eventos ou faixas", r"^Edit > Delete$", "feito", "`edit.delete` e `edit.ripple_delete`", "-"),
    ("edit.aparar", A_EDIT, "Aparar à seleção, início ou fim ao cursor", r"^Edit > Trim", "feito", "`edit.trim`, `edit.trim_start`, `edit.trim_end`", "-"),
    ("edit.dividir", A_EDIT, "Dividir no cursor", r"^Edit > Split$", "feito", "`edit.split`", "-"),
    ("edit.edicao_assist", A_EDIT, "Divisão inteligente, curar evento dividido e fechar lacunas", r"^Edit > (Smart Split|Event Heal|Close Gaps)", "previsto", "roteiro (README)", "P2"),
    ("edit.ajuste", A_EDIT, "Redimensionar eventos de ajuste ao projeto", r"^Edit > Resize", "previsto", "eventos de ajuste são `previsto`", "P2"),
    ("edit.navegar", A_EDIT, "Navegar: ir ao tempo, início e fim, quadro, marcador, ponto de edição, centralizar", r"^Edit > Navigate", "parcial", "a maioria feita (`cursor.*`); faltam passo por grade, pixel, evento selecionado e cursor para a reprodução", "P3"),
    ("edit.ripple_pos", A_EDIT, "Escopo do ripple pós-edição", r"^Edit > Post-Edit Ripple", "parcial", "3 dos 4 escopos (`edit.ripple.*`); sem faixas de barramento", "P3"),
    ("edit.selecao", A_EDIT, "Selecionar tudo, depois do cursor e bordas do evento", r"^Edit > Select > (Select All|Select Event)", "parcial", "tudo e bordas (aparo por teclado) feitos; depois do cursor não", "P3"),
    ("edit.selecao_tempo", A_EDIT, "Seleção de tempo: laço, zoom, vista, projeto, início e fim, restaurar", r"^Edit > Select > (Loop|Zoom|Set|Restore|Select Loop)", "parcial", "entrada, saída e região de laço feitas; as demais não", "P3"),
    ("edit.expandida", A_EDIT, "Modo de edição expandida (entrar e sair)", r"^Edit > Select > (Enter Expanded|Exit Edit)", "previsto", "roteiro (README)", "P2"),
    ("edit.ferramentas", A_EDIT, "Ferramentas de edição: normal, seleção, envelope, zoom, próxima e anterior", r"^Edit > Editing Tool", "parcial", "normal, slip, slide e embaralhar com ciclo (`D`); faltam esticar e dividir-aparar", "P2"),
    ("edit.interruptores", A_EDIT, "Interruptores do evento: mudo, trava, fase, laço, normalizar", r"^Edit > Switches > (Mute|Lock|Invert Phase|Normalize|Loop event)", "feito", "`edit.switch.*`", "-"),
    ("edit.interruptores_video", A_EDIT, "Interruptores de vídeo: último quadro, mistura de quadros, fluxo óptico, reamostragem, proporção", r"^Edit > Switches", "previsto", "roteiro (README)", "P2"),
    ("edit.takes", A_EDIT, "Takes do evento: renomear, escolher, próximo, anterior, apagar", r"^Edit > Take", "previsto", "`edit.take.*` têm atalho, sem ação", "P2"),
    ("edit.grupos", A_EDIT, "Grupos: criar, remover, limpar, selecionar, recortar, copiar e apagar tudo", r"^Edit > Group", "parcial", "criar, remover, desagrupar e selecionar feitos (`edit.group`, `edit.ungroup`)", "P3"),
    ("edit.fluxo_canais", A_EDIT, "Escolha do fluxo de origem e dos canais (ambos, esquerdo, direito, combinar, trocar)", r"^Edit > (Stream|Channels)", "parcial", "canais feitos (`ChannelMode`); fluxo de origem não", "P3"),
    ("edit.extensoes", A_EDIT, "Menu de extensões", r"^Edit > Extensions", "feito", "gerenciador de extensões (Exibir)", "-"),
    # ---------------------------------------------------------------- Inserir
    ("ins.env_audio", A_INS, "Envelopes de áudio: volume e pan", r"^Insert > Audio Envelopes > (Volume|Pan)$", "feito", "`track.volume_envelope`, `track.pan_envelope`", "-"),
    ("ins.env_audio_mudo", A_INS, "Envelope de mudo de áudio", r"^Insert > Audio Envelopes > Mute", "previsto", "volume e pan feitos; mudo não", "P3"),
    ("ins.ducking", A_INS, "Auto-ducking: controlador e ouvinte", r"^Insert > Audio Envelopes > Auto Ducking", "feito", "auto-ducking por faixa (`neoveg_audio::duck`)", "-"),
    ("ins.env_video", A_INS, "Envelopes de faixa de vídeo: mudo, nível de composição e esmaecer para cor", r"^Insert > Video Envelopes > (Track Mute|Track Composite Level|Track Fade to Color)", "feito", "Faixas > Envelopes de vídeo e `V` na faixa de vídeo; vale na prévia e na renderização", "-"),
    ("ins.env_video_evento", A_INS, "Envelopes de evento de vídeo: velocidade e progresso de transição", r"^Insert > Video Envelopes", "previsto", "velocidade pede remapeamento de tempo da fonte; progresso pede as transições de vídeo", "P1"),
    ("ins.faixas", A_INS, "Inserir faixa de áudio e de vídeo", r"^Insert > (Audio Track|Video Track)$", "feito", "`insert.audio_track`, `insert.video_track`", "-"),
    ("ins.faixa_ajuste", A_INS, "Inserir faixa de ajuste", r"^Insert > Adjustment Track", "previsto", "roteiro (eventos de ajuste)", "P2"),
    ("ins.barramentos", A_INS, "Inserir barramento de áudio, de entrada e efeito atribuível", r"^Insert > (Audio Bus|Audio Input Bus|Audio Assignable)", "previsto", "roteiro (barramentos e envios)", "P2"),
    ("ins.legendas", A_INS, "Inserir legendas de arquivo e de regiões", r"^Insert > Insert Subtitles", "parcial", "de arquivo (SRT, WebVTT) feito; de regiões não", "P3"),
    ("ins.evento_vazio", A_INS, "Inserir fluxo ausente, evento vazio e evento de ajuste", r"^Insert > (Add Missing|Empty Event|Adjustment Event)", "previsto", "roteiro (README)", "P2"),
    ("ins.gerada", A_INS, "Inserir texto, mídia gerada e apresentação de slides", r"^Insert > (Text|Generated Media|Slideshow)", "feito", "Faixas > Inserir mídia gerada, janela de slideshow", "-"),
    ("ins.transicao_audio", A_INS, "Inserir transição de áudio", r"^Insert > Audio Transition", "parcial", "crossfade automático e curvas de fade; sem janela de transições", "P2"),
    ("ins.tempo_marc", A_INS, "Inserir tempo, marcador e região", r"^Insert > (Time|Marker|Region)", "feito", "`insert.time`, `insert.marker`, `insert.region`", "-"),
    ("ins.cd", A_INS, "Inserir faixa e índice de CD de áudio", r"^Insert > Audio CD", "sem_plano", "CD de áudio fora do escopo", "-"),
    ("ins.comando", A_INS, "Inserir marcador de comando", r"^Insert > Command", "previsto", "`insert.command` tem atalho, sem ação", "P3"),
    # ---------------------------------------------------------------- Ferramentas
    ("fer.editor_ext", A_FER, "Abrir o evento em editor de áudio externo", r"^(Tools > Audio > Open|Open in Audio Editor)", "sem_plano", "sem editor de áudio externo", "-"),
    ("fer.projeto_origem", A_FER, "Editar o projeto de origem de um evento aninhado", r"^Tools > Audio > Edit Source", "feito", "duplo clique no sub-projeto abre a aba", "-"),
    ("fer.fx_audio", A_FER, "Efeitos de áudio do evento e da faixa, aplicar fora de tempo real", r"^Tools > Audio > (Audio Event FX|Track FX|Remove Assignable|Apply Non)", "parcial", "faixa e master feitos (28 efeitos); efeitos por evento não", "P2"),
    ("fer.fx_video", A_FER, "Efeitos de vídeo da mídia, do evento, da faixa e da saída", r"^Tools > Video > (Media FX|Video Event FX|Video Track FX|Video Output FX)", "feito", "cadeias de mídia, evento, faixa e projeto (4 efeitos)", "-"),
    ("fer.pancrop", A_FER, "Pan/Crop do evento: caixa de seleção, zoom, movimento, rotação e quadros-chave", r"^Tools > Video > Video Event Pan", "parcial", "janela com caixa e alças, campos numéricos, quadros-chave com suavidade e prévia ao lado; faltam máscaras, centro de rotação e o botão no cabeçalho do evento", "P2"),
    ("fer.movimento", A_FER, "Movimento da trilha: posição, escala e rotação da camada com quadros-chave", r"^Tools > Video > Track Motion", "parcial", "janela com caixa, picture-in-picture, rotação 2D, suavidade e prévia ao lado; faltam 3D, sombra, brilho e movimento do pai de um grupo", "P2"),
    ("fer.cor_mov", A_FER, "Modo de gradação de cor e rastreamento de movimento", r"^Tools > Video > (Color Grading|Motion Tracking)", "parcial", "correção de cor primária e LUT feitas; gradação por evento e rastreamento não", "P2"),
    ("fer.debug_video", A_FER, "Ferramentas de depuração de efeitos, transições, geradores e compositores", r"^Tools > Video > DEBUG", "sem_plano", "uso interno do fabricante", "-"),
    ("fer.renderizar_faixa", A_FER, "Renderizar para nova faixa", r"^Tools > Render to New Track", "previsto", "`tools.render_to_new_track` tem atalho, sem ação", "P2"),
    ("fer.monitor_ext", A_FER, "Prévia em monitor externo", r"^Tools > Preview on External", "parcial", "janela destacada em outro monitor; sem saída de dispositivo", "P3"),
    ("fer.prerender", A_FER, "Pré-renderização seletiva e prévia dinâmica em RAM", r"^Tools > (Selectively Prerender|Clean Up Prerendered|Build Dynamic|Clear Dynamic)", "previsto", "roteiro: barra verde, amarela e vermelha", "P2"),
    ("fer.ia", A_FER, "Ferramentas de IA: fala para texto, texto para fala, reenquadramento, assistente", r"^Tools > AI", "previsto", "roteiro (fase de IA; Whisper)", "P3"),
    ("fer.fita_disco", A_FER, "Imprimir em fita, gravar disco e montar CD de áudio", r"^Tools > (Print Video|Lay Out Audio CD|Burn Disc)", "sem_plano", "mídia física fora do escopo", "-"),
    ("fer.loudness_log", A_FER, "Gerar registro de loudness", r"^Tools > Generate Loudness", "previsto", "o medidor existe; o registro em arquivo não", "P3"),
    ("fer.hardware", A_FER, "Religar mídia de adaptador sem fio do fabricante", r"^Tools > Relink", "sem_plano", "hardware proprietário", "-"),
    ("fer.salvar_av", A_FER, "Salvar avançado", r"^Tools > Advanced Save", "previsto", "sem plano de detalhe", "P3"),
    ("fer.scripts", A_FER, "Executar scripts e reexaminar a pasta de scripts", r"^Tools > Scripting", "previsto", "roteiro: scripting em Rust", "P3"),
    ("fer.extensoes", A_FER, "Menu de extensões", r"^Tools > Extensions", "feito", "gerenciador de extensões", "-"),
    ("fer.multicam", A_FER, "Multicâmera: sincronizar, criar, expandir e editar", r"^Tools > Multicamera", "previsto", "roteiro (README)", "P3"),
    ("fer.limpeza", A_FER, "Limpar mídia do projeto e reconstruir áudio aninhado", r"^Tools > (Clean Project|Rebuild Offline)", "previsto", "relink existe; a limpeza não", "P3"),
    # ---------------------------------------------------------------- Opções
    ("opc.quantizar", A_OPC, "Quantizar para quadros e mostrar bordas não quantizadas", r"^Options > Quantize", "parcial", "quantizar feito; mostrar bordas não", "P3"),
    ("opc.snap", A_OPC, "Alinhar à grade, a marcadores e a todos os eventos", r"^Options > Snap", "feito", "`options.snap_*`", "-"),
    ("opc.grade", A_OPC, "Espaçamento da grade (marcas da régua, tempo, compassos, notas)", r"^Options > Grid Spacing", "previsto", "a grade é automática; escolher o passo não", "P3"),
    ("opc.regua", A_OPC, "Formato da régua (amostras, tempo, quadros, compassos, filme, SMPTE)", r"^(Options > Ruler Format|Time at Cursor)", "previsto", "roteiro (fase 12); hoje SMPTE com e sem drop", "P2"),
    ("opc.quickfade", A_OPC, "Fade rápido nas edições de áudio", r"^Options > Quickfade", "previsto", "roteiro (preferência `quick_fade_ms`)", "P3"),
    ("opc.ripple", A_OPC, "Ripple automático e escopo do ripple", r"^Options > (Auto Ripple|Ripple)", "feito", "`options.auto_ripple`, `edit.ripple.*`", "-"),
    ("opc.slip_takes", A_OPC, "Deslizar todos os takes", r"^Options > Slip All Takes", "previsto", "depende dos takes", "P3"),
    ("opc.metronomo", A_OPC, "Metrônomo", r"^Options > Metronome", "feito", "Ferramentas > Metrônomo", "-"),
    ("opc.mudo_geral", A_OPC, "Ignorar todos os efeitos de áudio e silenciar todo áudio ou vídeo", r"^Options > (Bypass All Audio|Mute All)", "previsto", "sem plano de detalhe", "P3"),
    ("opc.timecode_midi", A_OPC, "Timecode e relógio MIDI", r"^Options > Timecode", "sem_plano", "MIDI fora do escopo", "-"),
    ("opc.barras", A_OPC, "Personalizar a barra de ferramentas e a da linha do tempo", r"^Options > Customize (Toolbar|Timeline)", "previsto", "sem plano de detalhe", "P3"),
    ("opc.teclado", A_OPC, "Personalizar o teclado", r"^Options > Customize Keyboard", "feito", "Editar > Personalizar teclado", "-"),
    ("opc.preferencias", A_OPC, "Preferências e exportar ou importar preferências", r"^Options > (Preferences|Export Preferences|Import Preferences)", "parcial", "preferências feitas; exportar e importar não", "P3"),
    # ---------------------------------------------------------------- Exibir
    ("exi.barras", A_EXI, "Barra de ferramentas e barra de status", r"^View > (Toolbar|Status Bar)", "feito", "barra e status", "-"),
    ("exi.layouts", A_EXI, "Layouts de janela: organizar, salvar, recarregar, padrão e os dez salvos", r"^View > Window Layouts", "parcial", "layouts 1 a 9 e padrão na sessão; sem disco nem organizar", "P2"),
    ("exi.foco", A_EXI, "Foco na linha do tempo", r"^View > Focus", "feito", "`view.focus_timeline`", "-"),
    ("exi.janelas_ok", A_EXI, "Janelas: Trimmer, master, visualização, mídia, notas, escopos, detalhes de edição, loudness", r"^View > Window > (Trimmer|Master Bus|Video Preview|Project Media|Project Notes|Video Scopes|Edit Details|Loudness)", "feito", "`view.*` e janelas do cofre", "-"),
    ("exi.janela_fx", A_EXI, "Janelas de efeitos de vídeo e de geradores", r"^View > Window > (Video FX|Media Generators)", "feito", "aba Efeitos de Vídeo; diálogo de geradores", "-"),
    ("exi.explorador", A_EXI, "Janela do explorador de arquivos", r"^View > Window > (Explorer|Legacy Explorer)", "previsto", "roteiro (Explorer)", "P2"),
    ("exi.janelas_faltam", A_EXI, "Janelas de transições, compositores e gerenciador de plug-ins", r"^View > Window > (Transitions|Compositors|Plug-In Manager)", "previsto", "roteiro (README)", "P2"),
    ("exi.janelas_fora", A_EXI, "Janelas de pan surround, explorador de dispositivos, disco profissional e hub online", r"^View > Window > (Surround|Device|XDCAM|.*Hub)", "sem_plano", "hardware e serviço do fabricante", "-"),
    ("exi.mixer", A_EXI, "Fader de prévia do mixer e trilhas de barramento", r"^View > (Mixer Preview|Audio Bus|Video Bus)", "previsto", "roteiro (barramentos)", "P2"),
    ("exi.evento", A_EXI, "Interruptores de exibição do evento: marcadores, take, ondas, botões, fades, bordas, cabeçalho, comprimento", r"^View > (Event|Video Event Edge|Active Take|Audio Event Waveforms)", "previsto", "roteiro: atalhos de visualização", "P2"),
    ("exi.envelopes", A_EXI, "Mostrar envelopes de áudio e de vídeo", r"^View > (Audio Envelopes|Video Envelopes|Show Envelopes)", "parcial", "áudio e, no vídeo, nível, esmaecer e mudo feitos; velocidade, progresso, desfoque de movimento e supersampling não", "P2"),
    ("exi.trilhas", A_EXI, "Minimizar todas as trilhas e reconstruir picos de áudio", r"^View > (Minimize|Rebuild)", "previsto", "roteiro: cache de picos", "P3"),
    ("exi.extensoes", A_EXI, "Menu de extensões", r"^View > Extensions", "feito", "gerenciador de extensões", "-"),
    # ---------------------------------------------------------------- Ajuda
    ("aju.ajuda", A_AJU, "Conteúdo, atalhos, tutoriais, manual e comunidade", r"^Help > (Contents|What|Keyboard|Download|Show Me|Interactive|.*Community)", "parcial", "atalhos e créditos; sem manual embutido", "P3"),
    ("aju.sistema", A_AJU, "Atualizações, ativação, hardware e memória, rotinas de depuração", r"^Help", "sem_plano", "ativação e depuração do fabricante; atualização fica com o pacote", "-"),
    # ---------------------------------------------------------------- Menus de contexto e auxiliares
    ("aux.ctx_evento", A_AUX, "Menu de contexto do evento (abrir no Trimmer, aninhar, propriedades, escopo de cena)", r"^(POPUP_IDX_(AUDIO|VIDEO)_EVENT|POPUP_IDX_EVENT|Event > (Edit|Cut|Copy|Paste|Delete))", "parcial", "criar sub-projeto, abrir no Trimmer e propriedades; faltam cena e quadro congelado", "P2"),
    ("aux.ctx_faixa", A_AUX, "Menu de contexto da faixa (pasta, duplicar, apagar, cores de esmaecimento)", r"^POPUP_IDX_(AUDIO|VIDEO|BUS|VIDEOBUS|HEADER)", "parcial", "inserir, apagar e mudar a ordem; faltam pasta e duplicar", "P2"),
    ("aux.ctx_explorador", A_AUX, "Menu de arrastar com o botão direito e do explorador", r"^POPUP_IDX_(EXPLORER|PEXPLORER|DEFAULT|VIDEO_SURFACE)", "previsto", "roteiro (menus de contexto)", "P2"),
    ("aux.trimmer", A_AUX, "Menus do Trimmer (trilhas de áudio e vídeo, subclipe, captura de quadro)", r"^Trimmer", "parcial", "marcas e adicionar feitos; subclipe e captura de quadro não", "P2"),
    ("aux.qualidade", A_AUX, "Qualidade e zoom da visualização", r"^(Preview Quality|Preview Zoom|Preview Start)", "parcial", "qualidade feita; zoom em passos fixos não", "P3"),
    ("aux.escopos", A_AUX, "Opções dos escopos: histograma, vetorscópio, forma de onda, parada, ângulo", r"^(Histogram|Vectorscope|Waveform|Parade|View Angle|Histogram Settings)", "parcial", "escopos feitos; opções de canal e ângulo não", "P3"),
    ("aux.loudness", A_AUX, "Opções do medidor de loudness e do pico verdadeiro", r"^(Meter|True Peak|Loudness Scale|Reset|EBU|ATSC|Show True|Show Labels|Hold|Label in|Open in Trimmer)", "parcial", "metas, LUFS e pico verdadeiro feitos; escalas e retenção não", "P3"),
    ("aux.envelope_menu", A_AUX, "Menus de envelope e de ponto-chave (forma, definir valor, copiar e colar)", r"^(EnvelopeLine|EnvelopePoint|KeyframePoint|Keyframe|Keyframes|KeyframeBypass|KeyframeInRow|Automation Modes|Show Automation|Automation )", "parcial", "formas de ponto feitas; copiar e colar quadros-chave não", "P2"),
    ("aux.midi", A_AUX, "Entradas, saídas, canais e sintetizadores MIDI", r"^(MIDI|SoftSynth)", "sem_plano", "MIDI fora do escopo", "-"),
    ("aux.mixer_menu", A_AUX, "Menus do console: pan surround, modelos de pan, envios antes ou depois do fader, entradas", r"^(Surround Pan|Pan Models|Bus PrePost|BUSFX|Audio Input|Global|Pre Volume|Post Volume|Link to Main Track Pan|Switches)", "previsto", "roteiro (console e barramentos)", "P2"),
    ("aux.marcadores", A_AUX, "Menus de marcador, região e seleção (ir para, renomear, apagar)", r"^(Marker|Region Start|General|Command Marker)", "feito", "menu de marcadores e regiões da régua", "-"),
    ("aux.scripts_modelos", A_AUX, "Modelos de script", r"^Template", "previsto", "depende do scripting", "P3"),
    ("aux.barra_edicao", A_AUX, "Interruptores da barra de edição (agrupar, travar envelopes, crossfade, ferramentas)", r"^(Ignore Event|Lock Envelopes|Auto Ripple|Automatic Crossfades|Enable Snapping|Normal Edit|Shuffle|Slip|Slide|Time Stretch|Split Trim)", "parcial", "agrupar, crossfade, snap, ripple, slip, slide e embaralhar; faltam travar envelopes, esticar e dividir-aparar", "P2"),
    ("aux.fx_cadeia", A_AUX, "Menu da cadeia de efeitos: bypass, mover e remover efeito", r"^(Bypass|Move Left|Move Right|Remove)$|^FX", "feito", "botões da aba de efeitos (bypass, subir, descer, remover)", "-"),
    ("aux.midia_colunas", A_AUX, "Colunas e vistas da lista de mídia e do explorador (nome, tipo, taxas, tamanho, data, árvore, resumo)", r"^View > (Name|Use Count|Type|Media FX|Comment|Title|Timecode|Length|Tape|Size|Modified|Path|Avg|Sample|Bit|Channels|Audio|Frame|Field|Alpha|Pixel|Color Depth|Video|Date|Rating|UMID|Grid|Tree|Region|Summary|Details|All Files)|^(List|Details|Thumbnail|Media Tags|Auto-Size Columns)$", "previsto", "roteiro: janela de mídia com lista, miniaturas, filtros e busca", "P2"),
    ("aux.midia_acoes", A_AUX, "Ações da mídia do projeto: proxy, trocar arquivo, remover, cenas, tempo, binário, propriedades do formato", r"^(Open Parent Media|Detect Scenes|Tempo Detection|Select Timeline Events|Match Project Video|Create Video Proxy|Swap Video|Remove from|Search Media Bins|Replace\.\.\.|Media FX\.\.\.|Color Grading|Rotate 90|Pair as Stereo|Create New Bin|Save as|Edit Quick Tags|File Format|Add as CD|Rename$|Delete$|Properties)", "previsto", "roteiro: pastas de mídia, subclipes e proxies", "P2"),
    ("aux.explorador_acoes", A_AUX, "Ações do explorador: abrir pasta, nova pasta, favoritos, tocar, copiar, mover, importar de câmera e CD", r"^(Explore |Refresh|Add to Project Media|Add Media to Timeline|Play Media|Open Media in Player|Add Folder|New Folder|Add Media\.\.\.|Import files|Import DVD|Extract Audio|Copy Here|Move Here|Create Shortcut|Cancel)", "previsto", "roteiro: explorador de arquivos", "P2"),
    ("aux.cores_texto", A_AUX, "Cor do texto e do fundo de eventos e faixas", r"^(Text Color|Background Color)", "previsto", "sem plano de detalhe", "P3"),
    ("aux.sobreposicoes", A_AUX, "Sobreposições da visualização: grade e áreas seguras", r"^(Grid|Safe Areas)$", "feito", "sobreposições desligadas por padrão (AGENTS.md 4.2)", "-"),
    ("aux.canais_cor", A_AUX, "Ver canais de cor da visualização (vermelho, verde, azul, alfa, cinza)", r"^(Red|Green|Blue)( as Grayscale)?$|^Alpha as Grayscale", "previsto", "sem plano de detalhe", "P3"),
    ("aux.closed_caption", A_AUX, "Escolher o canal de closed caption exibido", r"^Closed Captioning CC", "sem_plano", "legendas SRT e WebVTT cobrem o uso; closed caption de TV não", "-"),
    ("aux.vista_dividida", A_AUX, "Vista dividida da visualização e metades selecionáveis", r"^(Retain Split|Select Left Half|Select Right Half)", "previsto", "a cadeia de efeitos já existe; falta o botão", "P3"),
    ("aux.genericos", A_AUX, "Itens genéricos repetidos (selecionar tudo, área de transferência)", r"^(Clipboard|Select All)$", "feito", "`edit.select_all` e a área de transferência de eventos", "-"),
    ("aux.olho_vermelho", A_AUX, "Áreas de correção de olho vermelho", r"^Red Eye", "sem_plano", "fora do escopo de edição", "-"),
    ("aux.grupos_faixa", A_AUX, "Grupos de faixas: agrupar, recolher, expandir, mudo e solo do grupo", r"^(Rename Track Group|Group Selected Tracks|Ungroup Selected|Collapse|Expand|Mute Track Group|Solo Track Group)", "previsto", "grupos de eventos existem; de faixas não", "P3"),
    ("aux.storyboard", A_AUX, "Sincronizar o storyboard com a linha do tempo", r"storyboard", "sem_plano", "storyboard fora do escopo", "-"),
    ("aux.formato_tempo", A_AUX, "Formato de tempo e posição da barra de status", r"^(Time Format|Event$|Preview$)", "parcial", "formato SMPTE na barra", "P3"),
]

ESTADOS = ["feito", "parcial", "previsto", "sem_plano", "a_classificar"]
ROTULO = {"feito": "feito", "parcial": "parcial", "previsto": "previsto", "sem_plano": "sem plano", "a_classificar": "a classificar"}
FALLBACK: Row = ("aux.solto", A_AUX, "Itens soltos de menus auxiliares ainda não analisados", r".*", "a_classificar", "ver a lista local da varredura", "P3")


def normalize(path: str) -> str:
    text = path.replace("›", ">").replace("�", ">")
    text = re.sub(r"\s*>\s*", " > ", text).strip()
    return text.strip("> ").strip()


def read_scan(scan: Path) -> List[Tuple[str, str]]:
    text = scan.read_text(encoding="utf-8")
    start = text.index("## Comandos por menu — VEGAS Pro 22")
    end = text.index("## Comandos só em", start)
    seen: Dict[str, str] = OrderedDict()
    for line in text[start:end].splitlines():
        match = re.match(r"\| (\d+) \| (.*?) \| (\S+) \|", line)
        if match and match.group(1) not in seen:
            path = normalize(match.group(2))
            seen[match.group(1)] = path if path else "(sem caminho)"
    return list(seen.items())


def classify(commands: List[Tuple[str, str]]) -> Tuple[Dict[str, int], int]:
    compiled = [(row, re.compile(row[3])) for row in ROWS]
    fallback = (FALLBACK, re.compile(FALLBACK[3]))
    counts: Dict[str, int] = {row[0]: 0 for row in ROWS}
    counts[FALLBACK[0]] = 0
    for _, path in commands:
        for row, pattern in compiled + [fallback]:
            if pattern.search(path):
                counts[row[0]] += 1
                break
    return counts, counts[FALLBACK[0]]


def score(rows: List[Row], counts: Dict[str, int]) -> Dict[str, float]:
    total = sum(counts.get(r[0], 0) for r in rows)
    scope = sum(counts.get(r[0], 0) for r in rows if r[4] != "sem_plano")
    done = sum(counts.get(r[0], 0) * {"feito": 1.0, "parcial": 0.5}.get(r[4], 0.0) for r in rows)
    return {"total": total, "scope": scope, "done": done, "pct": (100.0 * done / scope) if scope else 0.0}


def render(counts: Dict[str, int]) -> str:
    rows = ROWS + ([FALLBACK] if counts.get(FALLBACK[0]) else [])
    areas: "OrderedDict[str, List[Row]]" = OrderedDict()
    for row in rows:
        areas.setdefault(row[1], []).append(row)
    out: List[str] = []
    out.append("---\ntags: [neoveg, referencia, ui, paridade, roteiro]\n---")
    out.append("# Matriz de paridade funcional\n")
    out.append("Gerada por `scripts/parity_matrix.py`. Hub: [[fontes_de_referencia_de_ui]]. Voltar: [[Index]].\n")
    out.append(
        "Cada comando da varredura da versão recente do editor de referência (872 comandos únicos) pertence a **uma** linha "
        "abaixo. As linhas descrevem a função em palavras próprias, sem copiar o texto dos menus (AGENTS.md §28 e §37). "
        "A coluna *Comandos* diz quantos comandos da referência a linha cobre; a coluna *No Neoveg* diz o que existe hoje.\n"
    )
    out.append("**Estados:** `feito`, `parcial`, `previsto` (está no roteiro do README), `sem plano` (fora do escopo, com motivo) e `a classificar`.  ")
    out.append("**Prioridade:** P1 define \"igual\" no uso diário, P2 é importante, P3 é baixa ou opcional.\n")
    # resumo
    out.append("## Resumo por área\n")
    out.append("| Área | Comandos | No escopo | Feito | Parcial | Previsto | Sem plano | Cobertura* |")
    out.append("| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    grand = {"total": 0, "scope": 0, "done": 0.0}
    for area, items in areas.items():
        by = {e: sum(counts.get(r[0], 0) for r in items if r[4] == e) for e in ESTADOS}
        s = score(items, counts)
        grand["total"] += s["total"]; grand["scope"] += s["scope"]; grand["done"] += s["done"]
        out.append(f"| {area} | {s['total']} | {s['scope']} | {by['feito']} | {by['parcial']} | {by['previsto']} | {by['sem_plano']} | {s['pct']:.0f} % |")
    pct = 100.0 * grand["done"] / grand["scope"] if grand["scope"] else 0.0
    out.append(f"| **Total** | **{grand['total']}** | **{grand['scope']}** | | | | | **{pct:.0f} %** |\n")
    by_row = {e: sum(1 for r in rows if r[4] == e) for e in ESTADOS}
    in_scope_rows = len(rows) - by_row["sem_plano"]
    out.append(
        f"Por **linha da matriz** (cada linha é uma função, sem o peso do número de comandos): "
        f"{by_row['feito']} feitas, {by_row['parcial']} parciais, {by_row['previsto']} previstas e "
        f"{by_row['sem_plano']} sem plano, de {len(rows)} linhas ({in_scope_rows} no escopo).\n"
    )
    out.append("\\* Cobertura = (feito + metade do parcial) sobre os comandos no escopo, isto é, sem contar os de `sem plano`. É uma medida por **comando da referência**, não por esforço: um comando de menu pode esconder uma função enorme (por exemplo, o Pan/Crop).\n")
    # prioridades
    out.append("## Onde está o trabalho (P1 e P2 abertos)\n")
    out.append("| Prioridade | Função | Área | Comandos | Estado |")
    out.append("| :--- | :--- | :--- | ---: | :--- |")
    for prio in ("P1", "P2"):
        for row in rows:
            if row[6] == prio and row[4] in ("previsto", "parcial"):
                out.append(f"| {prio} | {row[2]} | {row[1]} | {counts.get(row[0], 0)} | {ROTULO[row[4]]} |")
    out.append("")
    # detalhe
    out.append("## Detalhe por área\n")
    for area, items in areas.items():
        out.append(f"### {area}\n")
        out.append("| Função | Comandos | Estado | No Neoveg | Prio. |")
        out.append("| :--- | ---: | :--- | :--- | :--- |")
        for row in items:
            out.append(f"| {row[2]} | {counts.get(row[0], 0)} | {ROTULO[row[4]]} | {row[5]} | {row[6]} |")
        out.append("")
    out.append("## Como manter\n")
    out.append("1. Ao concluir uma função, mude o estado da linha em `scripts/parity_matrix.py` e rode o script (sem a varredura ele usa `docs/research/parity_matrix.json`).")
    out.append("2. Linhas `a classificar` são itens soltos de menus auxiliares: analisar com a varredura e os manuais e criar uma linha própria.")
    out.append("3. Esta matriz mede **função e fluxo de trabalho**. A aparência continua original (AGENTS.md §28).")
    return "\n".join(out) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--scan", type=Path, default=DEFAULT_SCAN)
    parser.add_argument("--check", action="store_true", help="só confere a cobertura")
    args = parser.parse_args()

    counts: Dict[str, int]
    if args.scan.is_file():
        commands = read_scan(args.scan)
        counts, loose = classify(commands)
        print(f"{len(commands)} comandos da varredura; {loose} sem linha própria (a classificar)")
        if args.check:
            return 0 if loose == 0 else 1
        OUT_JSON.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "fonte": "varredura local da versão recente do editor de referência",
            "comandos": len(commands),
            "linhas": [
                {"id": r[0], "area": r[1], "estado": r[4], "prioridade": r[6], "comandos": counts.get(r[0], 0)}
                for r in ROWS + [FALLBACK]
            ],
        }
        OUT_JSON.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    elif OUT_JSON.is_file():
        print(f"varredura ausente ({args.scan}); usando os números de {OUT_JSON.name}")
        data = json.loads(OUT_JSON.read_text(encoding="utf-8"))
        counts = {item["id"]: item["comandos"] for item in data["linhas"]}
        if args.check:
            return 0
    else:
        print("nem a varredura nem o JSON existem", file=sys.stderr)
        return 1
    OUT_MD.parent.mkdir(parents=True, exist_ok=True)
    OUT_MD.write_text(render(counts), encoding="utf-8")
    print(f"gravado {OUT_MD.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
