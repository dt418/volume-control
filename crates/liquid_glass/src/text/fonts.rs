//! Font family selection and cross-platform Vietnamese fallback chains.

/// Default primary UI font family name for the current platform.
pub const fn default_font_family() -> &'static str {
    if cfg!(target_os = "windows") {
        "Segoe UI"
    } else if cfg!(target_os = "macos") {
        "SF Pro Text"
    } else {
        "Cantarell"
    }
}

/// Fallback font families in priority order ensuring complete Vietnamese diacritic coverage.
pub const fn vietnamese_fallback_chain() -> &'static [&'static str] {
    &[
        "Segoe UI",
        "SF Pro Text",
        "Arial",
        "Roboto",
        "Cantarell",
        "DejaVu Sans",
        "Noto Sans",
        "Tahoma",
    ]
}
