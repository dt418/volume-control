//! Downsampled blur pyramid levels and content-revision cache management.

/// Cache validation key for the blur pyramid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlurCacheKey {
    pub content_revision: u64,
    pub width: u32,
    pub height: u32,
    pub scale_factor: u32,
}

impl BlurCacheKey {
    pub fn new(content_revision: u64, width: u32, height: u32, scale_factor: f32) -> Self {
        Self {
            content_revision,
            width,
            height,
            scale_factor: (scale_factor * 1000.0) as u32,
        }
    }
}

/// A single downsampled resolution level in the blur pyramid containing ping-pong textures.
pub struct BlurLevel {
    pub width: u32,
    pub height: u32,
    pub texture_a: wgpu::Texture,
    pub view_a: wgpu::TextureView,
    pub texture_b: wgpu::Texture,
    pub view_b: wgpu::TextureView,
}

impl BlurLevel {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        label: &str,
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);

        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture_a = device.create_texture(&desc);
        let view_a = texture_a.create_view(&Default::default());

        let texture_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{}_pingpong", label)),
            ..desc
        });
        let view_b = texture_b.create_view(&Default::default());

        Self {
            width: w,
            height: h,
            texture_a,
            view_a,
            texture_b,
            view_b,
        }
    }
}

/// Multi-resolution blur pyramid caching 1/2, 1/4, and 1/8 downsampled textures.
pub struct BlurPyramid {
    pub half: BlurLevel,
    pub quarter: BlurLevel,
    pub eighth: BlurLevel,
    cached_key: Option<BlurCacheKey>,
}

impl BlurPyramid {
    /// Construct a new blur pyramid scaled to the given root surface dimensions.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let half = BlurLevel::new(
            device,
            format,
            (width / 2).max(1),
            (height / 2).max(1),
            "blur_pyramid_half",
        );
        let quarter = BlurLevel::new(
            device,
            format,
            (width / 4).max(1),
            (height / 4).max(1),
            "blur_pyramid_quarter",
        );
        let eighth = BlurLevel::new(
            device,
            format,
            (width / 8).max(1),
            (height / 8).max(1),
            "blur_pyramid_eighth",
        );

        Self {
            half,
            quarter,
            eighth,
            cached_key: None,
        }
    }

    /// Check if the pyramid must be rebuilt due to changed revision, scale factor, or resized extents.
    pub fn needs_rebuild(
        &self,
        content_revision: u64,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> bool {
        match self.cached_key {
            Some(key) => key != BlurCacheKey::new(content_revision, width, height, scale_factor),
            None => true,
        }
    }

    /// Record the active cache key after a successful blur pyramid pass.
    pub fn mark_clean(
        &mut self,
        content_revision: u64,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) {
        self.cached_key = Some(BlurCacheKey::new(
            content_revision,
            width,
            height,
            scale_factor,
        ));
    }

    /// Invalidate the active cache key (forces re-blur on next frame).
    pub fn invalidate(&mut self) {
        self.cached_key = None;
    }
}
