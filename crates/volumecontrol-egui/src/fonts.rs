//! Font installation: Vietnamese coverage on every platform.
//!
//! egui's bundled default font has no Vietnamese precomposed glyphs, so the
//! labels render as tofu. The host loads the platform's UI font (Segoe UI on
//! Windows, Helvetica/Arial on macOS, DejaVu/Noto on Linux) and installs it as
//! the primary proportional face, keeping the default font as a fallback.

use std::path::PathBuf;

use eframe::egui;

/// Install the platform UI font plus the icon font into the egui context.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    if let Some((name, data)) = load_system_font() {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(data));
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.insert(0, name.clone());
        }
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            family.push(name);
        }
    } else {
        log::warn!("no Vietnamese-capable system font found; default glyph coverage only");
    }

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
}

fn load_system_font() -> Option<(String, egui::FontData)> {
    for path in candidates() {
        match std::fs::read(&path) {
            Ok(bytes) => {
                log::info!("using UI font {}", path.display());
                return Some((
                    path.display().to_string(),
                    egui::FontData::from_owned(bytes),
                ));
            }
            Err(_) => continue,
        }
    }
    None
}

fn candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let fonts = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        vec![
            fonts.join("segoeui.ttf"),
            fonts.join("arial.ttf"),
            fonts.join("tahoma.ttf"),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
            PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
            PathBuf::from("/Library/Fonts/Arial.ttf"),
            PathBuf::from("/System/Library/Fonts/Helvetica.ttc"),
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"),
        ]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}
