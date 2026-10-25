//! # Liquid Glass
//!
//! Owned, portable WGPU-based native UI renderer providing high-fidelity
//! Liquid Glass optics, retained semantic scene management, closed-form
//! analytic springs, and AccessKit accessibility for VolumeControl.

pub mod a11y;
pub mod blur;
pub mod controller;
pub mod gpu;
pub mod input;
pub mod material;
pub mod motion;
pub mod scene;
pub mod surfaces;
pub mod text;

pub use controller::LiquidGlassSurfaceController;
use thiserror::Error;

/// Top-level error type for the Liquid Glass renderer.
#[derive(Debug, Error)]
pub enum Error {
    #[error("GPU error: {0}")]
    Gpu(String),
    #[error("Surface acquisition failed: {0}")]
    Surface(String),
    #[error("Text rendering error: {0}")]
    Text(String),
    #[error("Scene error: {0}")]
    Scene(String),
}
impl From<gpu::GpuError> for Error {
    fn from(err: gpu::GpuError) -> Self {
        Self::Gpu(err.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Version metadata for the Liquid Glass renderer.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn error_display_formats() {
        let err = Error::Gpu("device lost".into());
        assert_eq!(err.to_string(), "GPU error: device lost");
    }
}
