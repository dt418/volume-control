//! Native renderer host contract.
//!
//! This module defines the boundary between the platform-neutral host and
//! platform-specific renderer adapters (Win32/Direct2D, AppKit, GTK4). The
//! contract is normative and identical on every platform:
//!
//! - Renderers consume confirmed [`AppState`]. [`NativeRenderer::publish`]
//!   receives `&AppState`, so a renderer cannot mutate host-owned state; the
//!   host republishes whenever authoritative state changes.
//! - User intent becomes an [`AppAction`]. Renderers emit actions through
//!   [`NativeRenderer::dispatch`] on their owning thread, or through a
//!   cloned [`HostHandle`] from any thread; the host routes them.
//! - Renderers never mutate audio, configuration, or hotkey registration
//!   directly. They only enqueue actions and observe published state.
//! - After a mutation the host reads authoritative state (audio readback,
//!   configuration, hotkey status) and republishes it to every renderer.
//! - [`NativeRenderer::destroy`] is the explicit teardown hook. It is safe
//!   to call once; the host calls it when the renderer is retired.

use crate::ui::model::{AppAction, AppState};
use crate::ui::theme::ThemeTokens;
use crate::ui::UiCapabilities;

/// Opaque, thread-safe handle to the application host.
///
/// A renderer keeps one [`HostHandle`] to turn user intent into
/// [`AppAction`] values after its surfaces are created. The host internals
/// are hidden behind a boxed handler, so renderers can only enqueue actions
/// and can never touch audio, configuration, or hotkey registration
/// directly. Handles are cheap to clone and every clone targets the same
/// handler.
#[derive(Clone)]
pub struct HostHandle {
    inner: std::sync::Arc<dyn Fn(AppAction) + Send + Sync>,
}

impl HostHandle {
    /// Wrap a host-side action handler.
    pub fn new(handler: impl Fn(AppAction) + Send + Sync + 'static) -> Self {
        Self {
            inner: std::sync::Arc::new(handler),
        }
    }

    /// Hand an action to the host. The host normalizes and routes it.
    pub fn enqueue(&self, action: AppAction) {
        (self.inner)(action);
    }
}

/// Platform renderer adapter contract.
///
/// Each platform implements this trait once to own lifecycle, event
/// translation, painting, and accessibility integration. Renderers are
/// created with a [`HostHandle`] and a [`UiCapabilities`] snapshot, consume
/// confirmed [`AppState`] via [`Self::publish`], emit [`AppAction`] values
/// via [`Self::dispatch`], and tear down explicitly via [`Self::destroy`].
pub trait NativeRenderer {
    /// Failure type reported when the renderer cannot be created.
    type Error;

    /// Create the renderer, binding it to the host and the current
    /// capabilities. Called exactly once before any other method.
    fn create(host: HostHandle, capabilities: UiCapabilities) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Render the confirmed state with the current tokens and capabilities.
    ///
    /// The host calls this whenever authoritative state changes. The
    /// renderer receives shared references and must treat the state as
    /// read-only.
    fn publish(&mut self, state: &AppState, tokens: &ThemeTokens, capabilities: &UiCapabilities);

    /// Deliver user intent from this renderer to the host.
    fn dispatch(&mut self, action: AppAction);

