//! Unit tests for material tokens, appearance resolution, and pipeline creation.

use crate::gpu::GpuContext;
use crate::material::pipeline::MaterialPipeline;
use crate::material::tokens::{Appearance, ColorScheme, MaterialTokens};
use crate::material::MaterialRole;

#[test]
fn normal_appearance_resolves_role_optics() {
    let dark_app = Appearance {
        color_scheme: ColorScheme::Dark,
        reduce_transparency: false,
        increase_contrast: false,
        reduce_motion: false,
    };

    let hud = MaterialTokens::resolve(MaterialRole::Hud, &dark_app);
    assert_eq!(hud.blur_radius, 28.0);
    assert_eq!(hud.specular_rim, 1.0);
    assert!(!hud.is_opaque);

    let panel = MaterialTokens::resolve(MaterialRole::Panel, &dark_app);
    assert_eq!(panel.blur_radius, 20.0);
    assert_eq!(panel.specular_rim, 0.75);
    assert!(!panel.is_opaque);

    let accent = MaterialTokens::resolve(MaterialRole::Accent, &dark_app);
    assert_eq!(accent.blur_radius, 0.0);
    assert!(accent.is_opaque);
}

#[test]
fn reduced_transparency_forces_opaque_tokens() {
    let app = Appearance {
        reduce_transparency: true,
        ..Default::default()
    };

    for role in [
        MaterialRole::Hud,
        MaterialRole::Panel,
        MaterialRole::Sidebar,
        MaterialRole::Control,
        MaterialRole::Tooltip,
        MaterialRole::Menu,
    ] {
        let tokens = MaterialTokens::resolve(role, &app);
        assert!(tokens.is_opaque, "Role {:?} should be opaque", role);
        assert_eq!(tokens.blur_radius, 0.0);
        assert_eq!(tokens.opacity, 1.0);
    }
}

#[test]
fn high_contrast_forces_opaque_and_widens_edges() {
    let app = Appearance {
        increase_contrast: true,
        ..Default::default()
    };

    let tokens = MaterialTokens::resolve(MaterialRole::Panel, &app);
    assert!(tokens.is_opaque);
    assert_eq!(tokens.blur_radius, 0.0);
    assert_eq!(tokens.edge_width, 2.0, "High contrast requires 2px edge");
}

#[test]
fn material_pipeline_creation_smoke() {
    let ctx = pollster::block_on(GpuContext::init());
    match ctx {
        Ok(context) => {
            let pipeline =
                MaterialPipeline::new(context.device(), wgpu::TextureFormat::Bgra8UnormSrgb);
            assert!(pipeline.vertex_buffer().size() > 0);
        }
        Err(err) => {
            log::warn!(
                "Headless GPU context unavailable for material pipeline test: {:?}",
                err
            );
        }
    }
}
