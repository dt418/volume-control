//! Shared, platform-neutral UI contracts.
//!
//! Renderer and host integrations are intentionally kept out of this module.

mod model;

pub use model::{
    AppAction, AppState, MaterialMode, MotionMode, SurfaceId, SurfaceVisibility,
    SurfaceVisibilityState, ThemeMode, UiStatus,
};