    /// Explicit teardown hook. Safe to call once; the host calls it when
    /// the renderer is retired.
    fn destroy(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::model::{SurfaceId, ThemeMode, UiStatus};
    use crate::ui::surface::WorkArea;

    fn test_capabilities() -> UiCapabilities {
        UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: 1.25,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    fn test_state() -> AppState {
        let mut state = AppState::from_audio(72, false, Some("Speakers".into()));
        state.theme = ThemeMode::Dark;
        state.status = UiStatus::Ready;
        state.show(SurfaceId::Overlay);
        state
    }

    #[test]
    fn host_handle_delivers_actions_in_order() {
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_for_handler = std::sync::Arc::clone(&received);
        let host = HostHandle::new(move |action: AppAction| {
            received_for_handler
                .lock()
                .expect("handler lock")
                .push(action);
        });

        host.enqueue(AppAction::SetVolumePercent { percent: 55 });
        host.enqueue(AppAction::ToggleMute);
        host.enqueue(AppAction::ShowSurface(SurfaceId::Mixer));

        let guard = received.lock().expect("handler lock");
        assert_eq!(
            *guard,
            vec![
                AppAction::SetVolumePercent { percent: 55 },
                AppAction::ToggleMute,
                AppAction::ShowSurface(SurfaceId::Mixer),
            ]
        );
    }

    #[test]
    fn host_handle_clones_share_the_same_handler() {
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_for_handler = std::sync::Arc::clone(&received);
        let host = HostHandle::new(move |action: AppAction| {
            received_for_handler
                .lock()
                .expect("handler lock")
                .push(action);
        });
        let clone = host.clone();

        clone.enqueue(AppAction::ToggleMute);
        host.enqueue(AppAction::ResetVolume);

        let guard = received.lock().expect("handler lock");
        assert_eq!(*guard, vec![AppAction::ToggleMute, AppAction::ResetVolume]);
    }

    #[test]
    fn renderer_full_lifecycle_publishes_exact_values() {
        #[derive(Default)]
        struct MockRenderer {
            host: Option<HostHandle>,
            created_capabilities: Option<UiCapabilities>,
            publishes: Vec<(AppState, ThemeTokens, UiCapabilities)>,
            dispatches: Vec<AppAction>,
            destroyed: bool,
        }

        impl NativeRenderer for MockRenderer {
            type Error = String;

            fn create(host: HostHandle, capabilities: UiCapabilities) -> Result<Self, Self::Error> {
                Ok(Self {
                    host: Some(host),
                    created_capabilities: Some(capabilities),
                    ..Self::default()
                })
            }

            fn publish(
                &mut self,
                state: &AppState,
                tokens: &ThemeTokens,
                capabilities: &UiCapabilities,
            ) {
                self.publishes.push((state.clone(), *tokens, *capabilities));
            }

            fn dispatch(&mut self, action: AppAction) {
                self.dispatches.push(action);
            }

            fn destroy(&mut self) {
                self.destroyed = true;
            }
        }

        let mut renderer = MockRenderer::create(HostHandle::new(|_| {}), test_capabilities())
            .expect("mock renderer creates");

        let state = test_state();
        let tokens = crate::ui::tokens_for(
            crate::ui::ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        let caps = test_capabilities();

        renderer.publish(&state, &tokens, &caps);
        renderer.dispatch(AppAction::ToggleMute);
        renderer.destroy();

        assert_eq!(
            renderer.created_capabilities,
            Some(test_capabilities()),
            "create receives the exact capabilities passed"
        );
        assert_eq!(
            renderer.publishes,
            vec![(state, tokens, caps)],
            "publish observes the exact state, tokens, and capabilities"
        );
        assert_eq!(renderer.dispatches, vec![AppAction::ToggleMute]);
        assert!(renderer.destroyed);
        assert!(renderer.host.is_some(), "renderer keeps its host handle");
    }

    #[test]
    fn host_handle_passes_raw_actions_and_host_normalizes() {
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_for_handler = std::sync::Arc::clone(&received);
        let host = HostHandle::new(move |action: AppAction| {
            received_for_handler
                .lock()
                .expect("handler lock")
                .push(action);
        });

        host.enqueue(AppAction::SetVolumePercent { percent: 250 });

        let guard = received.lock().expect("handler lock");
        // The renderer must not pre-clamp: the handler sees the raw action.
        assert_eq!(*guard, vec![AppAction::SetVolumePercent { percent: 250 }]);
        // The host-side clamp is available for the host to apply.
        assert_eq!(guard[0].normalized(), Some(100));
    }

    #[test]
    fn renderer_trait_works_as_a_generic_bound() {
        #[derive(Default)]
        struct MockRenderer {
            dispatches: Vec<AppAction>,
            destroyed: bool,
        }

        impl NativeRenderer for MockRenderer {
            type Error = std::convert::Infallible;

            fn create(
                _host: HostHandle,
                _capabilities: UiCapabilities,
            ) -> Result<Self, Self::Error> {
                Ok(Self::default())
            }

            fn publish(
                &mut self,
                _state: &AppState,
                _tokens: &ThemeTokens,
                _capabilities: &UiCapabilities,
            ) {
            }

            fn dispatch(&mut self, action: AppAction) {
                self.dispatches.push(action);
            }

            fn destroy(&mut self) {
                self.destroyed = true;
            }
        }

        fn drive<R: NativeRenderer>(renderer: &mut R) {
            renderer.dispatch(AppAction::ToggleMute);
            renderer.dispatch(AppAction::ResetVolume);
            renderer.destroy();
        }

        let mut renderer = MockRenderer::create(HostHandle::new(|_| {}), test_capabilities())
            .expect("mock renderer creates");
        drive(&mut renderer);

        assert_eq!(
            renderer.dispatches,
            vec![AppAction::ToggleMute, AppAction::ResetVolume]
        );
        assert!(renderer.destroyed);
    }
}
