# Inventário e Mapeamento de Ícones SVG (`neoveg_ui`)

Este documento registra a varredura completa das pastas de referência visual no Google Drive e o mapeamento semântico para a biblioteca vetorial **100% autoral** do Neoveg em `crates/neoveg_ui/assets/icons/` (`viewBox="0 0 24 24"`, `currentColor`, traços planos `butt`/`miter` sem cópia de pixels).

---

## 1. Resumo da Varredura das Pastas de Referência

| Pasta Inspecionada | Total de Arquivos | Distribuição por Formato | Conteúdo Identificado |
| :--- | ---: | :--- | :--- |
| `results_assets\` | **393** | `368 .png`, `25 .bmp` | Atlas horizontais de barra de ferramentas e faixas (`16 px` 1x, `32 px` 2x, `64 px` 4x em temas escuro e claro — 249 células por tira), além de glifos individuais e ícones de interface NLE clássica. |
| `assets\icons\` | **553** | `310 .bmp`, `141 .png`, `57 .ico`, `34 .gif`, `11 .jpg` | Recursos funcionais de edição NLE clássica (transições, captura, plug-ins OFX) e controles de interface (`116` arquivos: keyframes, cadeia de efeitos, conta-gotas, shuttle/jog, explorador de arquivos, renderização). |
| `assets\icons 22\` | **1.228** | `765 .png`, `298 .bmp`, `66 .gif`, `64 .ico`, `34 .jpg`, `1 .svg` | Recursos de interface NLE contemporânea em múltiplas escalas HiDPI (`1x/2x/4x` dark/lite: correção de cor, explorador unificado, diálogo de renderização, transições, notas de projeto, utilitários de layout, cadeia de vídeo e captura). |
| **Total Geral** | **2.174** | `.png`, `.bmp`, `.ico`, `.gif`, `.jpg`, `.svg` | Cobertura integral de edição, eventos, cabeçalhos de faixa, transporte e painéis acopláveis (`egui_dock`). |

---

## 2. Catálogo Completo dos 64 Ícones Vetoriais Autorais (`crates/neoveg_ui/assets/icons/`)

Todos os arquivos seguem o estilo de casa validado automaticamente pelos testes unitários de `crates/neoveg_ui/src/icons.rs`:
- `viewBox="0 0 24 24"`, `width="24"`, `height="24"`
- `fill="currentColor"` e/ou `stroke="currentColor"` (adaptação automática a temas claro e escuro)
- Traços retos (`stroke-linecap="butt"`, `stroke-linejoin="miter"`), sem cores fixas (`#`/`rgb`), sem `<text>` ou `<image>` embutidos.

### 2.1 Timeline & Edição (17 ícones)

