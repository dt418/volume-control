//! Host-state-driven orchestration for native Liquid Glass surfaces.

use std::time::Instant;

use volumectl_lib::ui::{AppState, SurfaceId};

use crate::scene::Scene;
use crate::surfaces::{HelpSurface, MixerSurface, OverlaySurface, SettingsSurface};

/// Owns native surface state and converts authoritative host state into scenes.
///
/// This is deliberately window-system independent. A Winit adapter owns windows and
/// WGPU presentation; this controller owns no host state and never bypasses AppAction.
pub struct LiquidGlassSurfaceController {
    overlay: OverlaySurface,
    mixer: MixerSurface,
    settings: SettingsSurface,
    help: HelpSurface,
    last_published: AppState,
}

impl LiquidGlassSurfaceController {
    /// Create all native surface models from initial confirmed host state.
    pub fn new(state: &AppState, now: Instant) -> Self {
        let mut mixer = MixerSurface::new();
        mixer.sync_state(state);

        Self {
            overlay: OverlaySurface::new(now),
            mixer,
            settings: SettingsSurface::new(state),
            help: HelpSurface::new(state.language),
            last_published: state.clone(),
        }
    }

    /// Apply a confirmed host state publication.
    ///
    /// A changed master volume, mute state, or device triggers the volume HUD.
    /// Surface visibility remains host-authoritative.
    pub fn publish(&mut self, state: &AppState, now: Instant, reduce_motion: bool) {
        let volume_changed = state.volume_percent != self.last_published.volume_percent
            || state.muted != self.last_published.muted
            || state.device != self.last_published.device;

        self.mixer.sync_state(state);
        self.settings.sync_confirmed(state);
        self.help.set_language(state.language);

        if volume_changed && state.is_visible(SurfaceId::Overlay) {
            self.overlay.show(
                state.volume_percent,
                state.muted,
                state.device.as_deref(),
                now,
                reduce_motion,
            );
        }

        if state.is_visible(SurfaceId::Help) {
            self.help.open();
        } else {
            self.help.close();
        }

        self.last_published = state.clone();
    }

    /// Show a host-originated text toast, including update check result messages.
    pub fn show_notification(&mut self, text: impl Into<String>, now: Instant) {
        self.overlay.show_notification(text, now);
    }

    /// Build the current Overlay scene.
    pub fn populate_overlay_scene(&self, scene: &mut Scene, now: Instant) {
        self.overlay.populate_scene(scene, now);
    }

    /// Build the current Mixer scene.
    pub fn populate_mixer_scene(&self, scene: &mut Scene) {
        self.mixer.populate_scene(scene);
    }

    /// Build the current Settings scene at window dimensions.
    pub fn populate_settings_scene(&self, scene: &mut Scene, width: f32, height: f32) {
        self.settings.populate_scene(scene, width, height);
    }

    /// Build the current Help scene.
    pub fn populate_help_scene(&self, scene: &mut Scene) {
        self.help.populate_scene(scene);
    }

    pub fn overlay(&self) -> &OverlaySurface {
        &self.overlay
    }

    /// Advance overlay springs and auto-hide timers on the host event loop.
    pub fn update_overlay(&mut self, now: Instant) -> bool {
        self.overlay.update(now)
    }

    pub fn mixer(&self) -> &MixerSurface {
        &self.mixer
    }

    pub fn mixer_mut(&mut self) -> &mut MixerSurface {
        &mut self.mixer
    }
    pub fn settings(&self) -> &SettingsSurface {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut SettingsSurface {
        &mut self.settings
    }

    pub fn help(&self) -> &HelpSurface {
        &self.help
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volumectl_lib::ui::{AppState, SurfaceVisibility};

    use crate::surfaces::{NOTIFICATION_TEXT_ID, OVERLAY_ROOT_ID, PERCENT_TEXT_ID};

    #[test]
    fn changed_host_volume_shows_native_overlay() {
        let now = Instant::now();
        let initial = AppState::default();
        let mut controller = LiquidGlassSurfaceController::new(&initial, now);

        let mut updated = initial;
        updated.volume_percent = 72;
        updated.surfaces.overlay = SurfaceVisibility::Visible;
        controller.publish(&updated, now, false);

        let mut scene = Scene::new();
        controller.populate_overlay_scene(&mut scene, now);
        assert!(scene.get(OVERLAY_ROOT_ID).is_some());
        assert_eq!(
            scene
                .get(PERCENT_TEXT_ID)
                .and_then(|node| node.text.as_deref()),
            Some("72%")
        );
    }

    #[test]
    fn update_result_uses_native_text_toast() {
        let now = Instant::now();
        let state = AppState::default();
        let mut controller = LiquidGlassSurfaceController::new(&state, now);
        controller.show_notification("Update available: v0.2.0", now);

        let mut scene = Scene::new();
        controller.populate_overlay_scene(&mut scene, now);
        assert_eq!(
            scene
                .get(NOTIFICATION_TEXT_ID)
                .and_then(|node| node.text.as_deref()),
            Some("Update available: v0.2.0")
        );
    }
}
