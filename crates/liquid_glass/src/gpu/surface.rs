//! Surface configuration, 7-state acquisition lifecycle, and error recovery.

use crate::gpu::context::GpuContext;
use crate::gpu::error::{GpuError, SurfaceAcquisitionOutcome};

/// An acquired surface frame ready to render into and present.
pub struct AcquiredFrame {
    surface_texture: wgpu::SurfaceTexture,
    pub suboptimal: bool,
}

impl AcquiredFrame {
    /// Borrow the underlying WGPU texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.surface_texture.texture
    }

    /// Present the texture to the swapchain via the device queue.
    pub fn present(self, queue: &wgpu::Queue) {
        queue.present(self.surface_texture);
    }
}

/// Manages WGPU surface configuration, frame acquisition, and error recovery.
pub struct SurfaceManager<'window> {
    surface: wgpu::Surface<'window>,
    config: wgpu::SurfaceConfiguration,
    last_width: u32,
    last_height: u32,
    suboptimal_scheduled: bool,
    in_validation_error: bool,
    is_configured: bool,
}

impl<'window> SurfaceManager<'window> {
    /// Create a new surface manager with default configuration.
    ///
    /// `transparent` selects premultiplied alpha when the surface supports it,
    /// letting the OS backdrop (Mica / Acrylic / NSVisualEffectView) show
    /// through the swapchain instead of being hidden behind an opaque frame.
    pub fn new(
        surface: wgpu::Surface<'window>,
        context: &GpuContext,
        width: u32,
        height: u32,
        transparent: bool,
    ) -> Result<Self, GpuError> {
        let mut config = surface
            .get_default_config(context.adapter(), width.max(1), height.max(1))
            .ok_or_else(|| {
                GpuError::SurfaceConfiguration(
                    "Surface does not support any configurations with this adapter".into(),
                )
            })?;
        let capabilities = surface.get_capabilities(context.adapter());
        // Transparent windows (vibrancy / DWM backdrop) require a premultiplied
        // swapchain: Opaque discards the frame alpha and hides the OS backdrop.
        config.alpha_mode = if transparent
            && capabilities
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            wgpu::CompositeAlphaMode::Auto
        };

        let mut manager = Self {
            surface,
            config,
            last_width: width,
            last_height: height,
            suboptimal_scheduled: false,
            in_validation_error: false,
            is_configured: false,
        };

        if width > 0 && height > 0 {
            manager.reconfigure(context, width, height)?;
        }

        Ok(manager)
    }

    /// Reconfigure the surface to new nonzero dimensions.
    pub fn reconfigure(
        &mut self,
        context: &GpuContext,
        width: u32,
        height: u32,
    ) -> Result<(), GpuError> {
        if width == 0 || height == 0 {
            // Window is minimized or has zero size; avoid configuring invalid 0x0 extents
            self.is_configured = false;
            return Ok(());
        }

        self.last_width = width;
        self.last_height = height;
        self.config.width = width;
        self.config.height = height;

        self.surface.configure(context.device(), &self.config);
        self.is_configured = true;
        self.in_validation_error = false;
        Ok(())
    }

    /// Replace the underlying surface (e.g. after a surface Lost event).
    pub fn replace_surface(
        &mut self,
        new_surface: wgpu::Surface<'window>,
        context: &GpuContext,
    ) -> Result<(), GpuError> {
        self.surface = new_surface;
        if self.last_width > 0 && self.last_height > 0 {
            self.reconfigure(context, self.last_width, self.last_height)?;
        }
        Ok(())
    }

    /// Acquire the next surface frame for presentation.
    pub fn acquire(
        &mut self,
        context: &GpuContext,
    ) -> Result<AcquiredFrame, SurfaceAcquisitionOutcome> {
        if context.is_device_lost() {
            return Err(SurfaceAcquisitionOutcome::DeviceLost);
        }
        if !self.is_configured {
            return Err(SurfaceAcquisitionOutcome::SkipFrame {
                reason: "Surface is not configured (zero extents)",
            });
        }

        if self.in_validation_error {
            return Err(SurfaceAcquisitionOutcome::ValidationFailed);
        }

        if self.suboptimal_scheduled {
            self.suboptimal_scheduled = false;
            let _ = self.reconfigure(context, self.last_width, self.last_height);
        }

        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => Ok(AcquiredFrame {
                surface_texture,
                suboptimal: false,
            }),
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.suboptimal_scheduled = true;
                Ok(AcquiredFrame {
                    surface_texture,
                    suboptimal: true,
                })
            }
            wgpu::CurrentSurfaceTexture::Timeout => Err(SurfaceAcquisitionOutcome::SkipFrame {
                reason: "Surface acquisition timeout",
            }),
            wgpu::CurrentSurfaceTexture::Occluded => Err(SurfaceAcquisitionOutcome::SkipFrame {
                reason: "Surface occluded",
            }),
            wgpu::CurrentSurfaceTexture::Outdated => {
                let _ = self.reconfigure(context, self.last_width, self.last_height);
                Err(SurfaceAcquisitionOutcome::Reconfigured)
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                if context.is_device_lost() {
                    Err(SurfaceAcquisitionOutcome::DeviceLost)
                } else {
                    Err(SurfaceAcquisitionOutcome::SurfaceLost)
                }
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                self.in_validation_error = true;
                Err(SurfaceAcquisitionOutcome::ValidationFailed)
            }
        }
    }

    /// Evaluate an acquisition status and execute recovery logic (useful for testing).
    pub fn handle_status(
        &mut self,
        context: &GpuContext,
        status: &wgpu::CurrentSurfaceTexture,
    ) -> SurfaceAcquisitionOutcome {
        if context.is_device_lost() {
            return SurfaceAcquisitionOutcome::DeviceLost;
        }

        match status {
            wgpu::CurrentSurfaceTexture::Success(_) => {
                SurfaceAcquisitionOutcome::Ready { suboptimal: false }
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                self.suboptimal_scheduled = true;
                SurfaceAcquisitionOutcome::Ready { suboptimal: true }
            }
            wgpu::CurrentSurfaceTexture::Timeout => SurfaceAcquisitionOutcome::SkipFrame {
                reason: "Surface acquisition timeout",
            },
            wgpu::CurrentSurfaceTexture::Occluded => SurfaceAcquisitionOutcome::SkipFrame {
                reason: "Surface occluded",
            },
            wgpu::CurrentSurfaceTexture::Outdated => {
                let _ = self.reconfigure(context, self.last_width, self.last_height);
                SurfaceAcquisitionOutcome::Reconfigured
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                if context.is_device_lost() {
                    SurfaceAcquisitionOutcome::DeviceLost
                } else {
                    SurfaceAcquisitionOutcome::SurfaceLost
                }
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                self.in_validation_error = true;
                SurfaceAcquisitionOutcome::ValidationFailed
            }
        }
    }

    pub fn surface(&self) -> &wgpu::Surface<'window> {
        &self.surface
    }

    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    pub fn is_configured(&self) -> bool {
        self.is_configured
    }

    pub fn is_in_validation_error(&self) -> bool {
        self.in_validation_error
    }

    pub fn is_suboptimal_scheduled(&self) -> bool {
        self.suboptimal_scheduled
    }
}
