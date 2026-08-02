//! Shared, platform-neutral UI contracts.
//!
//! Renderer and host integrations are intentionally kept out of this module;
//! platform-specific primitives live under [`platform`] and are re-exported
//! only on Windows.

mod capabilities;
mod model;
mod surface;
mod theme;

pub mod platform;

pub use capabilities::{resolve_material, ResolvedMaterial, UiCapabilities};
pub use model::{
    AccentMode, AppAction, AppState, MaterialMode, MotionMode, SurfaceId, SurfaceVisibility,
    SurfaceVisibilityState, ThemeMode, UiStatus,
};
pub use surface::{
    place_mixer_above_overlay, place_overlay, SurfaceRect, SurfaceSize, WorkArea,
};
pub use theme::{
    accent_color, tokens_for, AnimationTokens, EasingPolicy, ElevationTokens, ErrorTokens,
    FocusTokens, HitTargetTokens, MaterialIntent, RadiusTokens, Rgba, SpacingTokens, TextRole,
    ThemeTokens, TypographyTokens, VolumeThresholdColors,
};

/// Windows rendering primitives (compile-gated; absent on other targets).
#[cfg(target_os = "windows")]
pub use platform::primitives;
