//! Table of every user-invocable command.
//!
//! Menus, toolbars, context menus and the keyboard map all refer to commands by [`Command`], so a
//! shortcut can be rebound without touching the code that runs the action. Commands whose
//! behaviour is not written yet are listed with `implemented() == false`: they already own a
//! default shortcut and a label, but the UI ignores them until the feature lands.

/// Functional group, used to organise the "Personalizar teclado" window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    File,
    Edit,
    Insert,
    Cursor,
    Transport,
    Options,
    Track,
    View,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::File,
        Category::Edit,
        Category::Insert,
        Category::Cursor,
        Category::Transport,
        Category::Options,
        Category::Track,
        Category::View,
    ];

    /// Label shown in the default language (pt-BR).
    pub fn label(self) -> &'static str {
        match self {
            Category::File => "Arquivo",
            Category::Edit => "Editar",
            Category::Insert => "Inserir",
            Category::Cursor => "Cursor e seleção",
            Category::Transport => "Reprodução",
            Category::Options => "Opções",
            Category::Track => "Faixas",
            Category::View => "Exibir",
        }
    }
}

macro_rules! commands {
    ($($variant:ident => ($id:literal, $label:literal, $category:ident, $implemented:literal)),+ $(,)?) => {
        /// A user-invocable command.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Command {
            $($variant),+
        }

        impl Command {
            /// Every command, in the order shown to the user.
            pub const ALL: &'static [Command] = &[$(Command::$variant),+];

            /// Stable dotted identifier stored in `preferences.json` (never translated).
            pub fn id(self) -> &'static str {
                match self {
                    $(Command::$variant => $id),+
                }
            }

            /// Label in the default language (pt-BR).
            pub fn label(self) -> &'static str {
                match self {
                    $(Command::$variant => $label),+
                }
            }

            pub fn category(self) -> Category {
                match self {
                    $(Command::$variant => Category::$category),+
                }
            }

            /// `false` while the feature behind the command is still on the roadmap.
            pub fn implemented(self) -> bool {
                match self {
                    $(Command::$variant => $implemented),+
                }
            }
        }
    };
}

