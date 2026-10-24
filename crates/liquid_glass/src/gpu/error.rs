//! GPU and surface error types and recovery policies.

use thiserror::Error;

/// GPU subsystem error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GpuError {
    #[error("No compatible graphics adapter found for requested backends: {0}")]
    NoAdapter(String),
    #[error("Failed to request graphics device: {0}")]
    RequestDevice(String),
    #[error("Surface creation failed: {0}")]
    SurfaceCreation(String),
    #[error("Surface configuration failed: {0}")]
    SurfaceConfiguration(String),
    #[error("Graphics device was lost and must be recreated")]
    DeviceLost,
    #[error("Out of memory on graphics device")]
    OutOfMemory,
}

/// High-level outcome of attempting to acquire a surface texture for rendering.
#[derive(Debug, PartialEq, Eq)]
pub enum SurfaceAcquisitionOutcome {
    /// Texture acquired and ready to render.
    Ready { suboptimal: bool },
    /// Transient status; skip current frame and await next redraw/exposure.
    SkipFrame { reason: &'static str },
    /// Surface configuration was outdated; reconfigured and ready for next attempt.
    Reconfigured,
    /// Surface was lost; caller must recreate surface and invoke `replace_surface()`.
    SurfaceLost,
    /// Device loss was detected; caller must rebuild entire GPU stack.
    DeviceLost,
    /// Validation error occurred; surface is in controlled error state.
    ValidationFailed,
}

/// Recovery action mapped from raw WGPU `CurrentSurfaceTexture` statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRecoveryAction {
    /// Normal presentation: render, submit, present.
    RenderAndPresent,
    /// Suboptimal presentation: render, submit, present, then schedule reconfigure.
    RenderPresentAndReconfigure,
    /// Skip frame: timeout or occlusion.
    SkipFrame,
    /// Reconfigure from latest nonzero extents before next acquisition.
    Reconfigure,
    /// Recreate surface via instance.create_surface(), then configure.
    RecreateSurface,
    /// Rebuild adapter, device, queue, surfaces, and resources.
    RebuildDevice,
    /// Hold error state until valid configuration exists.
    HoldError,
    /// Fatal out-of-memory: clean shutdown without retry loops.
    FatalOom,
}

impl SurfaceRecoveryAction {
    /// Classify a raw `wgpu::CurrentSurfaceTexture` status into a recovery action.
    pub fn from_status(status: &wgpu::CurrentSurfaceTexture, device_lost: bool) -> Self {
        if device_lost {
            return Self::RebuildDevice;
        }

        match status {
            wgpu::CurrentSurfaceTexture::Success(_) => Self::RenderAndPresent,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => Self::RenderPresentAndReconfigure,
            wgpu::CurrentSurfaceTexture::Timeout => Self::SkipFrame,
            wgpu::CurrentSurfaceTexture::Occluded => Self::SkipFrame,
            wgpu::CurrentSurfaceTexture::Outdated => Self::Reconfigure,
            wgpu::CurrentSurfaceTexture::Lost => Self::RecreateSurface,
            wgpu::CurrentSurfaceTexture::Validation => Self::HoldError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_loss_overrides_status() {
        let action =
            SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Timeout, true);
        assert_eq!(action, SurfaceRecoveryAction::RebuildDevice);
    }

    #[test]
    fn recovery_action_mapping_for_all_non_presentable_statuses() {
        assert_eq!(
            SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Timeout, false),
            SurfaceRecoveryAction::SkipFrame
        );
        assert_eq!(
            SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Occluded, false),
            SurfaceRecoveryAction::SkipFrame
        );
        assert_eq!(
            SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Outdated, false),
            SurfaceRecoveryAction::Reconfigure
        );
        assert_eq!(
            SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Lost, false),
            SurfaceRecoveryAction::RecreateSurface
        );
        assert_eq!(
            SurfaceRecoveryAction::from_status(&wgpu::CurrentSurfaceTexture::Validation, false),
            SurfaceRecoveryAction::HoldError
        );
    }
}
