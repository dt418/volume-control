//! Shared, platform-neutral UI contracts.
//!
//! Renderer and host integrations are intentionally kept out of this module.

mod model;
mod theme;

pub use model::{
    AccentMode, AppAction, AppState, MaterialMode, MotionMode, SurfaceId, SurfaceVisibility,
    SurfaceVisibilityState, ThemeMode, UiStatus,
};
pub use theme::{
    accent_color, tokens_for, AnimationTokens, EasingPolicy, ElevationTokens, ErrorTokens,
    FocusTokens, HitTargetTokens, MaterialIntent, RadiusTokens, Rgba, SpacingTokens, TextRole,
    ThemeTokens, TypographyTokens, VolumeThresholdColors,
};
