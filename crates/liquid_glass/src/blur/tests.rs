//! Unit tests for blur pyramid cache lifecycle and GPU pipeline initialization.

use crate::blur::pipeline::BlurPipeline;
use crate::blur::pyramid::BlurPyramid;
use crate::gpu::GpuContext;

#[test]
fn blur_pyramid_cache_key_invalidation() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let mut pyramid = BlurPyramid::new(
                context.device(),
                wgpu::TextureFormat::Bgra8UnormSrgb,
                800,
                600,
            );

            // Initially dirty
            assert!(pyramid.needs_rebuild(1, 800, 600, 1.0));

            // Mark clean for revision 1 at scale 1.0
            pyramid.mark_clean(1, 800, 600, 1.0);
            assert!(
                !pyramid.needs_rebuild(1, 800, 600, 1.0),
                "Should hit blur cache when unchanged"
            );

            // Changed revision triggers rebuild
            assert!(
                pyramid.needs_rebuild(2, 800, 600, 1.0),
                "Content revision change must rebuild blur"
            );

            // Resized window triggers rebuild
            assert!(
                pyramid.needs_rebuild(1, 1024, 768, 1.0),
                "Window resize must rebuild blur"
            );

            // Scale factor change triggers rebuild
            assert!(
                pyramid.needs_rebuild(1, 800, 600, 1.25),
                "DPI scale change must rebuild blur"
            );

            // Explicit invalidate triggers rebuild
            pyramid.invalidate();
            assert!(pyramid.needs_rebuild(1, 800, 600, 1.0));
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for blur pyramid test: {:?}",
                err
            );
        }
    }
}

#[test]
fn blur_pyramid_downsampled_dimensions() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let pyramid = BlurPyramid::new(
                context.device(),
                wgpu::TextureFormat::Bgra8UnormSrgb,
                800,
                600,
            );

            assert_eq!(pyramid.half.width, 400);
            assert_eq!(pyramid.half.height, 300);

            assert_eq!(pyramid.quarter.width, 200);
            assert_eq!(pyramid.quarter.height, 150);

            assert_eq!(pyramid.eighth.width, 100);
            assert_eq!(pyramid.eighth.height, 75);
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for blur dimensions test: {:?}",
                err
            );
        }
    }
}

#[test]
fn blur_pipeline_init_smoke() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let _pipeline =
                BlurPipeline::new(context.device(), wgpu::TextureFormat::Bgra8UnormSrgb);
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for blur pipeline test: {:?}",
                err
            );
        }
    }
}

#[test]
fn blur_pipeline_execution_encoding_test() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let format = wgpu::TextureFormat::Bgra8UnormSrgb;
            let pipeline = BlurPipeline::new(context.device(), format);
            let pyramid = BlurPyramid::new(context.device(), format, 800, 600);

            let source_texture = context.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("test_content_source"),
                size: wgpu::Extent3d {
                    width: 800,
                    height: 600,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let source_view = source_texture.create_view(&Default::default());

            let mut encoder =
                context
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("test_blur_encoder"),
                    });

            pipeline.execute(context.device(), &mut encoder, &pyramid, &source_view);
            let cmd_buffer = encoder.finish();
            context.queue().submit(std::iter::once(cmd_buffer));
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for blur execute test: {:?}",
                err
            );
        }
    }
}
