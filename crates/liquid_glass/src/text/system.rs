//! High-level TextSystem integrating Glyphon, cosmic-text, and WGPU render passes.

use glyphon::{
    Cache, FontSystem, PrepareError, RenderError, Resolution, SwashCache, TextArea, TextAtlas,
    TextRenderer, Viewport,
};

/// High-level text shaping, caching, and WGPU draw pipeline manager.
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    cache: Cache,
    atlas: TextAtlas,
    viewport: Viewport,
    renderer: TextRenderer,
}

impl TextSystem {
    /// Initialize text atlas, caching, and shader pipelines for the target surface format.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let mut viewport = Viewport::new(device, &cache);
        // A newly constructed Viewport is 0x0. Glyphon divides by its
        // resolution and clips all glyphs until a real size is assigned.
        viewport.update(
            queue,
            Resolution {
                width: 1,
                height: 1,
            },
        );
        let renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        Self {
            font_system,
            swash_cache,
            cache,
            atlas,
            viewport,
            renderer,
        }
    }

    /// Resize the text viewport resolution matching current surface extents.
    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.viewport.update(
            queue,
            Resolution {
                width: width.max(1),
                height: height.max(1),
            },
        );
    }

    /// Prepare text runs and rasterize missing glyphs into the atlas before rendering.
    pub fn prepare<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text_areas: impl IntoIterator<Item = TextArea<'a>>,
    ) -> Result<(), PrepareError> {
        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        )
    }

    /// Render shaped text runs into an active WGPU render pass.
    pub fn render<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
    ) -> Result<(), RenderError> {
        self.renderer.render(&self.atlas, &self.viewport, pass)
    }

    /// Current text viewport resolution in physical pixels.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.viewport.resolution()
    }

    /// Trim old or unused glyphs from the atlas cache.
    pub fn trim(&mut self) {
        self.atlas.trim();
    }

    /// Mutable reference to the underlying `FontSystem` (for shaping and buffer creation).
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Shared reference to the underlying Glyphon `Cache`.
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// Shared reference to the underlying `FontSystem`.
    pub fn font_system(&self) -> &FontSystem {
        &self.font_system
    }
}
