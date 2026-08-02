//! Platform-neutral in-memory edit draft for the Settings surface.
//!
//! The Settings window (Task 10) keeps the user's edits in a [`SettingsDraft`]
//! until host persistence succeeds: edits never touch disk directly, and a
//! failed [`SettingsDraft::commit`] leaves them intact so the user can retry.
//!
//! This module intentionally imports nothing platform-specific and compiles on
//! every target. Persistence, validation, and normalization all live in
//! `crate::config`; this draft is purely a consumer of that contract.

use crate::config::{Config, ConfigError, ConfigValidationError};

/// An in-memory edit buffer layered over the persisted [`Config`].
///
/// - [`original`][Self::original] is the last confirmed baseline (what the host
///   actually has loaded/persisted).
/// - [`current`][Self::current] is the working copy the window edits.
/// - [`dirty`][Self::is_dirty] is `original != current`, recomputed whenever
///   either side moves.
/// - [`error`][Self::error] holds the field-specific validation failure from
///   the most recent [`commit`][Self::commit] attempt, if that attempt failed
///   validation.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsDraft {
    original: Config,
    current: Config,
    dirty: bool,
    error: Option<ConfigValidationError>,
}

impl SettingsDraft {
    /// Seed a draft from a loaded config. Both the baseline and the working
    /// copy start equal, so the draft is clean.
    pub fn new(config: Config) -> Self {
        let original = config.clone();
        Self {
            original,
            current: config,
            dirty: false,
            error: None,
        }
    }

    /// Adopt `config` as the new confirmed baseline.
    ///
    /// Used when the host reports a new authoritative config (for example a
    /// live file reload). Any in-progress edits in [`current`][Self::current]
    /// are preserved and dirtiness is recomputed against the new baseline.
    pub fn replace(&mut self, config: Config) {
        self.original = config;
        self.error = None;
        self.recompute_dirty();
    }

    /// Replace the working copy with the user's latest edits.
    ///
    /// Recomputes dirtiness against the baseline and clears any stale
    /// validation error from an earlier commit attempt.
    pub fn set_current(&mut self, config: Config) {
        self.current = config;
        self.error = None;
        self.recompute_dirty();
    }

    /// Discard edits and restore the baseline. Equivalent to [`cancel`][Self::cancel].
    pub fn reset(&mut self) {
        self.current = self.original.clone();
        self.error = None;
        self.dirty = false;
    }

    /// Discard edits and restore the baseline. Identical to [`reset`][Self::reset]
    /// at the draft level; the window decides whether a cancel also closes.
    pub fn cancel(&mut self) {
        self.reset();
    }

