//! Application icon for viewports (taskbar / title bar).

use std::sync::Arc;

use eframe::egui;

/// Load the packaged 128×128 PNG icon.
///
/// Returns `None` (and the default egui icon is used) if decoding fails.
pub fn app_icon() -> Option<Arc<egui::IconData>> {
    static CACHE: std::sync::OnceLock<Option<Arc<egui::IconData>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(decode_icon).clone()
}

fn decode_icon() -> Option<Arc<egui::IconData>> {
    let bytes: &[u8] = include_bytes!("../../../src-tauri/icons/128x128.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => {
            let (pixels, _) = buffer.as_chunks::<3>();
            pixels
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect()
        }
        _ => return None,
    };
    Some(Arc::new(egui::IconData {
        rgba,
        width: info.width,
        height: info.height,
    }))
}

/// Decode a `data:image/png;base64,...` URL coming from `RunningApp.icon`.
///
/// Returns `None` for malformed URLs or decoders that cannot produce RGBA.
#[must_use]
pub fn decode_data_url(url: &str) -> Option<egui::ColorImage> {
    let payload = url.strip_prefix("data:image/png;base64,")?;
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => {
            let (pixels, _) = buffer.as_chunks::<3>();
            pixels
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect()
        }
        _ => return None,
    };
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [info.width as usize, info.height as usize],
        &rgba,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_app_icon_decodes() {
        let icon = app_icon().expect("bundled 128x128 icon decodes");
        assert_eq!(icon.width, 128);
        assert_eq!(icon.height, 128);
        assert_eq!(icon.rgba.len(), 128 * 128 * 4);
    }

    #[test]
    fn decodes_png_data_url_from_running_apps() {
        // 1x1 transparent PNG.
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let image = decode_data_url(url).expect("valid data URL");
        assert_eq!(image.size, [1, 1]);
        assert!(decode_data_url("not-a-data-url").is_none());
        assert!(decode_data_url("data:image/png;base64,!!!").is_none());
    }
}