commands! {
    // ---- Arquivo
    FileOpen => ("file.open", "Abrir projeto…", File, true),
    FileSave => ("file.save", "Salvar", File, true),
    FileSaveAs => ("file.save_as", "Salvar como…", File, true),
    FileNew => ("file.new", "Novo projeto", File, false),
    FileNewQuick => ("file.new_quick", "Novo projeto vazio (sem diálogo)", File, false),
    FileClose => ("file.close", "Fechar projeto", File, false),
    FileProperties => ("file.properties", "Propriedades do projeto…", File, false),

    // ---- Editar
    EditUndo => ("edit.undo", "Desfazer", Edit, true),
    EditRedo => ("edit.redo", "Refazer", Edit, true),
    EditDelete => ("edit.delete", "Excluir seleção", Edit, true),
    EditSplit => ("edit.split", "Dividir no cursor", Edit, true),
    EditCut => ("edit.cut", "Recortar", Edit, true),
    EditCopy => ("edit.copy", "Copiar", Edit, true),
    EditPaste => ("edit.paste", "Colar", Edit, true),
    EditPasteInsert => ("edit.paste_insert", "Colar inserindo", Edit, true),
    EditPasteRepeat => ("edit.paste_repeat", "Repetir colar…", Edit, true),
    EditTrimToSelection => ("edit.trim", "Aparar à seleção", Edit, true),
    EditTrimStart => ("edit.trim_start", "Aparar o início ao cursor", Edit, true),
    EditTrimEnd => ("edit.trim_end", "Aparar o fim ao cursor", Edit, true),
    EditSelectAll => ("edit.select_all", "Selecionar tudo", Edit, true),
    EditSelectNone => ("edit.select_none", "Desfazer seleção", Edit, true),
    EditGroup => ("edit.group", "Agrupar eventos", Edit, true),
    EditUngroup => ("edit.ungroup", "Desagrupar eventos", Edit, true),
    EditIgnoreGrouping => ("edit.ignore_grouping", "Ignorar agrupamento", Edit, true),
    EditToolNormal => ("edit.tool.normal", "Ferramenta normal", Edit, false),
    EditToolNext => ("edit.tool.next", "Próxima ferramenta de edição", Edit, false),
    EditToolPrevious => ("edit.tool.previous", "Ferramenta de edição anterior", Edit, false),
    EditNextTake => ("edit.take.next", "Próximo take", Edit, false),
    EditPreviousTake => ("edit.take.previous", "Take anterior", Edit, false),

    // ---- Inserir
    InsertAudioTrack => ("insert.audio_track", "Inserir faixa de áudio", Insert, true),
    InsertVideoTrack => ("insert.video_track", "Inserir faixa de vídeo", Insert, true),
    InsertMarker => ("insert.marker", "Inserir marcador", Insert, true),
    InsertRegion => ("insert.region", "Inserir região", Insert, true),
    InsertCommandMarker => ("insert.command", "Inserir marcador de comando…", Insert, false),
    RenderToNewTrack => ("tools.render_to_new_track", "Renderizar para nova faixa", Insert, false),

    // ---- Cursor e seleção
    CursorLeftByFrame => ("cursor.left_by_frame", "Cursor um quadro à esquerda", Cursor, true),
    CursorRightByFrame => ("cursor.right_by_frame", "Cursor um quadro à direita", Cursor, true),
    CursorProjectStart => ("cursor.project_start", "Ir para o início do projeto", Cursor, true),
    CursorProjectEnd => ("cursor.project_end", "Ir para o fim do projeto", Cursor, true),
    CursorSelectionStart => ("cursor.selection_start", "Ir para o início da seleção ou da vista", Cursor, true),
    CursorSelectionEnd => ("cursor.selection_end", "Ir para o fim da seleção ou da vista", Cursor, true),
    CursorPreviousMarker => ("cursor.previous_marker", "Marcador anterior", Cursor, true),
    CursorNextMarker => ("cursor.next_marker", "Próximo marcador", Cursor, true),
    CursorMarker1 => ("cursor.marker_one", "Ir para o marcador 1", Cursor, true),
    CursorMarker2 => ("cursor.marker_two", "Ir para o marcador 2", Cursor, true),
    CursorMarker3 => ("cursor.marker_three", "Ir para o marcador 3", Cursor, true),
    CursorMarker4 => ("cursor.marker_four", "Ir para o marcador 4", Cursor, true),
    CursorMarker5 => ("cursor.marker_five", "Ir para o marcador 5", Cursor, true),
    CursorMarker6 => ("cursor.marker_six", "Ir para o marcador 6", Cursor, true),
    CursorMarker7 => ("cursor.marker_seven", "Ir para o marcador 7", Cursor, true),
    CursorMarker8 => ("cursor.marker_eight", "Ir para o marcador 8", Cursor, true),
    CursorMarker9 => ("cursor.marker_nine", "Ir para o marcador 9", Cursor, true),
    CursorMarker10 => ("cursor.marker_ten", "Ir para o marcador 10", Cursor, true),
    CursorPreviousEdit => ("cursor.previous_edit", "Ponto de edição anterior", Cursor, true),
    CursorNextEdit => ("cursor.next_edit", "Próximo ponto de edição", Cursor, true),
    CursorGoTo => ("cursor.go_to", "Ir para o tempo…", Cursor, true),
    CursorCenterView => ("cursor.center_view", "Centralizar a vista no cursor", Cursor, true),
    SelectionSetIn => ("selection.set_in", "Definir ponto de entrada", Cursor, true),
    SelectionSetOut => ("selection.set_out", "Definir ponto de saída", Cursor, true),
    SelectionLoopRegion => ("selection.loop_region", "Selecionar a região de laço", Cursor, true),

    // ---- Reprodução
    TransportPlayToggle => ("transport.play_toggle", "Reproduzir / pausar", Transport, true),
    TransportStop => ("transport.stop", "Parar", Transport, false),
    TransportPlayFromStart => ("transport.play_from_start", "Reproduzir do início", Transport, false),
    TransportLoop => ("transport.loop", "Reprodução em laço", Transport, true),
    TransportRecord => ("transport.record", "Gravar", Transport, false),

    // ---- Opções
    OptionsSnapping => ("options.snapping", "Alinhamento (snap)", Options, true),
    OptionsAutoCrossfade => ("options.auto_crossfade", "Crossfade automático", Options, true),
    OptionsSnapToGrid => ("options.snap_grid", "Alinhar à grade", Options, false),
    OptionsSnapToMarkers => ("options.snap_markers", "Alinhar a marcadores", Options, false),
    OptionsSnapToAllEvents => ("options.snap_all_events", "Alinhar a todos os eventos", Options, false),
    OptionsQuantizeToFrames => ("options.quantize", "Quantizar para quadros", Options, false),
    OptionsAutoRipple => ("options.auto_ripple", "Ripple automático", Options, false),
    RippleAffectedTracks => ("edit.ripple.affected", "Ripple nas faixas afetadas", Options, false),
    RippleWithMarkers => ("edit.ripple.markers", "Ripple nas faixas, marcadores e regiões", Options, false),
    RippleAll => ("edit.ripple.all", "Ripple em tudo", Options, false),

    // ---- Faixas
    TrackMute => ("track.mute", "Mudo na faixa", Track, true),
    TrackMuteExclusive => ("track.mute_exclusive", "Mudo só nesta faixa", Track, true),
    TrackSolo => ("track.solo", "Solo na faixa", Track, true),
    TrackSoloExclusive => ("track.solo_exclusive", "Solo só nesta faixa", Track, true),
    TrackVolumeEnvelope => ("track.volume_envelope", "Envelope de volume da faixa", Track, false),
    TrackPanEnvelope => ("track.pan_envelope", "Envelope de pan da faixa", Track, false),
    TrackToggleEnvelopes => ("track.toggle_envelopes", "Mostrar/ocultar envelopes", Track, false),

    // ---- Exibir
    ViewFocusTimeline => ("view.focus_timeline", "Foco na linha do tempo", View, true),
    ViewProjectMedia => ("view.project_media", "Mídia do projeto", View, true),
    ViewTrimmer => ("view.trimmer", "Trimmer", View, true),
    ViewMasterBus => ("view.master_bus", "Barramento master", View, true),
    ViewPreview => ("view.preview", "Prévia de vídeo", View, true),
    ViewLoudnessMeters => ("view.loudness_meters", "Medidores de loudness", View, true),
    ViewVideoScopes => ("view.video_scopes", "Scopes de vídeo", View, true),
    ViewAudioEffects => ("view.audio_effects", "Efeitos de áudio", View, true),
}

