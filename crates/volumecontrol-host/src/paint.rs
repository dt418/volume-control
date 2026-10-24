//! WGPU frame composition: scene primitives -> blur pyramid -> glass pass -> text.

use std::collections::HashMap;

use wgpu::util::DeviceExt;

use liquid_glass::blur::{BlurPipeline, BlurPyramid};
use liquid_glass::gpu::context::GpuContext;
use liquid_glass::gpu::surface::SurfaceManager;
use liquid_glass::material::pipeline::{GlobalUniforms, MaterialPipeline, MaterialUniforms};
use liquid_glass::material::tokens::{Appearance, MaterialTokens};
use liquid_glass::material::MaterialRole;
use liquid_glass::scene::flatten::{RenderPrimitive, SceneFlattener};
use liquid_glass::scene::{Scene, SemanticId};
use liquid_glass::text::layout::{TextLayout, TextProperties};
use liquid_glass::text::system::TextSystem;
use liquid_glass::text::{Color, TextArea, TextBounds};
use volumectl::ui::{MaterialMode, MotionMode, ThemeMode};

/// Glass container roles rendered above the blurred backdrop.
const GLASS_ROLES: [MaterialRole; 4] = [
    MaterialRole::Panel,
    MaterialRole::Hud,
    MaterialRole::Sidebar,
    MaterialRole::Menu,
];

/// Corner radii (logical px) applied per role until scene nodes carry radii.
fn corner_radii_for(role: MaterialRole) -> [f32; 4] {
    match role {
        MaterialRole::Panel | MaterialRole::Menu => [16.0; 4],
        MaterialRole::Hud => [24.0; 4],
        MaterialRole::Sidebar => [12.0; 4],
        _ => [8.0; 4],
    }
}

fn is_glass(role: MaterialRole) -> bool {
    GLASS_ROLES.contains(&role)
}

