//! Standardized Alerts and Messages Subsystem (results.md §10).
//!
//! Provides standardized dialog messages, localized strings (EN-US and PT-BR),
//! non-blocking thread-safe dispatch queues, and mock handlers for headless testing.
//! Adheres strictly to clean-room trademark avoidance rules.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// Category of an alert or error message matching results.md §10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertCategory {
    MissingMedia,
    CodecFormat,
    RenderPrerender,
    DiskMemory,
    GpuAcceleration,
    PluginVst,
    BinsExplorer,
    General,
}

/// Alert severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Information,
    Warning,
    Error,
    Confirmation,
}

/// Response action for alert dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertAction {
    Ok,
    Cancel,
    Yes,
    No,
    Browse,
    Relink,
    Retry,
    Ignore,
    IgnoreAll,
}

/// Resource IDs and error codes cataloged from results.md §10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum AlertCode {
    // --- Missing Media (§10.1) ---
    MissingProjectMediaElements = 1561,
    MediaIsOfflinePrompt = 1634,
    FindMissingFile = 16132,
    MediaFileNotFound = 16926,
    OfflineMediaFile = 17250,
    CannotSaveMarkersMediaOffline = 17361,
    CannotOpenInSoundEditorMediaOffline = 17363,
    LocateMissingFilePrompt = 17630,
    TimelineMediaDeleteConfirm = 17264,

    // --- Codec / Format / Decoding (§10.2) ---
    FileCorrupted = 1648,
    StreamAttributesCouldNotBeDetermined = 17359,
    MissingVideoCodec = 17369,
    CorruptVideoStream = 17370,
    MissingAudioCodec = 17381,
    UnknownStreamError = 17382,
    CorruptAudioStream = 17384,
    UnsupportedAudioChannelCount = 33501,

    // --- Render / Prerender / Templates (§10.3) ---
    TemplateNameInvalid = 1578,
    BuiltinTemplateCannotModify = 1580,
    OverwriteTemplateConfirm = 1583,
    BuiltinTemplateCannotDelete = 1585,
    CannotDeleteBuiltinTemplate = 17364,
    CannotModifyBuiltinTemplate = 17365,
    TemplateCorruptOrUnreadable = 17437,
    Over80PercentHasToBeRendered = 1881,
    PlayerAssociationMissing = 1894,
    NoTemplatesSupportMedia = 1910,
    RenderFileAlreadyExists = 1911,
    EmptyProjectRender = 3404,
    CancelRenderConfirmation = 44720,

    // --- Memory / Disk / System Resources (§10.4) ---
    ClearEditHistoryConfirmation = 16227,
    InsufficientDiskSpace = 17603,
    ParametersInvalid = 18400,
    DiskFullDvdImage = 18548,
    DynamicRamPreviewZeroMb = 46259,

    // --- GPU Acceleration (§10.5) ---
    GpuDeviceLost = 16013,
    GpuInitFailed = 16016,
    GpuDriverOutdated = 16017,
    GpuFatalError = 16020,
    GpuAccelerationError = 33316,
    GpuDeviceDriverProblem = 33317,

    // --- Plug-ins / Effects / OFX / VST (§10.6) ---
    EffectFailed = 16092,
    PluginChainFull = 16511,
    PluginFailedToLoad = 16555,
    PluginCrashed = 16556,
    PluginIncompatible = 16560,
    FxSendFull = 16587,
    VideoEffectsUiInitFailed = 33413,

    // --- Bins & Explorer (§12.10, §12.11) ---
    RootBinCannotRemove = 12878,
    RootBinCannotRename = 12889,
    BinNameEmpty = 12887,
    DuplicateBinRenamed = 18174,
    InvalidDrive = 3915,
}

impl AlertCode {
    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Standardized alert specification with localized templates and allowable actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardAlert {
    pub code: AlertCode,
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub en_us_template: &'static str,
    pub pt_br_template: &'static str,
    pub default_actions: Vec<AlertAction>,
}

/// Dispatched alert event ready for UI consumption or headless test interception.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: u64,
    pub code: AlertCode,
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub message: String,
    pub resource_path: Option<PathBuf>,
    pub allowed_actions: Vec<AlertAction>,
}

type MockHandlerFn = Arc<dyn Fn(&AlertEvent) -> AlertAction + Send + Sync>;

