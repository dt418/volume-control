//! Update-card phase machine (legacy `UpdateCard` parity).

use crate::i18n::{tr, Key};
use volumectl::config::Language;

/// Visible update phase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    Available {
        version: String,
    },
    Downloading {
        percent: u8,
    },
    Ready {
        version: String,
    },
    Installing {
        version: String,
    },
    UpToDate,
    Failed {
        message: String,
    },
    ManagedExternally,
}

/// Primary user action offered by the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    Check,
    Download,
    ConfirmInstall,
    Retry,
}

impl UpdatePhase {
    /// Primary action for the current phase, if any.
    #[must_use]
    pub fn action(&self) -> Option<UpdateAction> {
        match self {
            UpdatePhase::Idle | UpdatePhase::UpToDate | UpdatePhase::Available { .. } => {
                Some(UpdateAction::Check)
            }
            UpdatePhase::Failed { .. } => Some(UpdateAction::Retry),
            UpdatePhase::Ready { .. } => Some(UpdateAction::ConfirmInstall),
            UpdatePhase::Checking
            | UpdatePhase::Downloading { .. }
            | UpdatePhase::Installing { .. }
            | UpdatePhase::ManagedExternally => None,
        }
    }

    /// Localized status line.
    #[must_use]
    pub fn status_text(&self, language: Language, current_version: &str) -> String {
        match self {
            UpdatePhase::Idle => format!(
                "{} v{current_version}",
                tr(Key::SettingsUpdatesCurrent, language, &[])
            ),
            UpdatePhase::Checking => tr(Key::SettingsUpdatesChecking, language, &[]).to_string(),
            UpdatePhase::Available { version } => {
                tr(Key::SettingsUpdatesAvailable, language, &[version]).to_string()
            }
            UpdatePhase::Downloading { percent } => format!(
                "{} {percent}%",
                tr(Key::SettingsUpdatesDownloading, language, &[])
            ),
            UpdatePhase::Ready { version } => {
                tr(Key::SettingsUpdatesReady, language, &[version]).to_string()
            }
            UpdatePhase::Installing { version } => {
                tr(Key::SettingsUpdatesConfirmBody, language, &[version]).to_string()
            }
            UpdatePhase::UpToDate => tr(Key::SettingsUpdatesUpToDate, language, &[]).to_string(),
            UpdatePhase::Failed { message } => {
                tr(Key::SettingsUpdatesInstallFailed, language, &[message]).to_string()
            }
            UpdatePhase::ManagedExternally => {
                tr(Key::SettingsUpdatesManagedExternally, language, &[]).to_string()
            }
        }
    }
}

/// Update-card state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateCard {
    pub phase: UpdatePhase,
    pub current_version: String,
}

impl UpdateCard {
    #[must_use]
    pub fn new(current_version: impl Into<String>) -> Self {
        Self {
            phase: UpdatePhase::Idle,
            current_version: current_version.into(),
        }
    }

    pub fn check_started(&mut self) {
        self.phase = UpdatePhase::Checking;
    }

    /// Finish a check: `Some(version)` means a newer release exists.
    pub fn check_finished(&mut self, newer: Option<String>) {
        self.phase = match newer {
            Some(version) => UpdatePhase::Available { version },
            None => UpdatePhase::UpToDate,
        };
    }

    pub fn download_progress(&mut self, percent: u8) {
        self.phase = UpdatePhase::Downloading {
            percent: percent.min(100),
        };
    }

    pub fn download_ready(&mut self, version: impl Into<String>) {
        self.phase = UpdatePhase::Ready {
            version: version.into(),
        };
    }

    pub fn install_started(&mut self, version: impl Into<String>) {
        self.phase = UpdatePhase::Installing {
            version: version.into(),
        };
    }

    pub fn failed(&mut self, message: impl Into<String>) {
        self.phase = UpdatePhase::Failed {
            message: message.into(),
        };
    }

    pub fn managed_externally(&mut self) {
        self.phase = UpdatePhase::ManagedExternally;
    }

    /// Dismiss the available/downloading/ready state (legacy "Để sau").
    pub fn dismiss(&mut self) {
        self.phase = UpdatePhase::Idle;
    }
}
