//! Unit tests for native text layout, Vietnamese shaping, and ellipsis truncation.

use glyphon::cosmic_text::FontSystem;

use crate::gpu::GpuContext;
use crate::text::layout::{TextLayout, TextProperties};
use crate::text::system::TextSystem;

#[test]
fn english_text_layout_and_measurement() {
    let mut font_system = FontSystem::new();
    let props = TextProperties::default();
    let buffer = TextLayout::create_buffer(&mut font_system, "Master Volume", &props, None);
    let (width, height) = TextLayout::measure_buffer(&buffer);

    assert!(width > 0.0, "Expected positive width, got {}", width);
    assert!(height > 0.0, "Expected positive height, got {}", height);
}

#[test]
fn vietnamese_diacritics_shaping() {
    let mut font_system = FontSystem::new();
    let props = TextProperties::default();

    let phrases = [
        "Âm lượng tổng thể",
        "Tắt tiếng",
        "Cài đặt giao diện",
        "Thiết bị đầu ra mặc định",
    ];

    for phrase in phrases {
        let buffer = TextLayout::create_buffer(&mut font_system, phrase, &props, None);
        let (width, height) = TextLayout::measure_buffer(&buffer);

        assert!(
            width > 0.0,
            "Failed shaping for Vietnamese phrase '{}'",
            phrase
        );
        assert!(height > 0.0);
    }
}

#[test]
fn ellipsis_truncation_limits_width() {
    let mut font_system = FontSystem::new();
    let props = TextProperties::default();
    let long_device_name = "Speakers (Realtek High Definition Audio Endpoint Device)";

    // Short limit forcing truncation
    let max_width = 80.0;
    let truncated =
        TextLayout::truncate_ellipsis(&mut font_system, long_device_name, &props, max_width);

    assert!(
        truncated.ends_with("..."),
        "Expected trailing ellipsis, got: '{}'",
        truncated
    );

    let truncated_buffer = TextLayout::create_buffer(&mut font_system, &truncated, &props, None);
    let (truncated_w, _) = TextLayout::measure_buffer(&truncated_buffer);

    assert!(
        truncated_w <= max_width,
        "Truncated width {} exceeded limit {}",
        truncated_w,
        max_width
    );

    // Generous limit retaining full string
    let full = TextLayout::truncate_ellipsis(&mut font_system, "Short Name", &props, 500.0);
    assert_eq!(full, "Short Name");
}

#[test]
fn text_system_gpu_pipeline_init_smoke() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let mut text_system = TextSystem::new(
                context.device(),
                context.queue(),
                wgpu::TextureFormat::Bgra8UnormSrgb,
            );

            text_system.resize(context.queue(), 800, 600);
            text_system.trim();
            assert_eq!(
                text_system.resolution(),
                glyphon::Resolution {
                    width: 800,
                    height: 600,
                }
            );
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for text system test: {:?}",
                err
            );
        }
    }
}

#[test]
fn text_system_creation_never_has_zero_resolution() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let text_system = TextSystem::new(
                context.device(),
                context.queue(),
                wgpu::TextureFormat::Bgra8UnormSrgb,
            );
            let resolution = text_system.resolution();
            assert_ne!(resolution.width, 0);
            assert_ne!(resolution.height, 0);
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for text viewport test: {:?}",
                err
            );
        }
    }
}