| Arquivo SVG | Variante `SvgIcon` | Método Utilitário `Icons` | Função na Interface |
| :--- | :--- | :--- | :--- |
| `split.svg` | `SvgIcon::Split` | `Icons::split()` | Dividir eventos no cursor (`S`) |
| `trim_start.svg` | `SvgIcon::TrimStart` | `Icons::trim_start()` | Aparar início do evento até o cursor (`Alt+[`) |
| `trim_end.svg` | `SvgIcon::TrimEnd` | `Icons::trim_end()` | Aparar fim do evento até o cursor (`Alt+]`) |
| `auto_ripple.svg` | `SvgIcon::AutoRipple` | `Icons::auto_ripple()` | Alternar edição com Auto-Ripple (`Ctrl+L`) |
| `snapping.svg` | `SvgIcon::Snapping` | `Icons::snapping()` | Ativar/desativar Snapping magnético (`F8`) |
| `envelope_volume.svg` | `SvgIcon::EnvelopeVolume` | `Icons::envelope_volume()` | Inserir/exibir envelope de Volume (`V`) |
| `envelope_pan.svg` | `SvgIcon::EnvelopePan` | `Icons::envelope_pan()` | Inserir/exibir envelope de Pan (`P`) |
| `pitch_stretch.svg` | `SvgIcon::PitchStretch` | `Icons::pitch_stretch()` | Time-stretch / preservação de Pitch |
| `group.svg` | `SvgIcon::Group` | `Icons::group()` | Agrupar eventos / respeitar agrupamento (`G`) |
| `ungroup.svg` | `SvgIcon::Ungroup` | `Icons::ungroup()` | Desagrupar eventos selecionados (`U`) |
| `crossfade.svg` | `SvgIcon::Crossfade` | `Icons::crossfade()` | Crossfade automático entre eventos sobrepostos |
| `undo.svg` | `SvgIcon::Undo` | `Icons::undo()` | Desfazer operação (`Ctrl+Z`) |
| `redo.svg` | `SvgIcon::Redo` | `Icons::redo()` | Refazer operação (`Ctrl+Y`) |
| `zoom_in.svg` | `SvgIcon::ZoomIn` | `Icons::zoom_in()` | Ampliar zoom horizontal na timeline |
| `zoom_out.svg` | `SvgIcon::ZoomOut` | `Icons::zoom_out()` | Reduzir zoom horizontal na timeline |
| `keyframe_add.svg` | `SvgIcon::KeyframeAdd` | `Icons::keyframe_add()` | Inserir keyframe na automação / FX |
| `keyframe_delete.svg` | `SvgIcon::KeyframeDelete` | `Icons::keyframe_delete()` | Remover keyframe selecionado |

### 2.2 Eventos de Vídeo e Áudio (6 ícones)

| Arquivo SVG | Variante `SvgIcon` | Método Utilitário `Icons` | Função na Interface |
| :--- | :--- | :--- | :--- |
| `pan_crop.svg` | `SvgIcon::PanCrop` | `Icons::pan_crop()` | Enquadramento e recorte do evento (Event Pan/Crop) |
| `event_fx.svg` | `SvgIcon::EventFx` | `Icons::event_fx()` | Cadeia de efeitos do evento (Event FX) |
| `mute_event.svg` | `SvgIcon::MuteEvent` | `Icons::mute_event()` | Silenciar evento individual (Mute Event) |
| `lock_event.svg` | `SvgIcon::LockEvent` | `Icons::lock_event()` | Travar evento contra movimentação/edição (Lock Event) |
| `active_take.svg` | `SvgIcon::ActiveTake` | `Icons::active_take()` | Selecionar/alternar Take ativo (`T` / `Shift+T`) |
| `normalize_audio.svg` | `SvgIcon::NormalizeAudio` | `Icons::normalize_audio()` | Normalizar pico de áudio do evento |

### 2.3 Cabeçalho de Faixas — Track Header (11 ícones)

| Arquivo SVG | Variante `SvgIcon` | Método Utilitário `Icons` | Função na Interface |
| :--- | :--- | :--- | :--- |
| `mute.svg` | `SvgIcon::Mute` | `Icons::mute()` | Silenciar áudio universal (Mute) |
| `solo.svg` | `SvgIcon::Solo` | `Icons::solo()` | Isolar escuta universal (Solo) |
| `mute_track.svg` | `SvgIcon::MuteTrack` | `Icons::mute_track()` | Silenciar faixa (`Z`) |
| `solo_track.svg` | `SvgIcon::SoloTrack` | `Icons::solo_track()` | Isolar escuta/visão da faixa (`X`) |
| `track_fx.svg` | `SvgIcon::TrackFx` | `Icons::track_fx()` | Abrir cadeia de efeitos da faixa (Track FX) |
| `arm_record.svg` | `SvgIcon::ArmRecord` | `Icons::arm_record()` | Armar faixa para gravação (Arm for Record) |
| `track_motion.svg` | `SvgIcon::TrackMotion` | `Icons::track_motion()` | Movimento/composição 2D da faixa (Track Motion) |
| `phase_invert.svg` | `SvgIcon::PhaseInvert` | `Icons::phase_invert()` | Inverter polaridade/fase do áudio da faixa |
| `bypass_fx.svg` | `SvgIcon::BypassFx` | `Icons::bypass_fx()` | Ignorar efeitos (Bypass FX) |
| `add_audio_track.svg` | `SvgIcon::AddAudioTrack` | `Icons::add_audio_track()` | Inserir nova faixa de áudio (`Ctrl+Q`) |
| `add_video_track.svg` | `SvgIcon::AddVideoTrack` | `Icons::add_video_track()` | Inserir nova faixa de vídeo (`Ctrl+Shift+Q`) |