    /// Strictly validate the current working copy without persisting anything.
    ///
    /// This is a pure check (does not mutate the draft's stored error); the
    /// error state is populated only by a failing [`commit`][Self::commit].
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        crate::config::validate(&self.current)
    }

    /// Validate, persist, and adopt the saved config as the new baseline.
    ///
    /// - **Success**: the config returned by persistence (already normalized by
    ///   `crate::config::save_validated`) becomes both the new baseline and the
    ///   working copy; dirtiness and error are cleared.
    /// - **Validation failure**: [`error`][Self::error] is set to the
    ///   field-specific failure and the edits stay intact.
    /// - **I/O or serialization failure**: the error is returned only and
    ///   [`error`][Self::error] is left clear (it is a validation error slot);
    ///   edits stay intact so the user can retry.
    pub fn commit(&mut self) -> Result<Config, ConfigError> {
        self.commit_with(crate::config::save_validated)
    }

    /// True when the working copy differs from the confirmed baseline.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The last confirmed baseline.
    pub fn original(&self) -> &Config {
        &self.original
    }

    /// The working copy the window is editing.
    pub fn current(&self) -> &Config {
        &self.current
    }

    /// The validation failure from the most recent commit attempt, if any.
    pub fn error(&self) -> Option<&ConfigValidationError> {
        self.error.as_ref()
    }

    /// Shared commit logic with an injectable persistence step.
    ///
    /// Private; the public [`commit`][Self::commit] plugs in the real
    /// `crate::config::save_validated`, and tests inject fakes to exercise the
    /// validation-failure and I/O-failure paths without touching disk.
    fn commit_with<F>(&mut self, persist: F) -> Result<Config, ConfigError>
    where
        F: FnOnce(&Config) -> Result<Config, ConfigError>,
    {
        match persist(&self.current) {
            Ok(saved) => {
                // The host now has `saved` in effect, so it is both the new
                // baseline and the working copy.
                self.original = saved.clone();
                self.current = saved;
                self.error = None;
                self.dirty = false;
                Ok(self.current.clone())
            }
            Err(ConfigError::Validation(validation_error)) => {
                // Validation never touched disk; keep every edit so the window
                // can show the offending field and let the user fix it.
                self.error = Some(validation_error.clone());
                Err(ConfigError::Validation(validation_error))
            }
            Err(other) => {
                // Persistence reached the disk and failed. Validation passed,
                // so there is no field-specific error to store; keep edits
                // intact and surface the I/O/serialization error to the caller.
                self.error = None;
                Err(other)
            }
        }
    }

    fn recompute_dirty(&mut self) {
        self.dirty = self.original != self.current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{normalize, ConfigError};

    /// A baseline that every commit attempt must pass strict validation.
    fn valid_config() -> Config {
        Config::default()
    }

    /// A working copy that fails strict validation (small step not strictly
    /// below the large step).
    fn invalid_config() -> Config {
        let mut cfg = Config::default();
        cfg.volume_step = 30;
        cfg.volume_step_large = 29;
        cfg
    }

    /// The last confirmed baseline changed by the caller.
    fn different_baseline() -> Config {
        let mut cfg = Config::default();
        cfg.overlay_duration_ms = 3000;
        cfg
    }

    #[test]
    fn new_is_clean_with_matching_original_and_current() {
        let config = valid_config();
        let draft = SettingsDraft::new(config.clone());

        assert_eq!(draft.original(), &config);
        assert_eq!(draft.current(), &config);
        assert!(!draft.is_dirty());
        assert!(draft.error().is_none());
        assert!(draft.validate().is_ok());
    }

    #[test]
    fn editing_current_marks_dirty_and_reset_clears_it() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;

        draft.set_current(edited.clone());
        assert!(draft.is_dirty());
        assert_eq!(draft.current(), &edited);

        draft.reset();
        assert!(!draft.is_dirty());
        assert_eq!(draft.current(), draft.original());
        assert!(draft.error().is_none());
    }

    #[test]
    fn cancel_discards_edits_and_clears_error_like_reset() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;
        draft.set_current(edited);
        draft.error = Some(ConfigValidationError {
            field: "volume_step",
            message: "stale".into(),
        });

        draft.cancel();

        assert!(!draft.is_dirty());
        assert_eq!(draft.current(), draft.original());
        assert!(draft.error().is_none());
    }

    #[test]
    fn replace_adopts_baseline_and_recomputes_dirty() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;
        draft.set_current(edited.clone());

        // New baseline differs from the in-progress edit -> still dirty.
        draft.replace(different_baseline());
        assert_eq!(draft.original(), &different_baseline());
        assert_eq!(draft.current(), &edited);
        assert!(draft.is_dirty());

        // New baseline matches the in-progress edit -> now clean.
        draft.replace(edited.clone());
        assert_eq!(draft.original(), &edited);
        assert!(!draft.is_dirty());
    }

    #[test]
    fn replace_clears_stale_validation_error() {
        let mut draft = SettingsDraft::new(valid_config());
        draft.set_current(invalid_config());
        draft.error = Some(ConfigValidationError {
            field: "volume_step_large",
            message: "stale".into(),
        });

        draft.replace(different_baseline());

        assert!(draft.error().is_none());
    }

    #[test]
    fn validate_reports_field_specific_error_for_invalid_current() {
        let mut draft = SettingsDraft::new(valid_config());
        draft.set_current(invalid_config());

        let error = draft.validate().expect_err("invalid config must fail");

        assert_eq!(error.field, "volume_step_large");
    }

    #[test]
    fn validate_is_ok_for_valid_current() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;
        draft.set_current(edited);

        assert!(draft.validate().is_ok());
    }

    #[test]
    fn commit_success_adopts_saved_config_and_clears_dirty() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;
        draft.set_current(edited);
        assert!(draft.is_dirty());

        // Injected persistence simulates save_validated: validate then return
        // the normalized config that the host would have persisted.
        let saved = draft
            .commit_with(|cfg| {
                crate::config::validate(cfg).map_err(ConfigError::Validation)?;
                Ok(normalize(cfg.clone()))
            })
            .expect("valid config persists");

        assert_eq!(saved.volume_step, 5);
        // The saved config is adopted as the new baseline *and* working copy.
        assert_eq!(draft.original(), &saved);
        assert_eq!(draft.current(), &saved);
        assert!(!draft.is_dirty());
        assert!(draft.error().is_none());
    }

    #[test]
    fn commit_success_adopts_the_returned_normalized_config() {
        let mut draft = SettingsDraft::new(valid_config());
        // An out-of-bounds-but-normalizable blacklist entry: strict validation
        // passes, but the persisted form is lowercased and trimmed.
        let mut edited = valid_config();
        edited.blacklist.push("  Chrome.EXE  ".into());
        draft.set_current(edited);

        let saved = draft
            .commit_with(|cfg| {
                crate::config::validate(cfg).map_err(ConfigError::Validation)?;
                Ok(normalize(cfg.clone()))
            })
            .expect("valid config persists");

        assert_eq!(saved.blacklist, vec!["chrome.exe".to_string()]);
        assert_eq!(draft.current(), &saved);
        assert!(!draft.is_dirty());
    }

    #[test]
    fn commit_validation_failure_keeps_edits_and_sets_error() {
        let mut draft = SettingsDraft::new(valid_config());
        draft.set_current(invalid_config());
        let before = draft.current().clone();

        // The public commit validates before touching disk, so an invalid
        // draft fails here without any side effects.
        let error = draft.commit().expect_err("invalid config must fail commit");

        assert!(matches!(error, ConfigError::Validation(_)));
        assert_eq!(draft.current(), &before, "edits stay intact");
        assert_eq!(draft.original(), &valid_config(), "baseline untouched");
        assert!(draft.is_dirty());
        let stored = draft.error().expect("validation error is stored");
        assert_eq!(stored.field, "volume_step_large");
    }

    #[test]
    fn commit_io_failure_keeps_edits_and_returns_error_only() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;
        draft.set_current(edited.clone());
        // Plant a stale error; a failed-but-validated attempt must clear it.
        draft.error = Some(ConfigValidationError {
            field: "volume_step",
            message: "stale".into(),
        });
        let before = draft.current().clone();

        let error = draft
            .commit_with(|_| {
                Err(ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "simulated disk failure",
                )))
            })
            .expect_err("IO failure surfaces to the caller");

        assert!(matches!(error, ConfigError::Io(_)));
        assert_eq!(draft.current(), &before, "edits stay intact");
        assert_eq!(draft.original(), &valid_config(), "baseline untouched");
        assert!(draft.is_dirty());
        assert!(draft.error().is_none(), "IO errors are returned, not stored");
    }

    #[test]
    fn commit_serialization_failure_keeps_edits() {
        let mut draft = SettingsDraft::new(valid_config());
        let mut edited = valid_config();
        edited.volume_step = 5;
        draft.set_current(edited.clone());

        let error = draft
            .commit_with(|_| {
                Err(ConfigError::Serialization(serde_json::Error::io(
                    std::io::Error::new(std::io::ErrorKind::Other, "simulated encode failure"),
                )))
            })
            .expect_err("serialization failure surfaces to the caller");

        assert!(matches!(error, ConfigError::Serialization(_)));
        assert_eq!(draft.current(), &edited);
        assert!(draft.is_dirty());
        assert!(draft.error().is_none());
    }
}