/// Fullscreen composite quad sampling the blurred backdrop.
struct CompositePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl CompositePipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("host_composite_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/composite.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("host_composite_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("host_composite_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("host_composite_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("host_composite_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            bind_group_layout,
            sampler,
        }
    }

    fn bind_group(&self, view: &wgpu::TextureView, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("host_composite_bind"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

/// Per-window GPU state: swapchain, pipelines, blur pyramid, content target.
pub struct SurfaceView {
    manager: SurfaceManager<'static>,
    text: TextSystem,
    format: wgpu::TextureFormat,
    pyramid: BlurPyramid,
    material: MaterialPipeline,
    blur: BlurPipeline,
    composite: CompositePipeline,
    content_texture: wgpu::Texture,
    content_view: wgpu::TextureView,
    global_buffer: wgpu::Buffer,
    global_bind: wgpu::BindGroup,
    primitive_buffers: Vec<wgpu::Buffer>,
    primitive_binds: Vec<wgpu::BindGroup>,
    text_buffers: HashMap<SemanticId, (String, glyphon::Buffer)>,
    width: u32,
    height: u32,
}

/// Shared frame renderer owning the GPU context and text system.
pub struct FrameRenderer {
    context: GpuContext,
    appearance: Appearance,
    started: std::time::Instant,
}

impl FrameRenderer {
    /// Initialize the GPU stack for the platform backend policy.
    ///
    /// # Errors
    ///
    /// Returns the liquid_glass error when no adapter/device can be created.
    pub fn new(theme: ThemeMode) -> Result<Self, liquid_glass::Error> {
        let context = pollster::block_on(GpuContext::init())?;
        Ok(Self {
            context,
            appearance: Appearance {
                color_scheme: if theme == ThemeMode::Light {
                    liquid_glass::material::tokens::ColorScheme::Light
                } else {
                    liquid_glass::material::tokens::ColorScheme::Dark
                },
                reduce_transparency: false,
                increase_contrast: false,
                reduce_motion: false,
            },
            started: std::time::Instant::now(),
        })
    }

    /// Update resolved appearance from confirmed host state.
    pub fn set_theme(&mut self, theme: ThemeMode) {
        self.appearance.color_scheme = if theme == ThemeMode::Light {
            liquid_glass::material::tokens::ColorScheme::Light
        } else {
            liquid_glass::material::tokens::ColorScheme::Dark
        };
    }

    /// Apply host-confirmed capabilities and persisted material preferences.
    pub fn set_preferences(
        &mut self,
        theme: ThemeMode,
        material: MaterialMode,
        motion: MotionMode,
        capabilities: &volumectl::ui::UiCapabilities,
    ) {
        self.set_theme(theme);
        self.appearance.reduce_transparency =
            material == MaterialMode::Opaque || capabilities.reduced_transparency;
        self.appearance.increase_contrast = capabilities.high_contrast;
        self.appearance.reduce_motion = motion != MotionMode::Full || capabilities.reduced_motion;
    }

    /// Create a WGPU surface with the platform-correct alpha path.
    fn create_surface(
        &self,
        window: &std::sync::Arc<winit::window::Window>,
    ) -> Result<wgpu::Surface<'static>, liquid_glass::Error> {
        self.context
            .create_surface(window.clone())
            .map_err(liquid_glass::Error::from)
    }

    /// Create per-window GPU state for a newly presented window.
    ///
    /// # Errors
    ///
    /// Surface creation or configuration failures surface as [`liquid_glass::Error`].
    pub fn create_view(
        &self,
        window: &std::sync::Arc<winit::window::Window>,
    ) -> Result<SurfaceView, liquid_glass::Error> {
        let surface = self.create_surface(window)?;
        let format = surface
            .get_default_config(self.context.adapter(), 1, 1)
            .ok_or_else(|| {
                liquid_glass::Error::Surface("adapter cannot configure composition surface".into())
            })?
            .format;
        let (width, height): (u32, u32) = window.inner_size().into();
        if width == 0 || height == 0 {
            return Err(liquid_glass::Error::Surface(
                "native window has zero dimensions".into(),
            ));
        }
        let manager = SurfaceManager::new(surface, &self.context, width, height, true)
            .map_err(liquid_glass::Error::from)?;
        let mut text = TextSystem::new(self.context.device(), self.context.queue(), format);
        text.resize(self.context.queue(), width, height);
        let pyramid = BlurPyramid::new(self.context.device(), format, width, height);
        let material = MaterialPipeline::new(self.context.device(), format);
        let blur = BlurPipeline::new(self.context.device(), format);
        let composite = CompositePipeline::new(self.context.device(), format);
        let content_texture = create_content_texture(self.context.device(), format, width, height);
        let content_view = content_texture.create_view(&Default::default());
        let global = GlobalUniforms {
            viewport_size: [width as f32, height as f32],
            time: 0.0,
            scale_factor: 1.0,
        };
        let global_buffer =
            self.context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("host_global_uniforms"),
                    contents: bytemuck::bytes_of(&global),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
        let global_bind = self
            .context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("host_global_bind"),
                layout: material.global_layout(),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: global_buffer.as_entire_binding(),
                }],
            });
        Ok(SurfaceView {
            manager,
            text,
            format,
            pyramid,
            material,
            blur,
            composite,
            content_texture,
            content_view,
            global_buffer,
            global_bind,
            primitive_buffers: Vec::new(),
            primitive_binds: Vec::new(),
            text_buffers: HashMap::new(),
            width: width.max(1),
            height: height.max(1),
        })
    }

    /// Render one scene into its window surface and present it.
    ///
    /// Transient surface acquisition outcomes follow the documented recovery
    /// paths: skipped frames and validation states retry on the next redraw.
    ///
    /// # Errors
    ///
    /// Propagates liquid_glass GPU errors, e.g. device loss during poll.
    pub fn render(
        &mut self,
        window: &std::sync::Arc<winit::window::Window>,
        view: &mut SurfaceView,
        scene: &Scene,
    ) -> Result<(), liquid_glass::Error> {
        let (physical_w, physical_h): (u32, u32) = window.inner_size().into();
        let scale = window.scale_factor() as f32;
        let logical_w = physical_w as f32 / scale;
        let logical_h = physical_h as f32 / scale;

        if (view.width, view.height) != (physical_w.max(1), physical_h.max(1)) {
            view.width = physical_w.max(1);
            view.height = physical_h.max(1);
            view.manager
                .reconfigure(&self.context, view.width, view.height)
                .map_err(liquid_glass::Error::from)?;
            view.pyramid =
                BlurPyramid::new(self.context.device(), view.format, view.width, view.height);
            view.content_texture =
                create_content_texture(self.context.device(), view.format, view.width, view.height);
            view.content_view = view.content_texture.create_view(&Default::default());
            view.pyramid.invalidate();
        }
        // `create_view` cannot know the final Winit size: placement uses
        // `request_inner_size`, and the first surface may be zero-sized.
        // Always synchronize the text viewport before preparing glyphs.
        view.text
            .resize(self.context.queue(), physical_w.max(1), physical_h.max(1));

        let mut flattener = SceneFlattener::new();
        let primitives: &[RenderPrimitive] = flattener.flatten(scene);

        // Pre-create per-primitive uniform buffers and bind groups.
        ensure_primitive_buffers(self.context.device(), view, primitives.len());
        let global = GlobalUniforms {
            viewport_size: [logical_w, logical_h],
            time: self.started.elapsed().as_secs_f32(),
            scale_factor: scale,
        };
        self.context
            .queue()
            .write_buffer(&view.global_buffer, 0, bytemuck::bytes_of(&global));

        // Text buffer maintenance before any pass borrows text immutably.
        refresh_text_buffers(view, scene);

        let mut encoder =
            self.context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("host_frame_encoder"),
                });

        // 1. Content pass: non-glass primitives into the offscreen target.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("host_content_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view.content_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            draw_primitives(
                self.context.queue(),
                &mut pass,
                view,
                &self.appearance,
                primitives,
                |role| !is_glass(role),
            );
        }

        let scene_revision = scene.content_revision();
        if view
            .pyramid
            .needs_rebuild(scene_revision, view.width, view.height, scale)
        {
            view.blur.execute_with_dimensions(
                self.context.device(),
                &mut encoder,
                &view.pyramid,
                &view.content_view,
                view.width,
                view.height,
            );
            view.pyramid
                .mark_clean(scene_revision, view.width, view.height, scale);
        }

        // Prepare text runs while no render pass is open.
        let default_color =
            if self.appearance.color_scheme == liquid_glass::material::tokens::ColorScheme::Dark {
                Color::rgba(245, 247, 250, 255)
            } else {
                Color::rgba(20, 22, 26, 255)
            };
        let buffers = &view.text_buffers;
        let areas = collect_text_areas(buffers, scene, scale, default_color);
        if !areas.is_empty() {
            view.text
                .prepare(self.context.device(), self.context.queue(), areas)
                .map_err(|error| {
                    liquid_glass::Error::Text(format!("text prepare failed: {error}"))
                })?;
        }

        // 4. Final pass: blurred backdrop, glass panels, then sharp text.
        let frame = match view.manager.acquire(&self.context) {
            Ok(frame) => frame,
            Err(outcome) => {
                log::warn!("surface frame acquisition failed: {outcome:?}");
                self.context.queue().submit(Some(encoder.finish()));
                match outcome {
                    liquid_glass::gpu::error::SurfaceAcquisitionOutcome::SurfaceLost => {
                        let surface = self.create_surface(window)?;
                        view.manager
                            .replace_surface(surface, &self.context)
                            .map_err(liquid_glass::Error::from)?;
                    }
                    liquid_glass::gpu::error::SurfaceAcquisitionOutcome::DeviceLost => {
                        return Err(liquid_glass::Error::Gpu(
                            "graphics device lost; rebuilding GPU context required".into(),
                        ));
                    }
                    _ => {}
                }
                return Ok(());
            }
        };
        {
            log::debug!(
                "frame extents {:?}x{:?}",
                frame.texture().width(),
                frame.texture().height()
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("host_present_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.texture().create_view(&Default::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            let composite_bind = view
                .composite
                .bind_group(&view.pyramid.half.view_b, self.context.device());
            pass.set_pipeline(&view.composite.pipeline);
            pass.set_bind_group(0, &composite_bind, &[]);
            pass.draw(0..3, 0..1);

            draw_primitives(
                self.context.queue(),
                &mut pass,
                view,
                &self.appearance,
                primitives,
                is_glass,
            );
            draw_primitives(
                self.context.queue(),
                &mut pass,
                view,
                &self.appearance,
                primitives,
                |role| !is_glass(role),
            );

            if !view.text_buffers.is_empty() {
                view.text.render(&mut pass).map_err(|error| {
                    liquid_glass::Error::Text(format!("text render failed: {error}"))
                })?;
            }
        }
        self.context.queue().submit(Some(encoder.finish()));
        frame.present(self.context.queue());
        log::debug!(
            "presented surface {:?} {}x{}",
            view.format,
            view.width,
            view.height
        );
        Ok(())
    }
}

