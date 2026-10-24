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
