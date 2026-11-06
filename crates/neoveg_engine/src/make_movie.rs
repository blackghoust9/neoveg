//! Make Movie Guided Export Wizard (results.md §8.1; Dialogs 804, 885, 881, 861).
//!
//! Provides the novice-friendly 3-step export wizard flow, preset recommendations,
//! disk space calculations, overwrite confirmations, and empty-project validation (Alert ID 3404).

use neoveg_common::alerts::AlertCode;
use neoveg_common::project::ProjectSettings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Destination target selected in Step 1 (Dialog 804/885).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportDestination {
    WebSharing,
    PortableDevice,
    HardDrive,
    DiscAuthoring,
}

impl ExportDestination {
    pub const ALL: [ExportDestination; 4] = [
        ExportDestination::WebSharing,
        ExportDestination::PortableDevice,
        ExportDestination::HardDrive,
        ExportDestination::DiscAuthoring,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExportDestination::WebSharing => "Upload to Web Video Service",
            ExportDestination::PortableDevice => "Save to Camera or Portable Device",
            ExportDestination::HardDrive => "Save to Hard Drive",
            ExportDestination::DiscAuthoring => "Burn to DVD or Blu-ray Disc",
        }
    }
}

/// Render settings configured in Step 2 (Dialog 881).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MakeMovieSettings {
    pub destination: ExportDestination,
    pub template_name: String,
    pub output_path: PathBuf,
    pub render_loop_region_only: bool,
    pub stretch_video_to_fill: bool,
    pub save_project_markers: bool,
    pub estimated_size_mb: u64,
}

/// Wizard step state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WizardStep {
    SelectDestination,
    ConfigureSettings(MakeMovieSettings),
    Rendering {
        progress_fraction: f32,
        eta_seconds: f64,
    },
    Completed {
        output_path: PathBuf,
    },
    Failed {
        error_code: AlertCode,
        message: String,
    },
}

/// Make Movie Wizard workflow engine.
#[derive(Debug, Clone)]
pub struct MakeMovieWizard {
    step: WizardStep,
    project_settings: ProjectSettings,
    has_timeline_events: bool,
    timeline_duration_seconds: f64,
}

impl MakeMovieWizard {
    pub fn new(
        project_settings: &ProjectSettings,
        has_timeline_events: bool,
        timeline_duration_seconds: f64,
    ) -> Self {
        Self {
            step: WizardStep::SelectDestination,
            project_settings: project_settings.clone(),
            has_timeline_events,
            timeline_duration_seconds,
        }
    }

    pub fn current_step(&self) -> &WizardStep {
        &self.step
    }

    /// Step 1: Destination selection transition.
    ///
    /// Validates project emptiness: if no timeline events or duration is 0,
    /// aborts immediately with Alert ID 3404 (EmptyProjectRender - Edge Case 23).
    pub fn choose_destination(&mut self, dest: ExportDestination) -> Result<(), AlertCode> {
        if !self.has_timeline_events || self.timeline_duration_seconds <= 0.0 {
            self.step = WizardStep::Failed {
                error_code: AlertCode::EmptyProjectRender,
                message: "Unable to render since project is empty.".into(),
            };
            return Err(AlertCode::EmptyProjectRender);
        }

        let recommended_template = self.recommend_preset(dest);
        let default_output = self.default_output_path(dest, &recommended_template);
        let estimated_size = self.estimate_file_size_mb(dest, self.timeline_duration_seconds);

        let settings = MakeMovieSettings {
            destination: dest,
            template_name: recommended_template,
            output_path: default_output,
            render_loop_region_only: false,
            stretch_video_to_fill: false,
            save_project_markers: false,
            estimated_size_mb: estimated_size,
        };

        self.step = WizardStep::ConfigureSettings(settings);
        Ok(())
    }

    /// Step 2: Validates settings and disk space before rendering.
    ///
    /// Checks if destination file already exists (Alert ID 1911 - Edge Case 25).
    /// Checks available disk space against estimated size (Alert ID 17603 - Edge Case 24).
    pub fn validate_and_start_render(
        &mut self,
        settings: MakeMovieSettings,
        available_disk_mb: u64,
        allow_overwrite: bool,
    ) -> Result<(), AlertCode> {
        if settings.output_path.exists() && !allow_overwrite {
            return Err(AlertCode::RenderFileAlreadyExists);
        }

        if settings.estimated_size_mb > available_disk_mb {
            return Err(AlertCode::InsufficientDiskSpace);
        }

        self.step = WizardStep::Rendering {
            progress_fraction: 0.0,
            eta_seconds: 0.0,
        };

        Ok(())
    }