fn create_content_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host_content_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// Grow per-primitive uniform buffers to at least `count` slots.
fn ensure_primitive_buffers(device: &wgpu::Device, view: &mut SurfaceView, count: usize) {
    while view.primitive_buffers.len() < count {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("host_material_uniform"),
            size: std::mem::size_of::<MaterialUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("host_material_bind"),
            layout: view.material.material_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        view.primitive_buffers.push(buffer);
        view.primitive_binds.push(bind);
    }
}

/// Draw primitives whose role passes `filter` with SDF glass materials.
fn draw_primitives<'pass>(
    queue: &wgpu::Queue,
    pass: &mut wgpu::RenderPass<'pass>,
    view: &SurfaceView,
    appearance: &Appearance,
    primitives: &[RenderPrimitive],
    filter: impl Fn(MaterialRole) -> bool,
) {
    let selected: Vec<(usize, &RenderPrimitive)> = primitives
        .iter()
        .enumerate()
        .filter(|(_, primitive)| filter(primitive.role))
        .collect();
    if selected.is_empty() {
        return;
    }
    pass.set_pipeline(view.material.pipeline());
    pass.set_vertex_buffer(0, view.material.vertex_buffer().slice(..));

    for (index, primitive) in selected {
        // Original scene index, not filtered pass position: content and
        // glass passes write disjoint uniform slots before one submission.
        let tokens = MaterialTokens::resolve(primitive.role, appearance);
        let uniforms = MaterialUniforms {
            rect: [
                primitive.bounds.x,
                primitive.bounds.y,
                primitive.bounds.width,
                primitive.bounds.height,
            ],
            corner_radii: corner_radii_for(primitive.role),
            tint_color: [
                tokens.tint_color.x,
                tokens.tint_color.y,
                tokens.tint_color.z,
                tokens.tint_color.w * tokens.opacity,
            ],
            optical_params: [
                tokens.blur_radius,
                tokens.saturation,
                tokens.specular_rim,
                tokens.edge_width,
            ],
        };
        let buffer = &view.primitive_buffers[index];
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&uniforms));
        pass.set_bind_group(0, &view.global_bind, &[]);
        pass.set_bind_group(1, &view.primitive_binds[index], &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Create or refresh shaped glyphon buffers for every visible text node.
fn refresh_text_buffers(view: &mut SurfaceView, scene: &Scene) {
    let family = liquid_glass::text::default_font_family().to_string();
    for (id, node) in scene.iter() {
        let Some(text) = node.text.as_ref() else {
            continue;
        };
        if !node.visible || text.is_empty() {
            continue;
        }
        let stale = match view.text_buffers.get(id) {
            Some((stored, _)) => stored != text,
            None => true,
        };
        if stale {
            let props = TextProperties {
                font_size: 14.0,
                line_height: 18.0,
                family: family.as_str(),
                weight: liquid_glass::text::Weight::NORMAL,
            };
            let font_system = view.text.font_system_mut();
            let buffer = TextLayout::create_buffer(
                font_system,
                text,
                &props,
                Some(node.layout_bounds.width.max(1.0)),
            );
            view.text_buffers.insert(*id, (text.clone(), buffer));
        }
    }
}

/// Horizontal padding applied to left-aligned text, logical px.
const TEXT_PADDING_X: f32 = 8.0;
/// Text line height used by `TextProperties` (see `refresh_text_buffers`).
const TEXT_LINE_HEIGHT: f32 = 18.0;

/// Collect immutable glyphon text areas for the present pass.
///
/// Text is vertically centered inside the node's bounds and horizontally
/// centered for button-like roles (`Control`/`Accent`), with a per-node
/// override via `SceneNode::text_centered`; other roles render left-aligned
/// with padding so labels do not touch the glass edge.
fn collect_text_areas<'a>(
    buffers: &'a HashMap<SemanticId, (String, glyphon::Buffer)>,
    scene: &Scene,
    scale: f32,
    default_color: Color,
) -> Vec<TextArea<'a>> {
    let mut areas = Vec::new();
    for (id, node) in scene.iter() {
        let Some(text) = node.text.as_ref() else {
            continue;
        };
        if !node.visible || text.is_empty() {
            continue;
        }
        let Some((_, buffer)) = buffers.get(id) else {
            continue;
        };
        let bounds = &node.layout_bounds;
        let centered = node.text_centered.unwrap_or(matches!(
            node.role,
            MaterialRole::Control | MaterialRole::Accent
        ));
        let text_width = TextLayout::measure_buffer(buffer).0;
        let left = if centered && bounds.width > text_width {
            bounds.x + (bounds.width - text_width) / 2.0
        } else {
            (bounds.x + TEXT_PADDING_X).min(bounds.x + bounds.width)
        };
        let top = bounds.y + ((bounds.height - TEXT_LINE_HEIGHT) / 2.0).max(0.0);
        areas.push(TextArea {
            buffer,
            left: left * scale,
            top: top * scale,
            scale,
            bounds: TextBounds {
                left: (bounds.x * scale).floor() as i32,
                top: (bounds.y * scale).floor() as i32,
                right: ((bounds.x + bounds.width) * scale).ceil() as i32,
                bottom: ((bounds.y + bounds.height) * scale).ceil() as i32,
            },
            default_color,
            custom_glyphs: &[],
        });
    }
    areas
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquid_glass::scene::node::SemanticId;

    #[test]
    fn filtered_primitive_slots_remain_distinct_across_passes() {
        let primitives = [
            RenderPrimitive {
                id: SemanticId(1),
                bounds: liquid_glass::scene::Rect::new(0.0, 0.0, 100.0, 100.0),
                role: MaterialRole::Panel,
                z_order: 0,
                clip: None,
            },
            RenderPrimitive {
                id: SemanticId(2),
                bounds: liquid_glass::scene::Rect::new(4.0, 4.0, 50.0, 20.0),
                role: MaterialRole::Control,
                z_order: 1,
                clip: None,
            },
        ];
        let slots = |filter: fn(MaterialRole) -> bool| -> Vec<usize> {
            primitives
                .iter()
                .enumerate()
                .filter(|(_, primitive)| filter(primitive.role))
                .map(|(index, _)| index)
                .collect()
        };
        assert_eq!(slots(is_glass), vec![0]);
        assert_eq!(slots(|role| !is_glass(role)), vec![1]);
        assert_ne!(slots(is_glass)[0], slots(|role| !is_glass(role))[0]);
    }
}
