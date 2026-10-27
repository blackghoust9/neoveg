---
tags: [neoveg, manual, atalhos]
---
# Atalhos de teclado

Os atalhos são comandos; qualquer um pode ser reatribuído em **Editar → Personalizar teclado…** (guardado em `preferences.json`). Resumo do uso diário em [[guia_de_inicio]].

<!-- auto:begin -->
Gerado de `commands.rs` e `keymap.rs` (107 comandos, 105 com atalho padrão).
Itens marcados como *planejado* já têm atalho reservado, mas a função ainda não existe.

### Arquivo

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Abrir projeto… | `Ctrl+O` | `file.open` | implementado |
| Salvar | `Ctrl+S` | `file.save` | implementado |
| Salvar como… | `Ctrl+Shift+S` | `file.save_as` | implementado |
| Salvar versão (incremental) | `Ctrl+Alt+S` | `file.save_incremental` | implementado |
| Novo projeto (em uma nova aba) | `Ctrl+N` | `file.new` | implementado |
| Criar sub-projeto a partir da seleção | `Ctrl+Shift+N` | `file.create_subproject` | implementado |
| Novo projeto vazio (sem diálogo) | — | `file.new_quick` | *planejado* |
| Fechar projeto | `Ctrl+F4` | `file.close` | *planejado* |
| Propriedades do projeto… | `Alt+Enter` | `file.properties` | implementado |

### Editar

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Desfazer | `Ctrl+Z` / `Alt+Backspace` | `edit.undo` | implementado |
| Refazer | `Ctrl+Y` / `Ctrl+Shift+Z` | `edit.redo` | implementado |
| Excluir seleção | `Delete` | `edit.delete` | implementado |
| Excluir e fechar o espaço (ripple) | `Ctrl+Delete` | `edit.ripple_delete` | implementado |
| Dividir no cursor | `S` | `edit.split` | implementado |
| Recortar | `Ctrl+X` / `Shift+Delete` | `edit.cut` | implementado |
| Copiar | `Ctrl+C` / `Ctrl+Insert` | `edit.copy` | implementado |
| Colar | `Ctrl+V` / `Shift+Insert` | `edit.paste` | implementado |
| Colar inserindo | `Ctrl+Shift+V` | `edit.paste_insert` | implementado |
| Repetir colar… | `Ctrl+B` | `edit.paste_repeat` | implementado |
| Aparar à seleção | `Ctrl+T` | `edit.trim` | implementado |
| Aparar o início ao cursor | `Alt+[` | `edit.trim_start` | implementado |
| Aparar o fim ao cursor | — | `edit.trim_end` | implementado |
| Selecionar tudo | `Ctrl+A` | `edit.select_all` | implementado |
| Desfazer seleção | `Ctrl+Shift+A` | `edit.select_none` | implementado |
| Agrupar eventos | `G` | `edit.group` | implementado |
| Desagrupar (dissolver o grupo inteiro) | `Ctrl+U` | `edit.ungroup` | implementado |
| Remover evento(s) do grupo | `U` | `edit.remove_from_group` | implementado |
| Selecionar todos os eventos do grupo | `Shift+G` | `edit.select_group` | implementado |
| Ignorar agrupamento | `Ctrl+Shift+U` | `edit.ignore_grouping` | implementado |
| Interruptor: silenciar evento | `Ctrl+Alt+M` | `edit.switch.mute` | implementado |
| Interruptor: bloquear evento | `Ctrl+Alt+L` | `edit.switch.lock` | implementado |
| Interruptor: inverter fase | `Ctrl+Alt+I` | `edit.switch.invert_phase` | implementado |
| Interruptor: repetir (laço do evento) | `Ctrl+Alt+O` | `edit.switch.loop` | implementado |
| Interruptor: normalizar | `Ctrl+Alt+N` | `edit.switch.normalize` | implementado |
| Recalcular a normalização | `Ctrl+Alt+Shift+N` | `edit.normalize.recalculate` | implementado |
| Ferramenta normal | `Ctrl+D` | `edit.tool.normal` | *planejado* |
| Próxima ferramenta de edição | `D` | `edit.tool.next` | *planejado* |
| Ferramenta de edição anterior | `Shift+D` | `edit.tool.previous` | *planejado* |
| Próximo take | `T` | `edit.take.next` | *planejado* |
| Take anterior | `Shift+T` | `edit.take.previous` | *planejado* |

### Inserir

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Inserir faixa de áudio | `Ctrl+Q` | `insert.audio_track` | implementado |
| Inserir faixa de vídeo | `Ctrl+Shift+Q` | `insert.video_track` | implementado |
| Inserir marcador | `M` | `insert.marker` | implementado |
| Inserir tempo… | `Ctrl+Alt+T` | `insert.time` | implementado |
| Inserir região | `R` | `insert.region` | implementado |
| Inserir marcador de comando… | `C` | `insert.command` | *planejado* |
| Renderizar para nova faixa | `Ctrl+M` | `tools.render_to_new_track` | *planejado* |

