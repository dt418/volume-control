//! Shared, platform-neutral UI contracts.
//!
//! Renderer and host integrations are intentionally kept out of this module.

mod capabilities;
mod model;
mod surface;
mod theme;

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
