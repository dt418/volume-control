//! High-level OS accessibility bridge managing AccessKit adapter lifecycle.

use accesskit::{ActionHandler, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_winit::Adapter;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// High-level OS accessibility bridge managing AccessKit adapter lifecycle.
pub struct A11yAdapter {
    adapter: Adapter,
}

impl A11yAdapter {
    /// Initialize AccessKit adapter for a window with direct action and activation callbacks.
    pub fn new(
        event_loop: &ActiveEventLoop,
        window: &Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        deactivation_handler: impl 'static + DeactivationHandler + Send,
    ) -> Self {
        let adapter = Adapter::with_direct_handlers(
            event_loop,
            window,
            activation_handler,
            action_handler,
            deactivation_handler,
        );
        Self { adapter }
    }

    /// Forward incoming window events to AccessKit for processing.
    pub fn process_event(&mut self, window: &Window, event: &winit::event::WindowEvent) {
        self.adapter.process_event(window, event);
    }

    /// Push an incremental or full accessibility tree update if assistive technology is active.
    pub fn update_if_active(&mut self, factory: impl FnOnce() -> TreeUpdate) {
        self.adapter.update_if_active(factory);
    }
}