### 2.4 Barra de Transporte (10 ícones)

| Arquivo SVG | Variante `SvgIcon` | Método Utilitário `Icons` | Função na Interface |
| :--- | :--- | :--- | :--- |
| `transport_play.svg` | `SvgIcon::TransportPlay` | `Icons::transport_play()` | Iniciar reprodução (`Espaço` / `Enter`) |
| `transport_pause.svg` | `SvgIcon::TransportPause` | `Icons::transport_pause()` | Pausar no frame atual (`Enter`) |
| `transport_stop.svg` | `SvgIcon::TransportStop` | `Icons::transport_stop()` | Parar e retornar à origem (`Espaço` / `Esc`) |
| `transport_record.svg` | `SvgIcon::TransportRecord` | `Icons::transport_record()` | Gravar entrada de áudio (`Ctrl+R`) |
| `goto_start.svg` | `SvgIcon::GotoStart` | `Icons::goto_start()` | Ir para o início do projeto (`Ctrl+Home` / `W`) |
| `goto_end.svg` | `SvgIcon::GotoEnd` | `Icons::goto_end()` | Ir para o final do projeto (`Ctrl+End`) |
| `loop_region.svg` | `SvgIcon::LoopRegion` | `Icons::loop_region()` | Reprodução em laço na região (`Q`) |
| `shuttle_reverse.svg` | `SvgIcon::ShuttleReverse` | `Icons::shuttle_reverse()` | Shuttle reverso (`J` — -1x/-2x/-4x) |
| `shuttle_hold.svg` | `SvgIcon::ShuttleHold` | `Icons::shuttle_hold()` | Shuttle pausa / passo fino (`K`) |
| `shuttle_forward.svg` | `SvgIcon::ShuttleForward` | `Icons::shuttle_forward()` | Shuttle avanço rápido (`L` — 1x/2x/4x) |

### 2.5 Janelas, Ferramentas (`egui_dock`) & Ações de Painel (20 ícones)

