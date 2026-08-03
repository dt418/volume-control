//! Shared, platform-neutral UI contracts.
//!
//! Platform-specific primitives live under [`platform`] and are re-exported
//! only on Windows; the native renderer contract in [`renderer`] is shared
//! by every platform adapter.

mod capabilities;
mod model;
mod renderer;
mod settings;
mod signal_rail;
mod surface;
mod theme;

pub mod platform;

pub use capabilities::{resolve_material, resolve_motion, ResolvedMaterial, UiCapabilities};
pub use model::{
    AccentMode, AppAction, AppState, MaterialMode, MotionMode, SurfaceId, SurfaceVisibility,
    SurfaceVisibilityState, ThemeMode, UiStatus,
};
pub use renderer::{HostHandle, NativeRenderer};
pub use settings::SettingsDraft;
pub use signal_rail::{
    rail_apply_key, rail_geometry, MarkerGeometry, RailKey, SignalRail, SignalRailGeometry,
    TrackRect,
};
pub use surface::{
    place_centered, place_mixer_above_overlay, place_overlay, SurfaceRect, SurfaceSize, WorkArea,
};
pub use theme::{
    accent_color, accent_color_for, tokens_for, AnimationTokens, EasingPolicy, ElevationTokens,
    ErrorTokens, FocusTokens, HitTargetTokens, MaterialIntent, RadiusTokens, Rgba,
    SignalGlassTokens, SpacingTokens, TextRole, ThemeTokens, TypographyTokens,
    VolumeThresholdColors,
};

/// Windows rendering primitives (compile-gated; absent on other targets).
#[cfg(target_os = "windows")]
pub use platform::primitives;