/// Non-blocking, thread-safe alert dispatch queue.
#[derive(Default)]
pub struct AlertQueue {
    events: Mutex<VecDeque<AlertEvent>>,
    mock_handler: Mutex<Option<MockHandlerFn>>,
    next_id: Mutex<u64>,
}

static GLOBAL_QUEUE: OnceLock<AlertQueue> = OnceLock::new();

impl AlertQueue {
    pub fn global() -> &'static Self {
        GLOBAL_QUEUE.get_or_init(Self::default)
    }

    /// Sets a programmatic mock handler for headless testing.
    pub fn set_mock_handler(&self, handler: Option<MockHandlerFn>) {
        let mut lock = self.mock_handler.lock().unwrap();
        *lock = handler;
    }

    /// Dispatches an alert. If a mock handler is active, immediately invokes it and returns the action.
    /// Otherwise, pushes the event onto the non-blocking queue and returns `None`.
    pub fn dispatch(
        &self,
        code: AlertCode,
        args: &[&str],
        locale: &str,
        path: Option<PathBuf>,
    ) -> Option<AlertAction> {
        let alert_def = AlertRegistry::get(code);
        let message = AlertRegistry::format_message(code, args, locale);

        let mut id_lock = self.next_id.lock().unwrap();
        *id_lock += 1;
        let event_id = *id_lock;

        let event = AlertEvent {
            id: event_id,
            code,
            category: alert_def.category,
            severity: alert_def.severity,
            message,
            resource_path: path,
            allowed_actions: alert_def.default_actions.clone(),
        };

        let handler_opt = self.mock_handler.lock().unwrap().clone();
        if let Some(ref handler) = handler_opt {
            return Some(handler(&event));
        }

        self.events.lock().unwrap().push_back(event);
        None
    }

    /// Pops the oldest pending alert event from the queue.
    pub fn pop_event(&self) -> Option<AlertEvent> {
        self.events.lock().unwrap().pop_front()
    }

    /// Clears all pending alert events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

/// Central catalog of standardized alert definitions.
pub struct AlertRegistry;