### Cursor e seleção

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Cursor um quadro à esquerda | `Left` / `Alt+Left` | `cursor.left_by_frame` | implementado |
| Cursor um quadro à direita | `Right` / `Alt+Right` | `cursor.right_by_frame` | implementado |
| Ir para o início do projeto | `Ctrl+Home` / `W` | `cursor.project_start` | implementado |
| Ir para o fim do projeto | `Ctrl+End` | `cursor.project_end` | implementado |
| Ir para o início da seleção ou da vista | `Home` | `cursor.selection_start` | implementado |
| Ir para o fim da seleção ou da vista | `End` | `cursor.selection_end` | implementado |
| Marcador anterior | `Ctrl+Left` | `cursor.previous_marker` | implementado |
| Próximo marcador | `Ctrl+Right` | `cursor.next_marker` | implementado |
| Ir para o marcador 1 | `1` | `cursor.marker_one` | implementado |
| Ir para o marcador 2 | `2` | `cursor.marker_two` | implementado |
| Ir para o marcador 3 | `3` | `cursor.marker_three` | implementado |
| Ir para o marcador 4 | `4` | `cursor.marker_four` | implementado |
| Ir para o marcador 5 | `5` | `cursor.marker_five` | implementado |
| Ir para o marcador 6 | `6` | `cursor.marker_six` | implementado |
| Ir para o marcador 7 | `7` | `cursor.marker_seven` | implementado |
| Ir para o marcador 8 | `8` | `cursor.marker_eight` | implementado |
| Ir para o marcador 9 | `9` | `cursor.marker_nine` | implementado |
| Ir para o marcador 10 | `0` | `cursor.marker_ten` | implementado |
| Ponto de edição anterior | `Ctrl+Alt+Left` | `cursor.previous_edit` | implementado |
| Próximo ponto de edição | `Ctrl+Alt+Right` | `cursor.next_edit` | implementado |
| Ir para o tempo… | `Ctrl+G` | `cursor.go_to` | implementado |
| Centralizar a vista no cursor | `\\` | `cursor.center_view` | implementado |
| Definir ponto de entrada | `I` | `selection.set_in` | implementado |
| Definir ponto de saída | `O` | `selection.set_out` | implementado |
| Selecionar a região de laço | `Shift+Q` | `selection.loop_region` | implementado |

### Reprodução

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Reproduzir / pausar | `Space` | `transport.play_toggle` | implementado |
| Parar | `Escape` | `transport.stop` | *planejado* |
| Reproduzir do início | `Shift+Space` | `transport.play_from_start` | *planejado* |
| Reprodução em laço | `Q` | `transport.loop` | implementado |
| Voltar (J; de novo acelera) | `J` | `transport.shuttle_reverse` | implementado |
| Pausar (K) | `K` | `transport.shuttle_pause` | implementado |
| Avançar (L; de novo acelera) | `L` | `transport.shuttle_forward` | implementado |
| Gravar | `Ctrl+R` | `transport.record` | implementado |

### Opções

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Alinhamento (snap) | `F8` | `options.snapping` | implementado |
| Crossfade automático | `Ctrl+Shift+X` | `options.auto_crossfade` | implementado |
| Alinhar à grade | `Ctrl+F8` | `options.snap_grid` | *planejado* |
| Alinhar a marcadores | `Shift+F8` | `options.snap_markers` | *planejado* |
| Alinhar a todos os eventos | `Ctrl+Shift+F8` | `options.snap_all_events` | *planejado* |
| Quantizar para quadros | `Alt+F8` | `options.quantize` | *planejado* |
| Ripple automático | `Ctrl+L` | `options.auto_ripple` | implementado |
| Ripple nas faixas afetadas | `F` | `edit.ripple.affected` | *planejado* |
| Ripple nas faixas, marcadores e regiões | `Ctrl+F` | `edit.ripple.markers` | *planejado* |
| Ripple em tudo | `Ctrl+Shift+F` | `edit.ripple.all` | *planejado* |

### Faixas

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Mudo na faixa | `Z` | `track.mute` | implementado |
| Foco na faixa de cima | `Alt+Shift+Up` | `track.focus_previous` | implementado |
| Foco na faixa de baixo | `Alt+Shift+Down` | `track.focus_next` | implementado |
| Mudo só nesta faixa | `Shift+Z` | `track.mute_exclusive` | implementado |
| Solo na faixa | `X` | `track.solo` | implementado |
| Solo só nesta faixa | `Shift+X` | `track.solo_exclusive` | implementado |
| Envelope de volume da faixa | `V` | `track.volume_envelope` | implementado |
| Envelope de pan da faixa | `P` | `track.pan_envelope` | implementado |
| Mostrar/ocultar envelopes | `Ctrl+Shift+E` | `track.toggle_envelopes` | implementado |

### Exibir

| Ação | Atalho | Id | Estado |
| :--- | :--- | :--- | :--- |
| Foco na linha do tempo | `Alt+0` | `view.focus_timeline` | implementado |
| Mídia do projeto | `Alt+5` | `view.project_media` | implementado |
| Trimmer | `Alt+2` | `view.trimmer` | implementado |
| Barramento master | `Alt+3` | `view.master_bus` | implementado |
| Prévia de vídeo | `Alt+4` | `view.preview` | implementado |
| Medidores de loudness | `Ctrl+Alt+7` | `view.loudness_meters` | implementado |
| Scopes de vídeo | `Ctrl+Alt+2` | `view.video_scopes` | implementado |
| Efeitos de áudio | `Ctrl+Alt+9` | `view.audio_effects` | implementado |
<!-- auto:end -->