    pub fn update_render_progress(&mut self, fraction: f32, eta_seconds: f64) {
        if let WizardStep::Rendering { .. } = self.step {
            self.step = WizardStep::Rendering {
                progress_fraction: fraction.clamp(0.0, 1.0),
                eta_seconds,
            };
        }
    }

    pub fn complete_render(&mut self, output_path: PathBuf) {
        self.step = WizardStep::Completed { output_path };
    }

    pub fn cancel_render(&mut self) -> Result<(), AlertCode> {
        self.step = WizardStep::SelectDestination;
        Ok(())
    }

    /// Recommends optimal preset based on destination and project settings.
    pub fn recommend_preset(&self, dest: ExportDestination) -> String {
        match dest {
            ExportDestination::WebSharing => {
                if self.project_settings.height <= 720 {
                    "Internet 720-30p".into()
                } else {
                    "Internet 1080-30p".into()
                }
            }
            ExportDestination::PortableDevice => "Internet 720-25p".into(),
            ExportDestination::HardDrive => {
                if self.project_settings.width >= 3840 {
                    "4K UHD 29,97".into()
                } else {
                    "Full HD 1080p 29,97".into()
                }
            }
            ExportDestination::DiscAuthoring => {
                if self.project_settings.fps > 28.0 {
                    "DVD NTSC widescreen".into()
                } else {
                    "DVD PAL widescreen".into()
                }
            }
        }
    }

    fn default_output_path(&self, _dest: ExportDestination, template: &str) -> PathBuf {
        let base_name = if self.project_settings.title.is_empty() {
            "RenderedMovie"
        } else {
            &self.project_settings.title
        };
        let ext = if template.contains("DVD") {
            "mpg"
        } else {
            "mp4"
        };
        PathBuf::from(format!("{base_name}.{ext}"))
    }

    fn estimate_file_size_mb(&self, dest: ExportDestination, duration_sec: f64) -> u64 {
        // Average megabits per second by target destination
        let mbps = match dest {
            ExportDestination::WebSharing => 12.0,
            ExportDestination::PortableDevice => 6.0,
            ExportDestination::HardDrive => 25.0,
            ExportDestination::DiscAuthoring => 8.0,
        };
        let total_bits = duration_sec * mbps * 1_000_000.0;
        let total_bytes = total_bits / 8.0;
        (total_bytes / (1024.0 * 1024.0)).ceil() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_project_aborts_with_alert_3404() {
        let settings = ProjectSettings::default();
        let mut wizard = MakeMovieWizard::new(&settings, false, 0.0);
        let res = wizard.choose_destination(ExportDestination::WebSharing);
        assert_eq!(res, Err(AlertCode::EmptyProjectRender));
        assert!(matches!(
            wizard.current_step(),
            WizardStep::Failed {
                error_code: AlertCode::EmptyProjectRender,
                ..
            }
        ));
    }

    #[test]
    fn insufficient_disk_space_yields_alert() {
        let settings = ProjectSettings::default();
        let mut wizard = MakeMovieWizard::new(&settings, true, 300.0);
        wizard
            .choose_destination(ExportDestination::HardDrive)
            .unwrap();

        if let WizardStep::ConfigureSettings(cfg) = wizard.current_step().clone() {
            // Provide 0 MB free space
            let res = wizard.validate_and_start_render(cfg, 0, false);
            assert_eq!(res, Err(AlertCode::InsufficientDiskSpace));
        } else {
            panic!("Expected ConfigureSettings step");
        }
    }

    #[test]
    fn successful_flow_to_completion() {
        let settings = ProjectSettings::default();
        let mut wizard = MakeMovieWizard::new(&settings, true, 60.0);
        wizard
            .choose_destination(ExportDestination::WebSharing)
            .unwrap();

        let cfg = match wizard.current_step() {
            WizardStep::ConfigureSettings(c) => c.clone(),
            _ => panic!("wrong step"),
        };

        wizard.validate_and_start_render(cfg, 10_000, true).unwrap();
        assert!(matches!(
            wizard.current_step(),
            WizardStep::Rendering { .. }
        ));

        let out = PathBuf::from("movie.mp4");
        wizard.complete_render(out.clone());
        assert_eq!(
            wizard.current_step(),
            &WizardStep::Completed { output_path: out }
        );
    }
}
