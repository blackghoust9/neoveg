---
tags: [neoveg, referencia, ui, icones]
---
# Mapa de ícones por função

Hub: [[fontes_de_referencia_de_ui]]. Inventário dos ícones do Neoveg: [[inventario_icones]].

## O que este mapa é, e o que não é
- Liga cada **função** de botão da referência ao **ícone original do Neoveg** que cumpre o mesmo papel. Os nomes de função vêm dos textos de dica da varredura e dos manuais.
- **Não** liga número de recurso a botão: os 384 arquivos de `results_assets/` são nomeados só pelo número do recurso, e a varredura não diz qual botão usa qual número. Esse mapa exato só se obteria abrindo o programa de referência, o que o projeto não faz (decisão do mantenedor, ver memória de clean-room).
- **Nenhum ícone da referência foi copiado**: os arquivos de `crates/neoveg_ui/assets/icons/` continuam 100 % autorais (AGENTS.md §28.2), e os 27 BMP coloridos vêm de Flat Color Icons (MIT).
- A célula da referência é de 16 px; o Neoveg desenha em 24×24 vetorial e mostra o glifo em cerca de 16,6 px nos botões de 26 px ([[medidas_da_interface]]).

## Barra de edição (dicas da varredura)
| Função da referência | Ícone do Neoveg (arquivo SVG) | Estado |
| :--- | :--- | :--- |
| Ferramenta de edição normal | `tool_normal` | existe |
| Ferramenta de seleção | `tool_selection` | existe |
| Ferramenta de envelope | `tool_envelope` | existe |
| Ponto de envelope | `keyframe_add` | existe |
| Quantizar para quadros | `quantize` | existe |
| Alinhar (snap) | `snapping` | existe |
| Ripple automático | `auto_ripple` | existe |
| Reproduzir em laço | `loop_region` | existe |
| Cor do envelope | (seletor de cor, sem ícone) | n/a |

## Transporte
| Função | Ícone | Estado |
| :--- | :--- | :--- |
| Reproduzir, pausar, parar, gravar | `transport_play`, `transport_pause`, `transport_stop`, `transport_record` | existem |
| Ir ao início e ao fim | `goto_start`, `goto_end` | existem |
| Um quadro para trás e para a frente | `step_back`, `step_forward` | existem |
| Shuttle (reverso, parado, avanço) | `shuttle_reverse`, `shuttle_hold`, `shuttle_forward` | existem |

## Edição e arquivo
| Função | Ícone | Estado |
| :--- | :--- | :--- |
| Dividir, cortar, copiar, colar, apagar | `split`, `cut`, `copy`, `paste`, `delete` | existem |
| Desfazer e refazer | `undo`, `redo` | existem (e BMP colorido) |
| Novo, abrir, salvar, projetos recentes | `new_project`, `open_project`, `save_project`, `recent_projects` | existem (os dois primeiros com BMP colorido) |
| Importar mídia e renderizar | `import_media`, `export_render` | existem (com BMP colorido) |
| Aparar início e fim | `trim_start`, `trim_end` | existem |
| Agrupar e desagrupar | `group`, `ungroup` | existem |
| Travar, mudo, fase invertida do evento | `lock_event`, `mute_event`, `phase_invert` | existem |
| Normalizar | `normalize_audio` | existe |
| Marcador, região | `marker`, `region` | existem |
| Fade de entrada e de saída, crossfade | `fade_in`, `fade_out`, `crossfade` | existem |
| Zoom | `zoom_in`, `zoom_out`, `tool_zoom` | existem |

## Trilhas e mixer
| Função | Ícone | Estado |
| :--- | :--- | :--- |
| Inserir trilha de áudio e de vídeo | `add_audio_track`, `add_video_track` | existem |
| Mudo e solo da trilha | `mute_track`, `solo_track` | existem |
| Armar gravação | `arm_record` | existe |
| Efeitos da trilha, movimento da trilha | `track_fx`, `track_motion` | existem (movimento ainda sem função) |
| Envelope de volume e de pan | `envelope_volume`, `envelope_pan` | existem |
| Canais | `channels` | existe |
| Mixer, master | `mixing_console`, `master_bus` | existem |

## Painéis e janelas
| Função | Ícone | Estado |
| :--- | :--- | :--- |
| Mídia do projeto, Trimmer, explorador | `project_media`, `trimmer`, `explorer` | existem |
| Visualização, escopos, loudness | `video_preview`, `video_scopes`, `loudness_meter` | existem |
| Geradores de mídia | `media_generators` | existe |
| Pan/Crop, efeitos do evento, bypass | `pan_crop`, `event_fx`, `bypass_fx` | existem |
| Correção de cor, chave de croma | `color_grading`, `chroma_key` | existem |
| Legendas, metrônomo | `captions`, `metronome` | existem |
| Propriedades do projeto, preferências, ajuda, boas-vindas | `project_properties`, `preferences`, `help`, `welcome` | existem |
| Sub-projeto | `subproject` | existe |

## Funções da referência sem ícone no Neoveg
Dashboard, modo de gradação de cor e de rastreamento como botões próprios, captura de vídeo, publicar, compartilhar e envio de projeto: ou `sem plano` ou ainda sem a função por trás. Quando uma delas entrar, o ícone é desenhado do zero (variante nova em `SvgIcon`, testes de estilo em `icons.rs`).
