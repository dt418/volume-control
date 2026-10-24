//! WGPU pipeline executing separable 1D Gaussian blur ping-pong passes.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Uniforms controlling Gaussian blur direction, kernel radius, and texel sampling steps.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BlurUniforms {
    pub direction: [f32; 2],
    pub radius: f32,
    pub _pad1: f32,
    pub texel_size: [f32; 2],
    pub _pad2: [f32; 2],
}

/// Pipeline executing separable horizontal and vertical Gaussian blur passes.
pub struct BlurPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    h_uniform_buffer: wgpu::Buffer,
    v_uniform_buffer: wgpu::Buffer,
}

impl BlurPipeline {
    /// Initialize the blur pipeline for the given render target texture format.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gaussian_blur_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blur.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gaussian_blur_pipeline"),
            layout: Some(&pipeline_layout),
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
                    blend: None, // Direct copy/filter into ping-pong buffer
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blur_linear_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let h_uniform = BlurUniforms {
            direction: [1.0, 0.0],
            radius: 1.0,
            _pad1: 0.0,
            texel_size: [1.0 / 800.0, 1.0 / 600.0],
            _pad2: [0.0, 0.0],
        };
        let h_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_uniform_horizontal"),
            contents: bytemuck::bytes_of(&h_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let v_uniform = BlurUniforms {
            direction: [0.0, 1.0],
            radius: 1.0,
            _pad1: 0.0,
            texel_size: [1.0 / 800.0, 1.0 / 600.0],
            _pad2: [0.0, 0.0],
        };
        let v_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_uniform_vertical"),
            contents: bytemuck::bytes_of(&v_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            h_uniform_buffer,
            v_uniform_buffer,
        }
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn h_uniform_buffer(&self) -> &wgpu::Buffer {
        &self.h_uniform_buffer
    }

    pub fn v_uniform_buffer(&self) -> &wgpu::Buffer {
        &self.v_uniform_buffer
    }

    /// Record a single 1D blur pass from source_view into target_view using the specified uniform buffer.
    pub fn render_1d_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        uniform_buffer: &wgpu::Buffer,
        source_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        pass_label: &str,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{}_bind_group", pass_label)),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(pass_label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1); // Fullscreen triangle
    }

    /// Execute dual-pass horizontal and vertical Gaussian blur passes across all 3 pyramid levels.
    ///
    /// Level 1 (Half): source -> view_a (horizontal) -> view_b (vertical).
    /// Level 2 (Quarter): half.view_b -> view_a (horizontal) -> view_b (vertical).
    /// Level 3 (Eighth): quarter.view_b -> view_a (horizontal) -> view_b (vertical).
    pub fn execute(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pyramid: &super::pyramid::BlurPyramid,
        source_view: &wgpu::TextureView,
    ) {
        self.execute_with_dimensions(device, encoder, pyramid, source_view, 0, 0);
    }

    /// Execute blur with actual source dimensions. Zero uses pyramid half extents.
    pub fn execute_with_dimensions(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pyramid: &super::pyramid::BlurPyramid,
        source_view: &wgpu::TextureView,
        source_width: u32,
        source_height: u32,
    ) {
        let width = if source_width == 0 {
            pyramid.half.width
        } else {
            source_width
        };
        let height = if source_height == 0 {
            pyramid.half.height
        } else {
            source_height
        };
        let horizontal = BlurUniforms {
            direction: [1.0, 0.0],
            radius: 1.0,
            _pad1: 0.0,
            texel_size: [1.0 / width.max(1) as f32, 1.0 / height.max(1) as f32],
            _pad2: [0.0, 0.0],
        };
        let vertical = BlurUniforms {
            direction: [0.0, 1.0],
            ..horizontal
        };
        let horizontal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_uniform_horizontal_runtime"),
            contents: bytemuck::bytes_of(&horizontal),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let vertical_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_uniform_vertical_runtime"),
            contents: bytemuck::bytes_of(&vertical),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        self.render_1d_pass(
            device,
            encoder,
            &horizontal_buffer,
            source_view,
            &pyramid.half.view_a,
            "blur_half_horizontal",
        );
        self.render_1d_pass(
            device,
            encoder,
            &vertical_buffer,
            &pyramid.half.view_a,
            &pyramid.half.view_b,
            "blur_half_vertical",
        );

        // 2. Quarter resolution level
        self.render_1d_pass(
            device,
            encoder,
            &horizontal_buffer,
            &pyramid.half.view_b,
            &pyramid.quarter.view_a,
            "blur_quarter_horizontal",
        );
        self.render_1d_pass(
            device,
            encoder,
            &vertical_buffer,
            &pyramid.quarter.view_a,
            &pyramid.quarter.view_b,
            "blur_quarter_vertical",
        );

        // 3. Eighth resolution level
        self.render_1d_pass(
            device,
            encoder,
            &horizontal_buffer,
            &pyramid.quarter.view_b,
            &pyramid.eighth.view_a,
            "blur_eighth_horizontal",
        );
        self.render_1d_pass(
            device,
            encoder,
            &vertical_buffer,
            &pyramid.eighth.view_a,
            &pyramid.eighth.view_b,
            "blur_eighth_vertical",
        );
    }
}