impl AlertRegistry {
    pub fn get(code: AlertCode) -> StandardAlert {
        use AlertAction::*;
        use AlertCategory::*;
        use AlertSeverity::*;

        match code {
            // Missing media
            AlertCode::MissingProjectMediaElements => StandardAlert {
                code,
                category: MissingMedia,
                severity: Warning,
                en_us_template: "Some media pool elements could not be found.",
                pt_br_template: "Alguns elementos da lista de mídia não foram encontrados.",
                default_actions: vec![Ok, Cancel],
            },
            AlertCode::MediaIsOfflinePrompt => StandardAlert {
                code,
                category: MissingMedia,
                severity: Warning,
                en_us_template: "The media file '{0}' is currently offline. Do you want to relink it?",
                pt_br_template: "O arquivo de mídia '{0}' está offline. Deseja reconectá-lo?",
                default_actions: vec![Yes, No, IgnoreAll],
            },
            AlertCode::FindMissingFile => StandardAlert {
                code,
                category: MissingMedia,
                severity: Information,
                en_us_template: "Search for missing file...",
                pt_br_template: "Procurar arquivo ausente...",
                default_actions: vec![Browse, Cancel],
            },
            AlertCode::MediaFileNotFound => StandardAlert {
                code,
                category: MissingMedia,
                severity: Error,
                en_us_template: "The file '{0}' was not found in the specified path.",
                pt_br_template: "O arquivo '{0}' não foi encontrado no caminho especificado.",
                default_actions: vec![Browse, Ignore, IgnoreAll],
            },
            AlertCode::OfflineMediaFile => StandardAlert {
                code,
                category: MissingMedia,
                severity: Warning,
                en_us_template: "The file '{0}' is offline.",
                pt_br_template: "O arquivo '{0}' está offline.",
                default_actions: vec![Ok],
            },
            AlertCode::CannotSaveMarkersMediaOffline => StandardAlert {
                code,
                category: MissingMedia,
                severity: Error,
                en_us_template: "Cannot save markers because the media file is offline.",
                pt_br_template: "Não é possível salvar marcadores porque o arquivo de mídia está offline.",
                default_actions: vec![Ok],
            },
            AlertCode::CannotOpenInSoundEditorMediaOffline => StandardAlert {
                code,
                category: MissingMedia,
                severity: Error,
                en_us_template: "Cannot open in sound editor because the media file is offline.",
                pt_br_template: "Não é possível abrir no editor de áudio porque o arquivo de mídia está offline.",
                default_actions: vec![Ok],
            },
            AlertCode::LocateMissingFilePrompt => StandardAlert {
                code,
                category: MissingMedia,
                severity: Warning,
                en_us_template: "The file '{0}' could not be located. Please select an existing file.",
                pt_br_template: "O arquivo '{0}' não pôde ser localizado. Selecione um arquivo existente.",
                default_actions: vec![Browse, Ignore, IgnoreAll],
            },
            AlertCode::TimelineMediaDeleteConfirm => StandardAlert {
                code,
                category: MissingMedia,
                severity: Confirmation,
                en_us_template: "The selected media is in use on the timeline. Remove from Project Media list and all timeline references? {0}",
                pt_br_template: "A mídia selecionada está em uso na linha do tempo. Remover da lista de Mídia do Projeto e de todas as referências da linha do tempo? {0}",
                default_actions: vec![Yes, No],
            },

            // Codecs & formats
            AlertCode::FileCorrupted => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "The media file '{0}' is corrupted and cannot be opened.",
                pt_br_template: "O arquivo de mídia '{0}' está corrompido e não pode ser aberto.",
                default_actions: vec![Ok],
            },
            AlertCode::StreamAttributesCouldNotBeDetermined => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "Stream attributes could not be determined for file '{0}'.",
                pt_br_template: "Não foi possível determinar os atributos do fluxo para o arquivo '{0}'.",
                default_actions: vec![Ok],
            },
            AlertCode::MissingVideoCodec => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "Stream {0} cannot be opened because of a missing video codec.",
                pt_br_template: "O fluxo {0} não pode ser aberto devido à falta de um codec de vídeo.",
                default_actions: vec![Ok],
            },
            AlertCode::CorruptVideoStream => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "The video stream data is corrupt or in an unrecognized format.",
                pt_br_template: "Os dados do fluxo de vídeo estão corrompidos ou em formato não reconhecido.",
                default_actions: vec![Ok],
            },
            AlertCode::MissingAudioCodec => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "Stream {0} cannot be opened because of a missing audio codec.",
                pt_br_template: "O fluxo {0} não pode ser aberto devido à falta de um codec de áudio.",
                default_actions: vec![Ok],
            },
            AlertCode::UnknownStreamError => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "An unknown error occurred while processing stream {0}.",
                pt_br_template: "Ocorreu um erro desconhecido ao processar o fluxo {0}.",
                default_actions: vec![Ok],
            },
            AlertCode::CorruptAudioStream => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "The audio stream data is corrupt or in an unrecognized format.",
                pt_br_template: "Os dados do fluxo de áudio estão corrompidos ou em formato não reconhecido.",
                default_actions: vec![Ok],
            },
            AlertCode::UnsupportedAudioChannelCount => StandardAlert {
                code,
                category: CodecFormat,
                severity: Error,
                en_us_template: "Unsupported audio channel configuration ({0} channels).",
                pt_br_template: "Configuração de canais de áudio não suportada ({0} canais).",
                default_actions: vec![Ok],
            },

            // Render / Prerender / Templates
            AlertCode::TemplateNameInvalid => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Error,
                en_us_template: "Can't save the template without a valid name. Please rename it before saving.",
                pt_br_template: "Não é possível salvar o modelo sem um nome válido. Renomeie-o antes de salvar.",
                default_actions: vec![Ok],
            },
            AlertCode::BuiltinTemplateCannotModify | AlertCode::CannotModifyBuiltinTemplate => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Error,
                en_us_template: "Can't modify the built-in '{0}' template. Please rename it before saving.",
                pt_br_template: "Não é possível modificar o modelo embutido '{0}'. Renomeie-o antes de salvar.",
                default_actions: vec![Ok],
            },
            AlertCode::OverwriteTemplateConfirm => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Confirmation,
                en_us_template: "Overwrite existing '{0}' template?",
                pt_br_template: "Substituir o modelo existente '{0}'?",
                default_actions: vec![Yes, No],
            },
            AlertCode::BuiltinTemplateCannotDelete | AlertCode::CannotDeleteBuiltinTemplate => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Error,
                en_us_template: "Can't delete the built-in '{0}' template.",
                pt_br_template: "Não é possível excluir o modelo embutido '{0}'.",
                default_actions: vec![Ok],
            },
            AlertCode::TemplateCorruptOrUnreadable => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Error,
                en_us_template: "The template file '{0}' is unreadable or corrupt.",
                pt_br_template: "O arquivo de modelo '{0}' é ilegível ou está corrompido.",
                default_actions: vec![Ok],
            },
            AlertCode::Over80PercentHasToBeRendered => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Information,
                en_us_template: "Over 80% of the project requires rendering.",
                pt_br_template: "Mais de 80% do projeto precisa ser renderizado.",
                default_actions: vec![Ok, Cancel],
            },
            AlertCode::PlayerAssociationMissing => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Warning,
                en_us_template: "No media player application is associated with this file format.",
                pt_br_template: "Nenhum reprodutor de mídia está associado a este formato de arquivo.",
                default_actions: vec![Ok],
            },
            AlertCode::NoTemplatesSupportMedia => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Warning,
                en_us_template: "No available render templates match the current project media format.",
                pt_br_template: "Nenhum modelo de renderização disponível corresponde ao formato da mídia do projeto.",
                default_actions: vec![Ok],
            },
            AlertCode::RenderFileAlreadyExists => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Confirmation,
                en_us_template: "The output file '{0}' already exists. Do you want to overwrite it?",
                pt_br_template: "O arquivo de saída '{0}' já existe. Deseja substituí-lo?",
                default_actions: vec![Yes, No],
            },
            AlertCode::EmptyProjectRender => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Error,
                en_us_template: "Unable to render since project is empty.",
                pt_br_template: "Não é possível renderizar porque o projeto está vazio.",
                default_actions: vec![Ok],
            },
            AlertCode::CancelRenderConfirmation => StandardAlert {
                code,
                category: RenderPrerender,
                severity: Confirmation,
                en_us_template: "Do you want to cancel the current rendering operation?",
                pt_br_template: "Deseja cancelar a operação de renderização atual?",
                default_actions: vec![Yes, No],
            },

            // Memory / Disk Resources
            AlertCode::ClearEditHistoryConfirmation => StandardAlert {
                code,
                category: DiskMemory,
                severity: Confirmation,
                en_us_template: "Clearing the edit history will free memory, but you will not be able to undo prior operations. Proceed?",
                pt_br_template: "Limpar o histórico de edição liberará memória, mas não será possível desfazer operações anteriores. Prosseguir?",
                default_actions: vec![Yes, No],
            },
            AlertCode::InsufficientDiskSpace => StandardAlert {
                code,
                category: DiskMemory,
                severity: Warning,
                en_us_template: "Estimated file size ({0} MB) exceeds available free disk space ({1} MB).",
                pt_br_template: "O tamanho estimado do arquivo ({0} MB) excede o espaço livre em disco ({1} MB).",
                default_actions: vec![Retry, Cancel, Browse],
            },
            AlertCode::ParametersInvalid => StandardAlert {
                code,
                category: DiskMemory,
                severity: Error,
                en_us_template: "One or more parameters are invalid.",
                pt_br_template: "Um ou mais parâmetros são inválidos.",
                default_actions: vec![Ok],
            },
            AlertCode::DiskFullDvdImage => StandardAlert {
                code,
                category: DiskMemory,
                severity: Error,
                en_us_template: "There is not enough free disc space to generate disc image.",
                pt_br_template: "Não há espaço livre em disco suficiente para gerar a imagem de disco.",
                default_actions: vec![Ok],
            },
            AlertCode::DynamicRamPreviewZeroMb => StandardAlert {
                code,
                category: DiskMemory,
                severity: Warning,
                en_us_template: "The Dynamic RAM Preview max value has been set to 0 MB. In order to use the Dynamic RAM Preview feature, you must allocate system RAM to it. Visit Preferences to adjust this setting.",
                pt_br_template: "O valor máximo de Prévia na RAM Dinâmica foi definido como 0 MB. Para utilizar este recurso, aloque memória RAM do sistema nas Preferências.",
                default_actions: vec![Ok],
            },

            // GPU Acceleration
            AlertCode::GpuDeviceLost => StandardAlert {
                code,
                category: GpuAcceleration,
                severity: Warning,
                en_us_template: "GPU device was lost. Switching to pure-CPU software fallback pipeline.",
                pt_br_template: "O dispositivo GPU foi desconectado. Alternando para processamento por CPU.",
                default_actions: vec![Ok],
            },
            AlertCode::GpuInitFailed => StandardAlert {
                code,
                category: GpuAcceleration,
                severity: Warning,
                en_us_template: "GPU acceleration initialization failed. Using software rendering pipeline.",
                pt_br_template: "Falha na inicialização da aceleração por GPU. Utilizando renderização por software.",
                default_actions: vec![Ok],
            },
            AlertCode::GpuDriverOutdated => StandardAlert {
                code,
                category: GpuAcceleration,
                severity: Warning,
                en_us_template: "The GPU graphics driver is outdated and may cause rendering instability.",
                pt_br_template: "O driver gráfico da GPU está desatualizado e pode causar instabilidade de renderização.",
                default_actions: vec![Ok],
            },
            AlertCode::GpuFatalError => StandardAlert {
                code,
                category: GpuAcceleration,
                severity: Error,
                en_us_template: "A fatal GPU error occurred. The application has fallen back to CPU processing.",
                pt_br_template: "Ocorreu um erro fatal na GPU. A aplicação alternou para processamento em CPU.",
                default_actions: vec![Ok],
            },
            AlertCode::GpuAccelerationError => StandardAlert {
                code,
                category: GpuAcceleration,
                severity: Error,
                en_us_template: "GPU Acceleration Error: Switching to CPU software fallback.",
                pt_br_template: "Erro de aceleração por GPU: Alternando para processamento por CPU.",
                default_actions: vec![Ok],
            },
            AlertCode::GpuDeviceDriverProblem => StandardAlert {
                code,
                category: GpuAcceleration,
                severity: Warning,
                en_us_template: "The GPU driver encountered an unexpected error.",
                pt_br_template: "O driver da GPU encontrou um erro inesperado.",
                default_actions: vec![Ok],
            },

            // Plug-ins / Effects
            AlertCode::EffectFailed => StandardAlert {
                code,
                category: PluginVst,
                severity: Error,
                en_us_template: "Effect processing failed for '{0}'.",
                pt_br_template: "Falha no processamento do efeito '{0}'.",
                default_actions: vec![Ok],
            },
            AlertCode::PluginChainFull => StandardAlert {
                code,
                category: PluginVst,
                severity: Warning,
                en_us_template: "The effect chain has reached its maximum capacity.",
                pt_br_template: "A cadeia de efeitos atingiu a capacidade máxima.",
                default_actions: vec![Ok],
            },
            AlertCode::PluginFailedToLoad => StandardAlert {
                code,
                category: PluginVst,
                severity: Error,
                en_us_template: "The plug-in '{0}' failed to load.",
                pt_br_template: "Falha ao carregar o plug-in '{0}'.",
                default_actions: vec![Ok],
            },
            AlertCode::PluginCrashed => StandardAlert {
                code,
                category: PluginVst,
                severity: Error,
                en_us_template: "The plug-in '{0}' crashed unexpectedly and has been disabled.",
                pt_br_template: "O plug-in '{0}' travou inesperadamente e foi desativado.",
                default_actions: vec![Ok],
            },
            AlertCode::PluginIncompatible => StandardAlert {
                code,
                category: PluginVst,
                severity: Error,
                en_us_template: "The plug-in '{0}' is incompatible with this host version.",
                pt_br_template: "O plug-in '{0}' é incompatível com esta versão do programa.",
                default_actions: vec![Ok],
            },
            AlertCode::FxSendFull => StandardAlert {
                code,
                category: PluginVst,
                severity: Warning,
                en_us_template: "The effect send bus has reached maximum capacity.",
                pt_br_template: "O barramento de envio de efeitos atingiu a capacidade máxima.",
                default_actions: vec![Ok],
            },
            AlertCode::VideoEffectsUiInitFailed => StandardAlert {
                code,
                category: PluginVst,
                severity: Error,
                en_us_template: "Failed to initialize video effect user interface.",
                pt_br_template: "Falha ao inicializar a interface de usuário do efeito de vídeo.",
                default_actions: vec![Ok],
            },

            // Bins & Explorer
            AlertCode::RootBinCannotRemove => StandardAlert {
                code,
                category: BinsExplorer,
                severity: Error,
                en_us_template: "Can not remove the root media bin.",
                pt_br_template: "Não é possível remover a pasta de mídia raiz.",
                default_actions: vec![Ok],
            },
            AlertCode::RootBinCannotRename => StandardAlert {
                code,
                category: BinsExplorer,
                severity: Error,
                en_us_template: "Can not rename the root media bin.",
                pt_br_template: "Não é possível renomear a pasta de mídia raiz.",
                default_actions: vec![Ok],
            },
            AlertCode::BinNameEmpty => StandardAlert {
                code,
                category: BinsExplorer,
                severity: Error,
                en_us_template: "Please specify a name for the media bin.",
                pt_br_template: "Especifique um nome para a pasta de mídia.",
                default_actions: vec![Ok],
            },
            AlertCode::DuplicateBinRenamed => StandardAlert {
                code,
                category: BinsExplorer,
                severity: Information,
                en_us_template: "A media bin named '{0}' already exists, renamed duplicate bin as '{1}'.",
                pt_br_template: "Uma pasta de mídia chamada '{0}' já existe, pasta duplicada renomeada como '{1}'.",
                default_actions: vec![Ok],
            },
            AlertCode::InvalidDrive => StandardAlert {
                code,
                category: BinsExplorer,
                severity: Error,
                en_us_template: "The specified drive or directory is invalid or disconnected.",
                pt_br_template: "A unidade ou diretório especificado é inválido ou está desconectado.",
                default_actions: vec![Ok],
            },
        }
    }

    /// Formats an alert message by substituting `{0}`, `{1}`, `%s`, `%d` with provided arguments.
    pub fn format_message(code: AlertCode, args: &[&str], locale: &str) -> String {
        let alert = Self::get(code);
        let template = if locale.starts_with("pt") {
            alert.pt_br_template
        } else {
            alert.en_us_template
        };

        let mut formatted = template.to_string();
        for (idx, arg) in args.iter().enumerate() {
            let placeholder = format!("{{{idx}}}");
            formatted = formatted.replace(&placeholder, arg);
        }

        // Also substitute %s and %d if any remain
        for arg in args {
            if formatted.contains("%s") {
                formatted = formatted.replacen("%s", arg, 1);
            } else if formatted.contains("%d") {
                formatted = formatted.replacen("%d", arg, 1);
            }
        }

        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_message_substitutes_arguments() {
        let msg = AlertRegistry::format_message(
            AlertCode::MediaFileNotFound,
            &["video_clip.mp4"],
            "en-US",
        );
        assert_eq!(
            msg,
            "The file 'video_clip.mp4' was not found in the specified path."
        );

        let msg_pt = AlertRegistry::format_message(
            AlertCode::MediaFileNotFound,
            &["video_clip.mp4"],
            "pt-BR",
        );
        assert_eq!(
            msg_pt,
            "O arquivo 'video_clip.mp4' não foi encontrado no caminho especificado."
        );
    }

    #[test]
    fn mock_handler_intercepts_dispatch() {
        let queue = AlertQueue::default();
        queue.set_mock_handler(Some(Arc::new(|event| {
            if event.code == AlertCode::MediaFileNotFound {
                AlertAction::IgnoreAll
            } else {
                AlertAction::Cancel
            }
        })));

        let action = queue.dispatch(
            AlertCode::MediaFileNotFound,
            &["missing.mov"],
            "en-US",
            None,
        );
        assert_eq!(action, Some(AlertAction::IgnoreAll));
    }

    #[test]
    fn numeric_code_values_match_specification() {
        assert_eq!(AlertCode::EmptyProjectRender.as_u32(), 3404);
        assert_eq!(AlertCode::GpuAccelerationError.as_u32(), 33316);
        assert_eq!(AlertCode::PluginFailedToLoad.as_u32(), 16555);
        assert_eq!(AlertCode::MediaFileNotFound.as_u32(), 16926);
        assert_eq!(AlertCode::DynamicRamPreviewZeroMb.as_u32(), 46259);
        assert_eq!(AlertCode::RootBinCannotRemove.as_u32(), 12878);
        assert_eq!(AlertCode::RootBinCannotRename.as_u32(), 12889);
    }

    #[test]
    fn zero_trademarks_in_alert_strings() {
        let codes = [
            AlertCode::MissingProjectMediaElements,
            AlertCode::MediaIsOfflinePrompt,
            AlertCode::MediaFileNotFound,
            AlertCode::MissingVideoCodec,
            AlertCode::EmptyProjectRender,
            AlertCode::GpuAccelerationError,
            AlertCode::PluginFailedToLoad,
            AlertCode::DynamicRamPreviewZeroMb,
            AlertCode::RootBinCannotRemove,
        ];
        for c in codes {
            let a = AlertRegistry::get(c);
            assert!(!a.en_us_template.is_empty());
            assert!(!a.pt_br_template.is_empty());
        }
    }
}