| Arquivo SVG | Variante `SvgIcon` | Método Utilitário `Icons` | Função na Interface |
| :--- | :--- | :--- | :--- |
| `project_media.svg` | `SvgIcon::ProjectMedia` | `Icons::project_media()` | Aba/Janela Mídia do Projeto (`Alt+5`) |
| `trimmer.svg` | `SvgIcon::Trimmer` | `Icons::trimmer()` | Aba/Janela Trimmer (`Alt+2`) |
| `video_scopes.svg` | `SvgIcon::VideoScopes` | `Icons::video_scopes()` | Aba/Janela Video Scopes (`Ctrl+Alt+2`) |
| `mixing_console.svg` | `SvgIcon::MixingConsole` | `Icons::mixing_console()` | Aba/Janela Mixing Console / FX de Áudio (`Alt+8`) |
| `explorer.svg` | `SvgIcon::Explorer` | `Icons::explorer()` | Aba/Janela Explorer de arquivos |
| `master_bus.svg` | `SvgIcon::MasterBus` | `Icons::master_bus()` | Aba/Janela Master Bus (`Alt+1`) |
| `loudness_meter.svg` | `SvgIcon::LoudnessMeter` | `Icons::loudness_meter()` | Aba/Janela Medidor de Loudness EBU R128 (`Ctrl+Alt+6`) |
| `video_preview.svg` | `SvgIcon::VideoPreview` | `Icons::video_preview()` | Aba/Janela Prévia de Vídeo (`Alt+4`) |
| `timeline_view.svg` | `SvgIcon::TimelineView` | `Icons::timeline_view()` | Aba/Janela Timeline (`Alt+0`) |
| `color_grading.svg` | `SvgIcon::ColorGrading` | `Icons::color_grading()` | Janela Correção de Cor (Lift / Gamma / Gain) |
| `chroma_key.svg` | `SvgIcon::ChromaKey` | `Icons::chroma_key()` | Janela Chaveamento de Croma (Chroma Key) |
| `captions.svg` | `SvgIcon::Captions` | `Icons::captions()` | Aba/Painel de Legendas SRT/VTT & Whisper |
| `media_generators.svg` | `SvgIcon::MediaGenerators` | `Icons::media_generators()` | Geradores de Mídia (Barras SMPTE, Sólido, Xadrez) |
| `import_media.svg` | `SvgIcon::ImportMedia` | `Icons::import_media()` | Importar arquivo ou legenda |
| `export_render.svg` | `SvgIcon::ExportRender` | `Icons::export_render()` | Exportar / Renderizar como |
| `reset.svg` | `SvgIcon::Reset` | `Icons::reset()` | Restaurar parâmetro ou atalho padrão |
| `add.svg` | `SvgIcon::Add` | `Icons::add()` | Adicionar item / efeito / legenda |
| `remove.svg` | `SvgIcon::Remove` | `Icons::remove()` | Remover item / efeito / legenda |
| `move_up.svg` | `SvgIcon::MoveUp` | `Icons::move_up()` | Mover efeito para cima na cadeia |
| `move_down.svg` | `SvgIcon::MoveDown` | `Icons::move_down()` | Mover efeito para baixo na cadeia |

---

## 3. Exportação para Formatos Secundários

O utilitário `scripts/generate_icon_formats.py` converte todos os 62 SVGs acima para o diretório de ambiente (`D:\ambiente_neoveg\assets\icons\` por padrão, ou via `NEOVEG_ENV_DIR` / `--out`):

- `svg\<nome>.svg` (62 arquivos `.svg`)
- `png\<nome>_32x32.png`, `png\<nome>_64x64.png`, `png\<nome>_128x128.png` (186 arquivos `.png` RGBA transparentes)
- `bmp\<nome>.bmp` (62 arquivos `.bmp` 32-bit BGRA `BITMAPV4HEADER` com transparência alpha)
- `jpg\<nome>.jpg` (62 arquivos `.jpg` 4:4:4 sobre fundo `#1e1e1e`)

---

## 3. Ícones acrescentados a partir dos comandos dos manuais (35)

Desenhados do zero no mesmo estilo (24x24, `currentColor`, traços retos), a partir da lista de botões e ferramentas que os manuais descrevem (barra principal, ferramentas de edição, marcadores e transporte). Nenhum pixel foi copiado de material de terceiros.

| Grupo | Arquivos |
| :--- | :--- |
| Arquivo e projeto | `new_project`, `open_project`, `save_project`, `project_properties`, `cut`, `copy`, `paste`, `delete`, `help` |
| Ferramentas de edição | `tool_normal`, `tool_shuffle`, `tool_slip`, `tool_slide`, `tool_stretch`, `tool_split_trim`, `tool_envelope`, `tool_selection`, `tool_zoom` |
| Linha do tempo e transporte | `quantize`, `metronome`, `marker`, `region`, `scrub`, `step_forward`, `step_back`, `insert_time`, `reverse`, `velocity`, `fade_in`, `fade_out` |
| Áudio e projeto | `channels`, `subproject` |
| Aplicativo | `preferences`, `welcome`, `recent_projects` |

A barra principal sob o menu usa 13 deles; os de ferramentas de edição, marcadores e transporte ficam prontos para os recursos que ainda faltam (slip, slide, embaralhar, velocidade, reverter).
