//! Bridge from [`volumectl::host_core::EventSink`] into the eframe loop.
//!
//! The core publishes from its own thread; every event is cloned into an
//! [`EguiEvent`] and sent over an mpsc channel, then the egui context is asked
//! for a repaint so the main loop drains it promptly.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use volumectl::audio::VolumeState;
use volumectl::config::Config;
use volumectl::host_core::{AudioSessionInfo, BackendStatus, EventSink};
use volumectl::hotkeys::HotkeyRegResult;
use volumectl::ui::SurfaceId;

/// A host notification waiting for the eframe main loop.
#[derive(Debug, Clone)]
pub enum EguiEvent {
    /// Confirmed volume/mute change.
    Volume { percent: u8, muted: bool },
    /// Hotkey registration status for every action.
    Hotkeys(Vec<HotkeyRegResult>),
    /// Audio-session list changed.
    Sessions(Vec<AudioSessionInfo>),
    /// Audio backend health changed.
    Backend(BackendStatus),
    /// Show a surface.
    Open(SurfaceId),
    /// Hide a surface.
    Close(SurfaceId),
    /// Toggle a surface.
    Toggle(SurfaceId),
    /// Show a HUD payload (volume toast or short text card).
    Overlay {
        text: Option<String>,
        state: VolumeState,
        config: Box<Config>,
    },
    /// The user opened the tray menu.
    ShowTrayMenu,
    /// The user asked to quit.
    Exit,
}

/// Map a legacy surface label (`"window-mixer"`) to a [`SurfaceId`].
///
/// Unknown labels return `None`: a host contract addition must not panic the
/// UI thread.
#[must_use]
pub fn surface_for_label(label: &str) -> Option<SurfaceId> {
    match label {
        "window-mixer" => Some(SurfaceId::Mixer),
        "window-settings" => Some(SurfaceId::Settings),
        "window-help" => Some(SurfaceId::Help),
        _ => None,
    }
}

/// EventSink implementation that feeds the eframe loop.
pub struct EguiSink {
    sender: Sender<EguiEvent>,
    context: egui::Context,
}

impl EguiSink {
    /// Create the sink and the receiver the eframe app drains every frame.
    #[must_use]
    pub fn new(context: egui::Context) -> (Arc<Self>, Receiver<EguiEvent>) {
        let (sender, receiver) = mpsc::channel();
        (Arc::new(Self { sender, context }), receiver)
    }

    fn publish(&self, event: EguiEvent) {
        if self.sender.send(event).is_ok() {
            self.context.request_repaint();
        }
    }
}

impl EventSink for EguiSink {
    fn volume(&self, pct: u8, muted: bool) {
        self.publish(EguiEvent::Volume {
            percent: pct,
            muted,
        });
    }

    fn hotkeys(&self, status: &[HotkeyRegResult]) {
        self.publish(EguiEvent::Hotkeys(status.to_vec()));
    }

    fn sessions(&self, sessions: &[AudioSessionInfo]) {
        self.publish(EguiEvent::Sessions(sessions.to_vec()));
    }

    fn backend(&self, status: &BackendStatus) {
        self.publish(EguiEvent::Backend(status.clone()));
    }

    fn open_surface(&self, label: &str) {
        if let Some(surface) = surface_for_label(label) {
            self.publish(EguiEvent::Open(surface));
        }
    }

    fn close_surface(&self, label: &str) {
        if let Some(surface) = surface_for_label(label) {
            self.publish(EguiEvent::Close(surface));
        }
    }

    fn toggle_surface(&self, label: &str) {
        if let Some(surface) = surface_for_label(label) {
            self.publish(EguiEvent::Toggle(surface));
        }
    }

    fn overlay(&self, text: Option<String>, state: VolumeState, config: Config) {
        self.publish(EguiEvent::Overlay {
            text,
            state,
            config: Box::new(config),
        });
    }

    fn show_tray_menu(&self) {
        self.publish(EguiEvent::ShowTrayMenu);
    }

    fn exit(&self) {
        self.publish(EguiEvent::Exit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_forwards_surface_events_and_skips_unknown_labels() {
        let (sink, receiver) = EguiSink::new(egui::Context::default());

        sink.open_surface("window-mixer");
        sink.close_surface("window-settings");
        sink.toggle_surface("window-help");
        sink.open_surface("window-unknown");

        assert!(matches!(
            receiver.try_recv(),
            Ok(EguiEvent::Open(SurfaceId::Mixer))
        ));
        assert!(matches!(
            receiver.try_recv(),
            Ok(EguiEvent::Close(SurfaceId::Settings))
        ));
        assert!(matches!(
            receiver.try_recv(),
            Ok(EguiEvent::Toggle(SurfaceId::Help))
        ));
        assert!(receiver.try_recv().is_err());
    }
}