impl Command {
    /// Looks a command up by its stable identifier.
    pub fn from_id(id: &str) -> Option<Command> {
        Command::ALL.iter().copied().find(|c| c.id() == id)
    }
}

impl Command {
    /// `1`–`10` for the commands that jump to the n-th marker (the `0` key is the tenth).
    pub fn marker_number(self) -> Option<usize> {
        match self {
            Command::CursorMarker1 => Some(1),
            Command::CursorMarker2 => Some(2),
            Command::CursorMarker3 => Some(3),
            Command::CursorMarker4 => Some(4),
            Command::CursorMarker5 => Some(5),
            Command::CursorMarker6 => Some(6),
            Command::CursorMarker7 => Some(7),
            Command::CursorMarker8 => Some(8),
            Command::CursorMarker9 => Some(9),
            Command::CursorMarker10 => Some(10),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_round_trip() {
        let mut seen = HashSet::new();
        for &c in Command::ALL {
            assert!(seen.insert(c.id()), "duplicate id {}", c.id());
            assert_eq!(Command::from_id(c.id()), Some(c));
            assert!(!c.label().is_empty());
        }
        assert_eq!(Command::from_id("does.not.exist"), None);
    }

    #[test]
    fn ids_are_dotted_lowercase() {
        for &c in Command::ALL {
            assert!(c.id().contains('.'), "{}", c.id());
            assert!(
                c.id()
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch == '.' || ch == '_'),
                "{}",
                c.id()
            );
        }
    }

    #[test]
    fn every_category_has_commands() {
        for cat in Category::ALL {
            assert!(
                Command::ALL.iter().any(|c| c.category() == cat),
                "{cat:?} is empty"
            );
        }
    }
}
